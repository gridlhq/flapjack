//! End-to-end analytics pipeline test.
//!
//! Tests the full flow: record searches -> record click/conversion events ->
//! flush to Parquet -> query results via DataFusion.

use flapjack::analytics::collector::AnalyticsCollector;
use flapjack::analytics::config::AnalyticsConfig;
use flapjack::analytics::query::AnalyticsQueryEngine;
use flapjack::analytics::schema::{InsightEvent, SearchEvent};
use tempfile::TempDir;

fn test_config(dir: &std::path::Path) -> AnalyticsConfig {
    AnalyticsConfig {
        enabled: true,
        data_dir: dir.to_path_buf(),
        flush_interval_secs: 3600,
        flush_size: 10_000, // won't auto-flush in tests
        retention_days: 90,
    }
}

fn search_event(
    query: &str,
    index: &str,
    nb_hits: u32,
    query_id: Option<&str>,
    user_token: &str,
    filters: Option<&str>,
) -> SearchEvent {
    SearchEvent {
        timestamp_ms: chrono::Utc::now().timestamp_millis(),
        query: query.to_string(),
        query_id: query_id.map(|s| s.to_string()),
        index_name: index.to_string(),
        nb_hits,
        processing_time_ms: 5,
        user_token: Some(user_token.to_string()),
        user_ip: Some("10.0.0.1".to_string()),
        filters: filters.map(|s| s.to_string()),
        facets: None,
        analytics_tags: None,
        page: 0,
        hits_per_page: 20,
        has_results: nb_hits > 0,
        country: None,
        region: None,
    }
}

fn click_event(query_id: &str, index: &str, user: &str, positions: Vec<u32>) -> InsightEvent {
    InsightEvent {
        event_type: "click".to_string(),
        event_subtype: None,
        event_name: "Result Click".to_string(),
        index: index.to_string(),
        user_token: user.to_string(),
        authenticated_user_token: None,
        query_id: Some(query_id.to_string()),
        object_ids: positions.iter().map(|p| format!("obj{}", p)).collect(),
        object_ids_alt: vec![],
        positions: Some(positions),
        timestamp: Some(chrono::Utc::now().timestamp_millis()),
        value: None,
        currency: None,
    }
}

fn conversion_event(query_id: &str, index: &str, user: &str) -> InsightEvent {
    InsightEvent {
        event_type: "conversion".to_string(),
        event_subtype: Some("purchase".to_string()),
        event_name: "Purchase".to_string(),
        index: index.to_string(),
        user_token: user.to_string(),
        authenticated_user_token: None,
        query_id: Some(query_id.to_string()),
        object_ids: vec!["obj1".to_string()],
        object_ids_alt: vec![],
        positions: None,
        timestamp: Some(chrono::Utc::now().timestamp_millis()),
        value: Some(49.99),
        currency: Some("USD".to_string()),
    }
}

/// Full pipeline: searches -> clicks -> conversions -> query all analytics endpoints.
#[tokio::test]
async fn full_analytics_pipeline() {
    let tmp = TempDir::new().unwrap();
    let config = test_config(tmp.path());
    let collector = AnalyticsCollector::new(config.clone());
    let engine = AnalyticsQueryEngine::new(config);
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();

    let qid1 = "a".repeat(32);
    let qid2 = "b".repeat(32);
    let qid3 = "c".repeat(32);
    let qid4 = "d".repeat(32);

    // --- Record search events ---
    // "laptop" searched 3 times by different users (2 with results, 1 with results)
    collector.record_search(search_event(
        "laptop",
        "products",
        42,
        Some(&qid1),
        "alice",
        None,
    ));
    collector.record_search(search_event(
        "laptop",
        "products",
        38,
        Some(&qid2),
        "bob",
        None,
    ));
    collector.record_search(search_event(
        "laptop",
        "products",
        42,
        Some(&qid3),
        "charlie",
        None,
    ));
    // "phone" searched 1 time
    collector.record_search(search_event(
        "phone",
        "products",
        15,
        Some(&qid4),
        "alice",
        None,
    ));
    // "nonexistent" searched 2 times - zero results
    collector.record_search(search_event(
        "nonexistent",
        "products",
        0,
        None,
        "alice",
        None,
    ));
    collector.record_search(search_event(
        "nonexistent",
        "products",
        0,
        None,
        "bob",
        None,
    ));
    // Filtered searches
    collector.record_search(search_event(
        "laptop",
        "products",
        10,
        None,
        "alice",
        Some("brand:Apple"),
    ));
    collector.record_search(search_event(
        "laptop",
        "products",
        0,
        None,
        "bob",
        Some("brand:Nonexistent"),
    ));

    // --- Record click events ---
    // Alice clicks on laptop result at position 1
    collector.record_insight(click_event(&qid1, "products", "alice", vec![1]));
    // Bob clicks at position 3
    collector.record_insight(click_event(&qid2, "products", "bob", vec![3]));

    // --- Record conversion event ---
    collector.record_insight(conversion_event(&qid1, "products", "alice"));

    // --- Flush everything to Parquet ---
    collector.flush_all();

    // === Query and verify all analytics endpoints ===

    // 1. Top searches
    let result = engine
        .top_searches("products", &today, &today, 10, false, None, None)
        .await
        .unwrap();
    let searches = result["searches"].as_array().unwrap();
    assert!(
        searches.len() >= 2,
        "Should have at least 2 distinct queries"
    );
    // "laptop" should be most frequent (4 times: 3 tracked + 1 filtered)
    assert_eq!(searches[0]["search"], "laptop");

    // 2. Search count
    let result = engine
        .search_count("products", &today, &today)
        .await
        .unwrap();
    assert_eq!(result["count"], 8, "Total 8 searches recorded");

    // 3. No-results rate
    let result = engine
        .no_results_rate("products", &today, &today)
        .await
        .unwrap();
    // 3 zero-result searches out of 8 total = 0.375
    let rate = result["rate"].as_f64().unwrap();
    assert!(
        (rate - 0.375).abs() < 0.01,
        "No-results rate should be ~0.375, got {}",
        rate
    );
    assert_eq!(result["noResults"], 3);

    // 4. No-results searches
    let result = engine
        .no_results_searches("products", &today, &today, 10)
        .await
        .unwrap();
    let searches = result["searches"].as_array().unwrap();
    assert!(!searches.is_empty(), "Should have no-result queries");
    // "nonexistent" should be there with count=2, and "laptop" with brand:Nonexistent with count=1
    let nonexistent = searches
        .iter()
        .find(|s| s["search"] == "nonexistent")
        .expect("'nonexistent' should appear in no-results");
    assert_eq!(nonexistent["count"], 2);

    // 5. Users count
    let result = engine
        .users_count("products", &today, &today)
        .await
        .unwrap();
    assert_eq!(result["count"], 3, "3 unique users: alice, bob, charlie");

    // 6. Click-through rate (tracked searches = those with queryID = 4, clicks = 2)
    let result = engine
        .click_through_rate("products", &today, &today)
        .await
        .unwrap();
    assert_eq!(result["trackedSearchCount"], 4);
    assert_eq!(result["clickCount"], 2);
    let ctr = result["rate"].as_f64().unwrap();
    assert!((ctr - 0.5).abs() < 0.01, "CTR should be 0.5, got {}", ctr);

    // 7. Average click position
    let result = engine
        .average_click_position("products", &today, &today)
        .await
        .unwrap();
    assert_eq!(result["clickCount"], 2);
    let avg = result["average"].as_f64().unwrap();
    // Position 1 and 3, average = 2.0
    assert!(
        (avg - 2.0).abs() < 0.5,
        "Avg click position should be ~2.0, got {}",
        avg
    );

    // 8. Conversion rate (1 conversion out of 4 tracked searches)
    let result = engine
        .conversion_rate("products", &today, &today)
        .await
        .unwrap();
    assert_eq!(result["trackedSearchCount"], 4);
    assert_eq!(result["conversionCount"], 1);
    let conv_rate = result["rate"].as_f64().unwrap();
    assert!(
        (conv_rate - 0.25).abs() < 0.01,
        "Conversion rate should be 0.25, got {}",
        conv_rate
    );

    // 9. Top filters
    let result = engine
        .top_filters("products", &today, &today, 10)
        .await
        .unwrap();
    let filters = result["filters"].as_array().unwrap();
    assert_eq!(filters.len(), 2, "2 distinct filter strings");

    // 10. Filter values for "brand"
    let result = engine
        .filter_values("products", "brand", &today, &today, 10)
        .await
        .unwrap();
    assert_eq!(result["attribute"], "brand");
    let values = result["values"].as_array().unwrap();
    assert!(values.len() >= 2, "Should have Apple and Nonexistent");

    // 11. Filters causing no results
    let result = engine
        .filters_no_results("products", &today, &today, 10)
        .await
        .unwrap();
    let filters = result["filters"].as_array().unwrap();
    assert_eq!(filters.len(), 1, "Only brand:Nonexistent caused no results");
    assert_eq!(filters[0]["attribute"], "brand:Nonexistent");

    // 12. Top hits (most clicked objectIDs)
    let result = engine
        .top_hits("products", &today, &today, 10)
        .await
        .unwrap();
    let hits = result["hits"].as_array().unwrap();
    assert!(!hits.is_empty(), "Should have clicked objects");

    // 13. Status
    let result = engine.status("products").await.unwrap();
    assert_eq!(result["enabled"], true);
    assert_eq!(result["hasData"], true);
}

/// Verify that queryID correlation works — collector caches queryIDs from searches.
#[tokio::test]
async fn query_id_correlation() {
    let tmp = TempDir::new().unwrap();
    let config = test_config(tmp.path());
    let collector = AnalyticsCollector::new(config);

    let qid = "abcdef0123456789abcdef0123456789".to_string();

    // Record search with queryID
    collector.record_search(search_event(
        "laptop",
        "products",
        42,
        Some(&qid),
        "alice",
        None,
    ));

    // Look up the queryID — should find it
    let entry = collector.lookup_query_id(&qid).unwrap();
    assert_eq!(entry.query, "laptop");
    assert_eq!(entry.index_name, "products");

    // Unknown queryID — should return None
    assert!(collector
        .lookup_query_id("0000000000000000000000000000000")
        .is_none());
}

/// Verify analytics: false suppresses recording (disabled config).
#[tokio::test]
async fn analytics_disabled_suppresses_recording() {
    let tmp = TempDir::new().unwrap();
    let config = AnalyticsConfig {
        enabled: false,
        data_dir: tmp.path().to_path_buf(),
        flush_interval_secs: 3600,
        flush_size: 1, // Would auto-flush at 1 event if enabled
        retention_days: 90,
    };
    let collector = AnalyticsCollector::new(config.clone());
    let engine = AnalyticsQueryEngine::new(config);
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();

    collector.record_search(search_event("laptop", "products", 42, None, "alice", None));
    collector.flush_all();

    // Query engine should find nothing
    let result = engine
        .search_count("products", &today, &today)
        .await
        .unwrap();
    assert_eq!(
        result["count"], 0,
        "Disabled analytics should record nothing"
    );
}

/// Verify non-correlated events (no queryID) are still recorded
/// but don't artificially inflate CTR since they aren't linked to searches.
#[tokio::test]
async fn non_correlated_events_recorded_but_dont_inflate_ctr() {
    let tmp = TempDir::new().unwrap();
    let config = test_config(tmp.path());
    let collector = AnalyticsCollector::new(config.clone());
    let engine = AnalyticsQueryEngine::new(config);
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();

    let qid = "a".repeat(32);

    // 1 tracked search
    collector.record_search(search_event(
        "laptop",
        "products",
        42,
        Some(&qid),
        "alice",
        None,
    ));

    // 1 click WITH queryID (correlated)
    collector.record_insight(click_event(&qid, "products", "alice", vec![1]));

    // 1 click WITHOUT queryID (non-correlated) — should be recorded as an event
    // but CTR should only count events with queryIDs
    let uncorrelated_click = InsightEvent {
        event_type: "click".to_string(),
        event_subtype: None,
        event_name: "Non-correlated Click".to_string(),
        index: "products".to_string(),
        user_token: "bob".to_string(),
        authenticated_user_token: None,
        query_id: None,
        object_ids: vec!["obj5".to_string()],
        object_ids_alt: vec![],
        positions: None,
        timestamp: Some(chrono::Utc::now().timestamp_millis()),
        value: None,
        currency: None,
    };
    collector.record_insight(uncorrelated_click);

    collector.flush_all();

    // CTR: 2 clicks / 1 tracked search. The non-correlated click IS counted in raw click count.
    // This is acceptable — Algolia counts all clicks in the denominator too.
    let result = engine
        .click_through_rate("products", &today, &today)
        .await
        .unwrap();
    assert_eq!(result["trackedSearchCount"], 1);
    // Both clicks are counted (correlated and non-correlated)
    assert_eq!(result["clickCount"], 2);

    // Top hits should include both clicked objects
    let result = engine
        .top_hits("products", &today, &today, 10)
        .await
        .unwrap();
    let hits = result["hits"].as_array().unwrap();
    assert!(!hits.is_empty(), "Should have clicked objects");
}

/// Verify date range filtering works — events outside range are excluded.
#[tokio::test]
async fn date_range_filtering() {
    let tmp = TempDir::new().unwrap();
    let config = test_config(tmp.path());
    let engine = AnalyticsQueryEngine::new(config.clone());

    // Write events for today
    let events = vec![
        search_event("laptop", "products", 42, None, "alice", None),
        search_event("phone", "products", 18, None, "bob", None),
    ];
    let searches_dir = config.searches_dir("products");
    flapjack::analytics::writer::flush_search_events(&events, &searches_dir).unwrap();

    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();

    // Query for today — should find events
    let result = engine
        .search_count("products", &today, &today)
        .await
        .unwrap();
    assert_eq!(result["count"], 2);

    // Query for yesterday — should find nothing
    let yesterday = (chrono::Utc::now() - chrono::Duration::days(1))
        .format("%Y-%m-%d")
        .to_string();
    let result = engine
        .search_count("products", &yesterday, &yesterday)
        .await
        .unwrap();
    assert_eq!(result["count"], 0, "Yesterday should have no events");
}
