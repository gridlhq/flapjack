//! Tests for AnalyticsCollector (collector.rs): buffering, flushing, queryID cache.

use flapjack::analytics::collector::AnalyticsCollector;
use flapjack::analytics::config::AnalyticsConfig;
use flapjack::analytics::schema::{InsightEvent, SearchEvent};
use tempfile::TempDir;

fn test_config(dir: &std::path::Path, flush_size: usize) -> AnalyticsConfig {
    AnalyticsConfig {
        enabled: true,
        data_dir: dir.to_path_buf(),
        flush_interval_secs: 3600, // won't trigger in tests
        flush_size,
        retention_days: 7,
    }
}

fn make_search(query: &str, index: &str, query_id: Option<&str>) -> SearchEvent {
    SearchEvent {
        timestamp_ms: chrono::Utc::now().timestamp_millis(),
        query: query.to_string(),
        query_id: query_id.map(|s| s.to_string()),
        index_name: index.to_string(),
        nb_hits: 10,
        processing_time_ms: 5,
        user_token: Some("user1".to_string()),
        user_ip: Some("127.0.0.1".to_string()),
        filters: None,
        facets: None,
        analytics_tags: None,
        page: 0,
        hits_per_page: 20,
        has_results: true,
        country: None,
        region: None,
    }
}

fn make_insight(event_type: &str, index: &str) -> InsightEvent {
    InsightEvent {
        event_type: event_type.to_string(),
        event_subtype: None,
        event_name: "Test".to_string(),
        index: index.to_string(),
        user_token: "user1".to_string(),
        authenticated_user_token: None,
        query_id: None,
        object_ids: vec!["obj1".to_string()],
        object_ids_alt: vec![],
        positions: None,
        timestamp: Some(chrono::Utc::now().timestamp_millis()),
        value: None,
        currency: None,
    }
}

#[test]
fn record_search_stores_in_buffer() {
    let tmp = TempDir::new().unwrap();
    let config = test_config(tmp.path(), 1000); // high threshold, won't auto-flush
    let collector = AnalyticsCollector::new(config);

    collector.record_search(make_search("laptop", "products", None));
    collector.record_search(make_search("phone", "products", None));

    // Force flush
    collector.flush_all();

    // Check files were written
    let searches_dir = tmp.path().join("products").join("searches");
    assert!(
        searches_dir.exists(),
        "Search parquet dir should exist after flush"
    );
}

#[test]
fn auto_flush_at_threshold() {
    let tmp = TempDir::new().unwrap();
    let config = test_config(tmp.path(), 3); // flush after 3 events
    let collector = AnalyticsCollector::new(config);

    // Record 3 events — should auto-flush
    collector.record_search(make_search("a", "products", None));
    collector.record_search(make_search("b", "products", None));
    collector.record_search(make_search("c", "products", None));

    // Files should already exist (auto-flushed at threshold)
    let searches_dir = tmp.path().join("products").join("searches");
    assert!(searches_dir.exists(), "Should auto-flush at threshold");
}

#[test]
fn disabled_collector_does_not_write() {
    let tmp = TempDir::new().unwrap();
    let config = AnalyticsConfig {
        enabled: false,
        data_dir: tmp.path().to_path_buf(),
        flush_interval_secs: 3600,
        flush_size: 1,
        retention_days: 7,
    };
    let collector = AnalyticsCollector::new(config);

    collector.record_search(make_search("laptop", "products", None));
    collector.record_insight(make_insight("click", "products"));
    collector.flush_all();

    // Nothing should be written
    let entries: Vec<_> = std::fs::read_dir(tmp.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .collect();
    assert_eq!(entries.len(), 0, "Disabled collector should write nothing");
}

#[test]
fn query_id_cache_stores_and_retrieves() {
    let tmp = TempDir::new().unwrap();
    let config = test_config(tmp.path(), 1000);
    let collector = AnalyticsCollector::new(config);

    let qid = "a".repeat(32);
    collector.record_search(make_search("laptop", "products", Some(&qid)));

    let entry = collector.lookup_query_id(&qid);
    assert!(entry.is_some(), "queryID should be in cache");
    let entry = entry.unwrap();
    assert_eq!(entry.query, "laptop");
    assert_eq!(entry.index_name, "products");
}

#[test]
fn query_id_cache_returns_none_for_unknown() {
    let tmp = TempDir::new().unwrap();
    let config = test_config(tmp.path(), 1000);
    let collector = AnalyticsCollector::new(config);

    assert!(collector.lookup_query_id("nonexistent").is_none());
}

#[test]
fn insight_events_flush_to_parquet() {
    let tmp = TempDir::new().unwrap();
    let config = test_config(tmp.path(), 1000);
    let collector = AnalyticsCollector::new(config);

    collector.record_insight(make_insight("click", "products"));
    collector.record_insight(make_insight("conversion", "products"));
    collector.flush_all();

    let events_dir = tmp.path().join("products").join("events");
    assert!(events_dir.exists(), "Events dir should exist after flush");
}

#[test]
fn events_grouped_by_index() {
    let tmp = TempDir::new().unwrap();
    let config = test_config(tmp.path(), 1000);
    let collector = AnalyticsCollector::new(config);

    collector.record_search(make_search("a", "index1", None));
    collector.record_search(make_search("b", "index2", None));
    collector.flush_all();

    assert!(tmp.path().join("index1").join("searches").exists());
    assert!(tmp.path().join("index2").join("searches").exists());
}

#[test]
fn flush_all_empties_both_buffers() {
    let tmp = TempDir::new().unwrap();
    let config = test_config(tmp.path(), 1000);
    let collector = AnalyticsCollector::new(config);

    collector.record_search(make_search("a", "products", None));
    collector.record_insight(make_insight("click", "products"));
    collector.flush_all();

    // Second flush should be a no-op (buffers empty)
    // Just verify it doesn't panic
    collector.flush_all();
}
