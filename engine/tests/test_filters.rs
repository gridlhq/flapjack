use flapjack::index::settings::IndexSettings;
use flapjack::types::{Document, FieldValue, Filter};
use flapjack::IndexManager;
use flapjack_http::dto::SearchRequest;
use flapjack_http::filter_parser::parse_filter;
use std::collections::HashMap;
use tempfile::TempDir;

fn make_request(json: serde_json::Value) -> SearchRequest {
    serde_json::from_value(json).expect("Failed to deserialize SearchRequest")
}

mod params {
    use super::*;

    #[test]
    fn facet_filters_single_string() {
        let req =
            make_request(serde_json::json!({"query": "test", "facetFilters": ["brand:Apple"]}));
        let filter = req.build_combined_filter().unwrap();
        match filter {
            Filter::Equals { field, value } => {
                assert_eq!(field, "brand");
                assert_eq!(value, FieldValue::Text("Apple".to_string()));
            }
            _ => panic!("Expected Equals, got {:?}", filter),
        }
    }

    #[test]
    fn facet_filters_disjunctive() {
        let req = make_request(
            serde_json::json!({"query": "test", "facetFilters": [["brand:Apple", "brand:Samsung"]]}),
        );
        let filter = req.build_combined_filter().unwrap();
        match filter {
            Filter::Or(parts) => assert_eq!(parts.len(), 2),
            _ => panic!("Expected Or"),
        }
    }

    #[test]
    fn facet_filters_mixed_and_or() {
        let req = make_request(
            serde_json::json!({"query": "test", "facetFilters": [["brand:Apple", "brand:Samsung"], "category:Electronics"]}),
        );
        let filter = req.build_combined_filter().unwrap();
        match filter {
            Filter::And(parts) => {
                assert_eq!(parts.len(), 2);
                assert!(matches!(&parts[0], Filter::Or(p) if p.len() == 2));
                assert!(matches!(&parts[1], Filter::Equals { field, .. } if field == "category"));
            }
            _ => panic!("Expected And"),
        }
    }

    #[test]
    fn facet_filters_negated() {
        let req =
            make_request(serde_json::json!({"query": "test", "facetFilters": ["-brand:Apple"]}));
        let filter = req.build_combined_filter().unwrap();
        assert!(
            matches!(filter, Filter::Not(inner) if matches!(*inner, Filter::Equals { ref field, .. } if field == "brand"))
        );
    }

    #[test]
    fn numeric_filters_simple() {
        let req = make_request(
            serde_json::json!({"query": "test", "numericFilters": ["price>=10", "price<=100"]}),
        );
        let filter = req.build_combined_filter().unwrap();
        match filter {
            Filter::And(parts) => {
                assert_eq!(parts.len(), 2);
                assert!(
                    matches!(&parts[0], Filter::GreaterThanOrEqual { field, value } if field == "price" && *value == FieldValue::Integer(10))
                );
                assert!(
                    matches!(&parts[1], Filter::LessThanOrEqual { field, value } if field == "price" && *value == FieldValue::Integer(100))
                );
            }
            _ => panic!("Expected And"),
        }
    }

    #[test]
    fn numeric_filters_float() {
        let req =
            make_request(serde_json::json!({"query": "test", "numericFilters": ["price>=10.50"]}));
        let filter = req.build_combined_filter().unwrap();
        assert!(
            matches!(filter, Filter::GreaterThanOrEqual { ref field, value: FieldValue::Float(f) } if field == "price" && f == 10.50)
        );
    }

    #[test]
    fn combined_filters_and_facet_filters() {
        let req = make_request(
            serde_json::json!({"query": "test", "filters": "price > 50", "facetFilters": ["brand:Apple"]}),
        );
        let filter = req.build_combined_filter().unwrap();
        assert!(matches!(filter, Filter::And(parts) if parts.len() == 2));
    }

    #[test]
    fn tag_filters() {
        let req = make_request(
            serde_json::json!({"query": "test", "tagFilters": ["electronics", "sale"]}),
        );
        let filter = req.build_combined_filter().unwrap();
        match filter {
            Filter::And(parts) => {
                assert_eq!(parts.len(), 2);
                for p in &parts {
                    assert!(matches!(p, Filter::Equals { field, .. } if field == "_tags"));
                }
            }
            _ => panic!("Expected And"),
        }
    }

    #[test]
    fn params_string_parsing() {
        let mut req = make_request(
            serde_json::json!({"indexName": "products", "params": "query=laptop&hitsPerPage=5&page=2&facets=%5B%22brand%22%2C%22category%22%5D"}),
        );
        req.apply_params_string();
        assert_eq!(req.query, "laptop");
        assert_eq!(req.hits_per_page, Some(5));
        assert_eq!(req.page, 2);
        assert_eq!(
            req.facets,
            Some(vec!["brand".to_string(), "category".to_string()])
        );
    }

    #[test]
    fn params_string_with_facet_filters() {
        let mut req = make_request(
            serde_json::json!({"indexName": "products", "params": "query=phone&facetFilters=%5B%5B%22brand%3AApple%22%2C%22brand%3ASamsung%22%5D%5D"}),
        );
        req.apply_params_string();
        assert_eq!(req.query, "phone");
        let filter = req.build_combined_filter().unwrap();
        assert!(matches!(filter, Filter::Or(parts) if parts.len() == 2));
    }

    #[test]
    fn params_string_does_not_override_explicit_fields() {
        let mut req = make_request(
            serde_json::json!({"indexName": "products", "query": "explicit", "hitsPerPage": 10, "params": "query=fromparams&hitsPerPage=5"}),
        );
        req.apply_params_string();
        assert_eq!(req.query, "explicit");
        assert_eq!(req.hits_per_page, Some(10));
    }

    #[test]
    fn empty_facet_filters() {
        let req = make_request(serde_json::json!({"query": "test", "facetFilters": []}));
        assert!(req.build_combined_filter().is_none());
    }

    #[test]
    fn all_filter_types_combined() {
        let req = make_request(
            serde_json::json!({"query": "test", "filters": "in_stock:true", "facetFilters": ["brand:Apple"], "numericFilters": ["price>=10"], "tagFilters": ["sale"]}),
        );
        let filter = req.build_combined_filter().unwrap();
        assert!(matches!(filter, Filter::And(parts) if parts.len() == 4));
    }
}

mod precedence {
    use super::*;

    #[test]
    fn and_of_ors() {
        let result = parse_filter("(brand:Nike OR brand:Adidas) AND category:Shoes");
        assert!(result.is_ok());
    }

    #[test]
    fn or_of_ands() {
        let result =
            parse_filter("(brand:Nike AND category:Shoes) OR (brand:Adidas AND category:Apparel)");
        assert!(result.is_ok());
    }

    #[test]
    fn algolia_example_from_docs() {
        let result = parse_filter("(county:Maricopa OR county:Pima) AND employees > 500");
        assert!(result.is_ok());
    }
}

mod enforcement {
    use super::*;

    #[tokio::test]
    async fn facet_filter_requires_settings() {
        let temp = TempDir::new().unwrap();
        let manager = IndexManager::new(temp.path());
        manager.create_tenant("test").unwrap();

        let mut fields = HashMap::new();
        fields.insert(
            "category".to_string(),
            FieldValue::Text("electronics".to_string()),
        );
        fields.insert("name".to_string(), FieldValue::Text("laptop".to_string()));
        let doc = Document {
            id: "1".to_string(),
            fields,
        };
        manager.add_documents_sync("test", vec![doc]).await.unwrap();

        let filter = parse_filter("category:electronics").unwrap();
        let result = manager.search("test", "", Some(&filter), None, 10).unwrap();
        assert_eq!(
            result.total, 0,
            "Should return 0 without attributesForFaceting"
        );
    }

    #[tokio::test]
    async fn facet_filter_works_with_settings() {
        let temp = TempDir::new().unwrap();
        let manager = IndexManager::new(temp.path());
        manager.create_tenant("test").unwrap();

        let settings = IndexSettings::default_with_facets(vec!["category".to_string()]);
        settings
            .save(temp.path().join("test/settings.json"))
            .unwrap();

        let mut fields = HashMap::new();
        fields.insert(
            "category".to_string(),
            FieldValue::Text("electronics".to_string()),
        );
        fields.insert("name".to_string(), FieldValue::Text("laptop".to_string()));
        let doc = Document {
            id: "1".to_string(),
            fields,
        };
        manager.add_documents_sync("test", vec![doc]).await.unwrap();

        let filter = parse_filter("category:electronics").unwrap();
        let result = manager.search("test", "", Some(&filter), None, 10).unwrap();
        assert_eq!(result.total, 1);
    }

    #[tokio::test]
    async fn numeric_filters_work_without_settings() {
        let temp = TempDir::new().unwrap();
        let manager = IndexManager::new(temp.path());
        manager.create_tenant("test").unwrap();

        let mut fields = HashMap::new();
        fields.insert("price".to_string(), FieldValue::Integer(100));
        fields.insert("name".to_string(), FieldValue::Text("laptop".to_string()));
        let doc = Document {
            id: "1".to_string(),
            fields,
        };
        manager.add_documents_sync("test", vec![doc]).await.unwrap();

        let filter = parse_filter("price >= 50").unwrap();
        let result = manager.search("test", "", Some(&filter), None, 10).unwrap();
        assert_eq!(result.total, 1);
    }
}
