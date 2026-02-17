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

async fn setup_facet_search_env(
    faceting_attrs: Vec<&str>,
) -> (TempDir, std::sync::Arc<IndexManager>) {
    let temp_dir = TempDir::new().unwrap();
    let manager = IndexManager::new(temp_dir.path());
    manager.create_tenant("test").unwrap();

    let settings = IndexSettings {
        attributes_for_faceting: faceting_attrs.iter().map(|s| s.to_string()).collect(),
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
                ("brand", text("Samsonite")),
                ("category", text("Luggage")),
                ("name", text("Carry-On")),
            ],
        ),
        doc(
            "4",
            vec![
                ("brand", text("Apple")),
                ("category", text("Phones")),
                ("name", text("iPhone 15")),
            ],
        ),
        doc(
            "5",
            vec![
                ("brand", text("Apple")),
                ("category", text("Laptops")),
                ("name", text("MacBook Pro")),
            ],
        ),
        doc(
            "6",
            vec![
                ("brand", text("Sony")),
                ("category", text("Audio")),
                ("name", text("WH-1000XM5")),
            ],
        ),
        doc(
            "7",
            vec![
                ("brand", text("Sony")),
                ("category", text("Phones")),
                ("name", text("Xperia")),
            ],
        ),
        doc(
            "8",
            vec![
                ("brand", text("Dell")),
                ("category", text("Laptops")),
                ("name", text("XPS 15")),
            ],
        ),
    ];

    manager.add_documents_sync("test", docs).await.unwrap();
    (temp_dir, manager)
}

// ============================================================
// searchable_facet_set() unit tests
// ============================================================

#[test]
fn test_searchable_facet_set_only_searchable_modifier() {
    let settings = IndexSettings {
        attributes_for_faceting: vec![
            "searchable(brand)".to_string(),
            "searchable(category)".to_string(),
        ],
        ..Default::default()
    };
    let set = settings.searchable_facet_set();
    assert!(set.contains("brand"));
    assert!(set.contains("category"));
    assert_eq!(set.len(), 2);
}

#[test]
fn test_searchable_facet_set_excludes_bare_names() {
    let settings = IndexSettings {
        attributes_for_faceting: vec!["brand".to_string(), "searchable(category)".to_string()],
        ..Default::default()
    };
    let set = settings.searchable_facet_set();
    assert!(
        !set.contains("brand"),
        "bare 'brand' must NOT be searchable"
    );
    assert!(set.contains("category"));
    assert_eq!(set.len(), 1);
}

#[test]
fn test_searchable_facet_set_excludes_filter_only() {
    let settings = IndexSettings {
        attributes_for_faceting: vec![
            "filterOnly(price)".to_string(),
            "searchable(brand)".to_string(),
        ],
        ..Default::default()
    };
    let set = settings.searchable_facet_set();
    assert!(!set.contains("price"), "filterOnly must NOT be searchable");
    assert!(set.contains("brand"));
    assert_eq!(set.len(), 1);
}

#[test]
fn test_searchable_facet_set_excludes_after_distinct() {
    let settings = IndexSettings {
        attributes_for_faceting: vec![
            "afterDistinct(status)".to_string(),
            "searchable(brand)".to_string(),
        ],
        ..Default::default()
    };
    let set = settings.searchable_facet_set();
    assert!(!set.contains("status"));
    assert!(set.contains("brand"));
}

#[test]
fn test_searchable_facet_set_empty_when_no_searchable() {
    let settings = IndexSettings {
        attributes_for_faceting: vec!["brand".to_string(), "filterOnly(price)".to_string()],
        ..Default::default()
    };
    let set = settings.searchable_facet_set();
    assert!(set.is_empty(), "No searchable() modifiers means empty set");
}

#[test]
fn test_facet_set_still_includes_all() {
    let settings = IndexSettings {
        attributes_for_faceting: vec![
            "brand".to_string(),
            "searchable(category)".to_string(),
            "filterOnly(price)".to_string(),
        ],
        ..Default::default()
    };
    let facet_set = settings.facet_set();
    assert!(facet_set.contains("brand"));
    assert!(facet_set.contains("category"));
    assert!(facet_set.contains("price"));
    assert_eq!(facet_set.len(), 3);
}

// ============================================================
// Facet search via manager (integration-level)
// ============================================================

#[tokio::test]
async fn test_facet_search_with_searchable_modifier() {
    let (_tmp, mgr) =
        setup_facet_search_env(vec!["searchable(brand)", "searchable(category)"]).await;

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
            Some(1000),
        )
        .unwrap();

    let brands = result.facets.get("brand").expect("brand facets");
    assert!(
        brands.len() >= 4,
        "Should have Samsung, Samsonite, Apple, Sony, Dell"
    );
    let samsung = brands.iter().find(|f| f.path == "Samsung");
    assert_eq!(samsung.map(|f| f.count), Some(2));
}

#[tokio::test]
async fn test_facet_search_prefix_filtering() {
    let (_tmp, mgr) = setup_facet_search_env(vec!["searchable(brand)"]).await;

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
            Some(1000),
        )
        .unwrap();

    let brands = result.facets.get("brand").expect("brand facets");
    let sam_matches: Vec<_> = brands
        .iter()
        .filter(|f| f.path.to_lowercase().starts_with("sam"))
        .collect();
    assert_eq!(sam_matches.len(), 2, "Samsung + Samsonite");
}

#[tokio::test]
async fn test_facet_search_empty_query_returns_all() {
    let (_tmp, mgr) = setup_facet_search_env(vec!["searchable(brand)"]).await;

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
            Some(1000),
        )
        .unwrap();

    let brands = result.facets.get("brand").expect("brand facets");
    assert!(
        brands.len() >= 4,
        "Empty query should return all brands, got {}",
        brands.len()
    );
}

#[tokio::test]
async fn test_facet_search_respects_max_values() {
    let (_tmp, mgr) = setup_facet_search_env(vec!["searchable(brand)"]).await;

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
            Some(2),
        )
        .unwrap();

    let brands = result.facets.get("brand").expect("brand facets");
    assert!(
        brands.len() <= 2,
        "maxValuesPerFacet=2 should cap results, got {}",
        brands.len()
    );
}

// ============================================================
// DTO / params string tests
// ============================================================

#[test]
fn test_search_facet_values_request_deserialize() {
    let json = r#"{"facetQuery":"sam","maxFacetHits":5}"#;
    let req: flapjack_http::dto::SearchFacetValuesRequest = serde_json::from_str(json).unwrap();
    assert_eq!(req.facet_query, "sam");
    assert_eq!(req.max_facet_hits, 5);
    assert!(req.filters.is_none());
}

#[test]
fn test_search_facet_values_request_defaults() {
    let json = r#"{"facetQuery":"test"}"#;
    let req: flapjack_http::dto::SearchFacetValuesRequest = serde_json::from_str(json).unwrap();
    assert_eq!(req.max_facet_hits, 10, "default maxFacetHits should be 10");
}

#[test]
fn test_search_facet_values_response_field_names() {
    let resp = flapjack_http::dto::SearchFacetValuesResponse {
        facet_hits: vec![],
        exhaustive_facets_count: true,
        processing_time_ms: 42,
    };
    let json = serde_json::to_string(&resp).unwrap();
    assert!(
        json.contains("processingTimeMS"),
        "Should serialize as processingTimeMS, got: {}",
        json
    );
    assert!(
        json.contains("exhaustiveFacetsCount"),
        "Should serialize as exhaustiveFacetsCount"
    );
    assert!(json.contains("facetHits"), "Should serialize as facetHits");
}

#[test]
fn test_facet_hit_serialization() {
    let hit = flapjack_http::dto::FacetHit {
        value: "Samsung".to_string(),
        highlighted: "<em>Sam</em>sung".to_string(),
        count: 633,
    };
    let json = serde_json::to_value(&hit).unwrap();
    assert_eq!(json["value"], "Samsung");
    assert_eq!(json["highlighted"], "<em>Sam</em>sung");
    assert_eq!(json["count"], 633);
}
// ============================================================
// params string parsing tests (HTTP handler parity)
// ============================================================

#[test]
fn test_parse_facet_params_basic() {
    use flapjack_http::handlers::parse_facet_params;
    let req = parse_facet_params("facetQuery=sam&maxFacetHits=5");
    assert_eq!(req.facet_query, "sam");
    assert_eq!(req.max_facet_hits, 5);
    assert!(req.filters.is_none());
}

#[test]
fn test_parse_facet_params_with_filters() {
    use flapjack_http::handlers::parse_facet_params;
    let req = parse_facet_params("facetQuery=sam&filters=brand%3ASamsung&maxFacetHits=3");
    assert_eq!(req.facet_query, "sam");
    assert_eq!(req.filters, Some("brand:Samsung".to_string()));
    assert_eq!(req.max_facet_hits, 3);
}

#[test]
fn test_parse_facet_params_empty_query() {
    use flapjack_http::handlers::parse_facet_params;
    let req = parse_facet_params("facetQuery=&maxFacetHits=10");
    assert_eq!(req.facet_query, "");
    assert_eq!(req.max_facet_hits, 10);
}

#[test]
fn test_parse_facet_params_defaults() {
    use flapjack_http::handlers::parse_facet_params;
    let req = parse_facet_params("facetQuery=test");
    assert_eq!(req.facet_query, "test");
    assert_eq!(req.max_facet_hits, 10, "default maxFacetHits should be 10");
    assert!(req.filters.is_none());
}

#[test]
fn test_parse_facet_params_empty_string() {
    use flapjack_http::handlers::parse_facet_params;
    let req = parse_facet_params("");
    assert_eq!(req.facet_query, "");
    assert_eq!(req.max_facet_hits, 10);
    assert!(req.filters.is_none());
}

#[tokio::test]
async fn test_facet_search_params_string_integration() {
    let (_tmp, mgr) = setup_facet_search_env(vec!["searchable(brand)"]).await;
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
            Some(1000),
        )
        .unwrap();
    let brands = result.facets.get("brand").expect("brand facets");
    let query = "sam";
    let matching: Vec<_> = brands
        .iter()
        .filter(|f| f.path.to_lowercase().starts_with(query))
        .collect();
    assert_eq!(
        matching.len(),
        2,
        "Samsung + Samsonite should match 'sam' prefix"
    );
    assert!(matching.iter().any(|f| f.path == "Samsung"));
    assert!(matching.iter().any(|f| f.path == "Samsonite"));
}

#[tokio::test]
async fn test_facet_search_rejects_non_searchable() {
    let (_tmp, _mgr) = setup_facet_search_env(vec!["filterOnly(brand)"]).await;
    let settings_path = _tmp.path().join("test/settings.json");
    let settings = IndexSettings::load(&settings_path).unwrap();
    let searchable = settings.searchable_facet_set();
    assert!(
        !searchable.contains("brand"),
        "filterOnly(brand) should not be in searchable set"
    );
}

#[tokio::test]
async fn test_facet_search_sorted_by_count_desc() {
    let (_tmp, mgr) = setup_facet_search_env(vec!["searchable(brand)"]).await;
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
            Some(1000),
        )
        .unwrap();
    let brands = result.facets.get("brand").expect("brand facets");
    let counts: Vec<u64> = brands.iter().map(|f| f.count).collect();
    for w in counts.windows(2) {
        assert!(
            w[0] >= w[1],
            "Facet values should be sorted by count desc, got {:?}",
            counts
        );
    }
}
#[tokio::test]
async fn test_facet_json_serialization_preserves_count_order() {
    let (_tmp, mgr) = setup_facet_search_env(vec!["searchable(brand)"]).await;
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
            Some(1000),
        )
        .unwrap();
    let brands = result.facets.get("brand").expect("brand facets");
    let facet_map: serde_json::Map<String, serde_json::Value> = brands
        .iter()
        .map(|fc| (fc.path.clone(), serde_json::json!(fc.count)))
        .collect();
    let json_obj = serde_json::Value::Object(facet_map);
    let serialized = serde_json::to_string(&json_obj).unwrap();
    let roundtrip: serde_json::Map<String, serde_json::Value> =
        serde_json::from_str(&serialized).unwrap();
    let counts: Vec<u64> = roundtrip.values().map(|v| v.as_u64().unwrap()).collect();
    for w in counts.windows(2) {
        assert!(
            w[0] >= w[1],
            "Facet order lost during JSON serialization: {:?}",
            counts
        );
    }
}
