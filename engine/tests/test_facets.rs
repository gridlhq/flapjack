//! Facet tests that depend on flapjack_http types (DTO, params parsing).
//!
//! The 43 pure-flapjack facet tests were moved to engine/src/integ_tests/test_facets.rs
//! so they run in-process via `cargo test --lib` (~1s) instead of nextest (~65s).
//! Only tests referencing flapjack_http remain here.
//!
//! NOTE: parse_facet_params tests live in flapjack-http/src/handlers/facets.rs
//! (unit test module). Do not duplicate them here.

// ============================================================
// DTO serialization tests
// ============================================================

#[test]
fn test_search_facet_values_request_deserialize() {
    let req: flapjack_http::dto::SearchFacetValuesRequest =
        serde_json::from_str(r#"{"facetQuery":"sam","maxFacetHits":5}"#).unwrap();
    assert_eq!(req.facet_query, "sam");
    assert_eq!(req.max_facet_hits, 5);
    assert!(req.filters.is_none());
}

#[test]
fn test_search_facet_values_request_defaults() {
    let req: flapjack_http::dto::SearchFacetValuesRequest =
        serde_json::from_str(r#"{"facetQuery":"test"}"#).unwrap();
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
    assert!(json.contains("processingTimeMS"));
    assert!(json.contains("exhaustiveFacetsCount"));
    assert!(json.contains("facetHits"));
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
// Params string / SearchRequest tests
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
async fn test_get_ranking_info_json_deserialize() {
    let req: flapjack_http::dto::SearchRequest =
        serde_json::from_str(r#"{"query":"test","getRankingInfo":true,"hitsPerPage":5}"#).unwrap();
    assert_eq!(req.get_ranking_info, Some(true));
    assert_eq!(req.query, "test");
}

#[tokio::test]
async fn test_response_fields_json_deserialize() {
    let req: flapjack_http::dto::SearchRequest =
        serde_json::from_str(r#"{"query":"test","responseFields":["hits","nbHits","facets"]}"#)
            .unwrap();
    assert_eq!(
        req.response_fields,
        Some(vec![
            "hits".to_string(),
            "nbHits".to_string(),
            "facets".to_string()
        ])
    );
}

// ============================================================
// Response fields (flapjack_http tests only)
// ============================================================

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
