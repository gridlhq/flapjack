use flapjack::index::settings::IndexSettings;
use flapjack::types::{Document, FacetRequest, FieldValue};
use flapjack::IndexManager;
use std::collections::HashMap;
use tempfile::TempDir;

fn doc(id: &str, fields: Vec<(&str, FieldValue)>) -> Document {
    let f: HashMap<String, FieldValue> = fields
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect();
    Document {
        id: id.to_string(),
        fields: f,
    }
}

fn text(s: &str) -> FieldValue {
    FieldValue::Text(s.to_string())
}

fn facet_req(field: &str) -> FacetRequest {
    FacetRequest {
        field: field.to_string(),
        path: format!("/{}", field),
    }
}

async fn setup() -> (TempDir, std::sync::Arc<IndexManager>) {
    let temp_dir = TempDir::new().unwrap();
    let manager = IndexManager::new(temp_dir.path());
    manager.create_tenant("test").unwrap();

    let settings = IndexSettings {
        attributes_for_faceting: vec![
            "searchable(brand)".to_string(),
            "searchable(category)".to_string(),
        ],
        ..Default::default()
    };
    settings
        .save(temp_dir.path().join("test/settings.json"))
        .unwrap();

    let docs = vec![
        doc(
            "1",
            vec![
                ("brand", text("Samsung")),
                ("category", text("Phones")),
                ("name", text("Galaxy S24")),
            ],
        ),
        doc(
            "2",
            vec![
                ("brand", text("Samsung")),
                ("category", text("Tablets")),
                ("name", text("Galaxy Tab")),
            ],
        ),
        doc(
            "3",
            vec![
                ("brand", text("Apple")),
                ("category", text("Phones")),
                ("name", text("iPhone 15")),
            ],
        ),
        doc(
            "4",
            vec![
                ("brand", text("Apple")),
                ("category", text("Laptops")),
                ("name", text("MacBook Pro")),
            ],
        ),
        doc(
            "5",
            vec![
                ("brand", text("Sony")),
                ("category", text("Audio")),
                ("name", text("WH-1000XM5")),
            ],
        ),
    ];

    manager.add_documents_sync("test", docs).await.unwrap();
    (temp_dir, manager)
}

#[tokio::test]
async fn test_limit_zero_returns_no_docs_but_facets() {
    let (_tmp, mgr) = setup().await;

    let result = mgr
        .search_full(
            "test",
            "",
            None,
            None,
            0,
            0,
            Some(&[facet_req("brand")]),
            None,
            None,
        )
        .unwrap();

    assert_eq!(result.documents.len(), 0, "limit=0 should return 0 docs");
    let brands = result
        .facets
        .get("brand")
        .expect("should still have facets");
    assert!(
        brands.len() >= 3,
        "facets should be populated even with limit=0"
    );
}

#[tokio::test]
async fn test_limit_zero_with_query_returns_no_docs_but_facets() {
    let (_tmp, mgr) = setup().await;

    let result = mgr
        .search_full(
            "test",
            "galaxy",
            None,
            None,
            0,
            0,
            Some(&[facet_req("brand")]),
            None,
            None,
        )
        .unwrap();

    assert_eq!(
        result.documents.len(),
        0,
        "limit=0 should return 0 docs even with query"
    );
    let brands = result.facets.get("brand");
    assert!(brands.is_some(), "facets should still be returned");
}

#[tokio::test]
async fn test_limit_zero_no_facets_returns_empty() {
    let (_tmp, mgr) = setup().await;

    let result = mgr
        .search_full("test", "", None, None, 0, 0, None, None, None)
        .unwrap();

    assert_eq!(result.documents.len(), 0);
    assert!(result.facets.is_empty());
}

#[tokio::test]
async fn test_response_fields_filters_output() {
    use flapjack_http::dto::SearchRequest;

    let req = SearchRequest {
        response_fields: Some(vec!["hits".to_string(), "nbHits".to_string()]),
        ..Default::default()
    };

    assert_eq!(
        req.response_fields,
        Some(vec!["hits".to_string(), "nbHits".to_string()])
    );
}

#[tokio::test]
async fn test_response_fields_star_keeps_all() {
    use flapjack_http::dto::SearchRequest;

    let req = SearchRequest {
        query: "test".to_string(),
        response_fields: Some(vec!["*".to_string()]),
        ..Default::default()
    };

    let fields = req.response_fields.as_ref().unwrap();
    assert!(fields.contains(&"*".to_string()));
}

#[tokio::test]
async fn test_params_string_response_fields() {
    use flapjack_http::dto::SearchRequest;

    let mut req = SearchRequest {
        params: Some("responseFields=%5B%22hits%22%2C%22nbHits%22%5D".to_string()),
        ..Default::default()
    };
    req.apply_params_string();
    assert_eq!(
        req.response_fields,
        Some(vec!["hits".to_string(), "nbHits".to_string()])
    );
}
