//! Tests for Priority 1 deferred items from handoff #136/#141
//!
//! 1. maxValuesPerFacet enforcement (settings + per-query override)
//! 2. getRankingInfo doesn't crash
//! 3. highlightPreTag/PostTag via params string
//! 4. responseFields deserialization

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

async fn setup_many_brands(n: usize) -> (TempDir, std::sync::Arc<IndexManager>) {
    let temp_dir = TempDir::new().unwrap();
    let manager = IndexManager::new(temp_dir.path());
    manager.create_tenant("test").unwrap();

    let settings = IndexSettings {
        attributes_for_faceting: vec!["brand".to_string()],
        max_values_per_facet: 10,
        ..Default::default()
    };
    settings
        .save(temp_dir.path().join("test/settings.json"))
        .unwrap();

    let docs: Vec<Document> = (0..n)
        .map(|i| {
            doc(
                &format!("{}", i),
                vec![
                    ("brand", text(&format!("Brand_{:04}", i))),
                    ("name", text(&format!("Product {}", i))),
                ],
            )
        })
        .collect();

    manager.add_documents_sync("test", docs).await.unwrap();
    (temp_dir, manager)
}

// ============================================================
// maxValuesPerFacet
// ============================================================

#[tokio::test]
async fn test_max_values_per_facet_settings_enforced() {
    let (_tmp, mgr) = setup_many_brands(50).await;

    let result = mgr
        .search_with_facets("test", "", None, None, 0, 0, Some(&[facet_req("brand")]))
        .unwrap();

    let brands = result.facets.get("brand").expect("brand facets");
    assert_eq!(
        brands.len(),
        10,
        "Settings maxValuesPerFacet=10 should limit to 10, got {}",
        brands.len()
    );
}

#[tokio::test]
async fn test_max_values_per_facet_query_override() {
    let (_tmp, mgr) = setup_many_brands(50).await;

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
            Some(5),
        )
        .unwrap();

    let brands = result.facets.get("brand").expect("brand facets");
    assert_eq!(
        brands.len(),
        5,
        "Per-query maxValuesPerFacet=5 should override settings, got {}",
        brands.len()
    );
}

#[tokio::test]
async fn test_max_values_per_facet_query_override_higher() {
    let (_tmp, mgr) = setup_many_brands(50).await;

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
            Some(25),
        )
        .unwrap();

    let brands = result.facets.get("brand").expect("brand facets");
    assert_eq!(
        brands.len(),
        25,
        "Per-query maxValuesPerFacet=25 should override settings=10, got {}",
        brands.len()
    );
}

#[tokio::test]
async fn test_max_values_per_facet_capped_at_1000() {
    let (_tmp, mgr) = setup_many_brands(50).await;

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
            Some(9999),
        )
        .unwrap();

    let brands = result.facets.get("brand").expect("brand facets");
    assert_eq!(
        brands.len(),
        50,
        "Cap at 1000 but only 50 exist, should return 50, got {}",
        brands.len()
    );
}

#[tokio::test]
async fn test_max_values_default_100_when_no_settings() {
    let temp_dir = TempDir::new().unwrap();
    let manager = IndexManager::new(temp_dir.path());
    manager.create_tenant("test").unwrap();

    let docs: Vec<Document> = (0..150)
        .map(|i| {
            doc(
                &format!("{}", i),
                vec![
                    ("brand", text(&format!("Brand_{:04}", i))),
                    ("name", text(&format!("Product {}", i))),
                ],
            )
        })
        .collect();

    let settings = IndexSettings {
        attributes_for_faceting: vec!["brand".to_string()],
        ..Default::default()
    };
    settings
        .save(temp_dir.path().join("test/settings.json"))
        .unwrap();
    manager.add_documents_sync("test", docs).await.unwrap();

    let result = manager
        .search_with_facets("test", "", None, None, 0, 0, Some(&[facet_req("brand")]))
        .unwrap();

    let brands = result.facets.get("brand").expect("brand facets");
    assert_eq!(
        brands.len(),
        100,
        "Default maxValuesPerFacet should be 100, got {}",
        brands.len()
    );
}

// ============================================================
// Params string: highlightPreTag/PostTag, getRankingInfo
// ============================================================

#[tokio::test]
async fn test_params_string_highlight_tags() {
    use flapjack_http::dto::SearchRequest;

    let mut req = SearchRequest {
        query: "test".to_string(),
        params: Some("highlightPreTag=%3Cb%3E&highlightPostTag=%3C%2Fb%3E".to_string()),
        ..Default::default()
    };
    req.apply_params_string();
    assert_eq!(req.highlight_pre_tag.as_deref(), Some("<b>"));
    assert_eq!(req.highlight_post_tag.as_deref(), Some("</b>"));
}

#[tokio::test]
async fn test_params_string_get_ranking_info() {
    use flapjack_http::dto::SearchRequest;

    let mut req = SearchRequest {
        params: Some("getRankingInfo=true".to_string()),
        ..Default::default()
    };
    req.apply_params_string();
    assert_eq!(req.get_ranking_info, Some(true));
}

#[tokio::test]
async fn test_params_string_max_values_per_facet() {
    use flapjack_http::dto::SearchRequest;

    let mut req = SearchRequest {
        params: Some("maxValuesPerFacet=15&facets=%5B%22brand%22%5D".to_string()),
        ..Default::default()
    };
    req.apply_params_string();
    assert_eq!(req.max_values_per_facet, Some(15));
    assert_eq!(req.facets, Some(vec!["brand".to_string()]));
}

#[tokio::test]
async fn test_max_values_per_facet_missing_from_json_defaults_100() {
    let json = r#"{"attributesForFaceting":["brand"]}"#;
    let settings: IndexSettings = serde_json::from_str(json).unwrap();
    assert_eq!(
        settings.max_values_per_facet, 100,
        "Missing maxValuesPerFacet should default to 100 (Algolia parity)"
    );
}

#[tokio::test]
async fn test_get_ranking_info_json_deserialize() {
    let json = r#"{"query":"test","getRankingInfo":true,"hitsPerPage":5}"#;
    let req: flapjack_http::dto::SearchRequest = serde_json::from_str(json).unwrap();
    assert_eq!(req.get_ranking_info, Some(true));
    assert_eq!(req.query, "test");
}

#[tokio::test]
async fn test_response_fields_json_deserialize() {
    let json = r#"{"query":"test","responseFields":["hits","nbHits","facets"]}"#;
    let req: flapjack_http::dto::SearchRequest = serde_json::from_str(json).unwrap();
    assert_eq!(
        req.response_fields,
        Some(vec![
            "hits".to_string(),
            "nbHits".to_string(),
            "facets".to_string()
        ])
    );
}
