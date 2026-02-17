//! Tests for Parquet write + read roundtrip (writer.rs).

use flapjack::analytics::config::AnalyticsConfig;
use flapjack::analytics::query::AnalyticsQueryEngine;
use flapjack::analytics::schema::{InsightEvent, SearchEvent};
use flapjack::analytics::writer;
use tempfile::TempDir;

fn test_config(dir: &std::path::Path) -> AnalyticsConfig {
    AnalyticsConfig {
        enabled: true,
        data_dir: dir.to_path_buf(),
        flush_interval_secs: 1,
        flush_size: 100,
        retention_days: 7,
    }
}

fn make_search_event(query: &str, index: &str, nb_hits: u32) -> SearchEvent {
    SearchEvent {
        timestamp_ms: chrono::Utc::now().timestamp_millis(),
        query: query.to_string(),
        query_id: Some("a".repeat(32)),
        index_name: index.to_string(),
        nb_hits,
        processing_time_ms: 5,
        user_token: Some("user1".to_string()),
        user_ip: Some("192.168.1.1".to_string()),
        filters: None,
        facets: None,
        analytics_tags: None,
        page: 0,
        hits_per_page: 20,
        has_results: nb_hits > 0,
        country: None,
        region: None,
    }
}

fn make_insight_event(event_type: &str, index: &str, query_id: Option<&str>) -> InsightEvent {
    InsightEvent {
        event_type: event_type.to_string(),
        event_subtype: None,
        event_name: "Test Event".to_string(),
        index: index.to_string(),
        user_token: "user1".to_string(),
        authenticated_user_token: None,
        query_id: query_id.map(|s| s.to_string()),
        object_ids: vec!["obj1".to_string()],
        object_ids_alt: vec![],
        positions: if event_type == "click" && query_id.is_some() {
            Some(vec![3])
        } else {
            None
        },
        timestamp: Some(chrono::Utc::now().timestamp_millis()),
        value: None,
        currency: None,
    }
}

#[test]
fn write_search_events_creates_parquet_file() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path().join("searches");

    let events = vec![
        make_search_event("laptop", "products", 42),
        make_search_event("phone", "products", 18),
    ];

    writer::flush_search_events(&events, &dir).unwrap();

    // Check that the Hive-partitioned directory was created
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let partition_dir = dir.join(format!("date={}", today));
    assert!(partition_dir.exists(), "Partition dir should exist");

    // Check that a .parquet file was created
    let entries: Vec<_> = std::fs::read_dir(&partition_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .map(|ext| ext == "parquet")
                .unwrap_or(false)
        })
        .collect();
    assert_eq!(entries.len(), 1, "Should have 1 parquet file");
    assert!(
        entries[0].metadata().unwrap().len() > 0,
        "File should not be empty"
    );
}

#[test]
fn write_insight_events_creates_parquet_file() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path().join("events");

    let events = vec![
        make_insight_event("click", "products", Some(&"a".repeat(32))),
        make_insight_event("conversion", "products", Some(&"b".repeat(32))),
        make_insight_event("view", "products", None),
    ];

    writer::flush_insight_events(&events, &dir).unwrap();

    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let partition_dir = dir.join(format!("date={}", today));
    assert!(partition_dir.exists());

    let parquet_files: Vec<_> = std::fs::read_dir(&partition_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .map(|ext| ext == "parquet")
                .unwrap_or(false)
        })
        .collect();
    assert_eq!(parquet_files.len(), 1);
}

#[test]
fn empty_events_is_noop() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path().join("searches");

    writer::flush_search_events(&[], &dir).unwrap();
    assert!(!dir.exists(), "Dir should not be created for empty events");
}

#[tokio::test]
async fn search_events_roundtrip_via_query_engine() {
    let tmp = TempDir::new().unwrap();
    let config = test_config(tmp.path());
    let index_name = "products";

    // Write some search events
    let events = vec![
        make_search_event("laptop", index_name, 42),
        make_search_event("laptop", index_name, 35),
        make_search_event("phone", index_name, 18),
        make_search_event("nonexistent", index_name, 0),
    ];
    let searches_dir = config.searches_dir(index_name);
    writer::flush_search_events(&events, &searches_dir).unwrap();

    // Query them back
    let engine = AnalyticsQueryEngine::new(config);
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();

    let result = engine
        .top_searches(index_name, &today, &today, 10, false, None, None)
        .await
        .unwrap();

    let searches = result["searches"].as_array().unwrap();
    assert_eq!(searches.len(), 3, "Should have 3 distinct queries");

    // "laptop" should be first (count=2)
    assert_eq!(searches[0]["search"], "laptop");
    assert_eq!(searches[0]["count"], 2);
}

#[tokio::test]
async fn search_count_with_daily_breakdown() {
    let tmp = TempDir::new().unwrap();
    let config = test_config(tmp.path());
    let index_name = "products";

    let events = vec![
        make_search_event("a", index_name, 1),
        make_search_event("b", index_name, 2),
        make_search_event("c", index_name, 3),
    ];
    let searches_dir = config.searches_dir(index_name);
    writer::flush_search_events(&events, &searches_dir).unwrap();

    let engine = AnalyticsQueryEngine::new(config);
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();

    let result = engine
        .search_count(index_name, &today, &today)
        .await
        .unwrap();

    assert_eq!(result["count"], 3);
    let dates = result["dates"].as_array().unwrap();
    assert_eq!(dates.len(), 1, "All events on same day");
    assert_eq!(dates[0]["count"], 3);
    assert_eq!(dates[0]["date"], today);
}

#[tokio::test]
async fn no_results_rate_calculation() {
    let tmp = TempDir::new().unwrap();
    let config = test_config(tmp.path());
    let index_name = "products";

    let events = vec![
        make_search_event("found", index_name, 10),
        make_search_event("found2", index_name, 5),
        make_search_event("notfound1", index_name, 0),
        make_search_event("notfound2", index_name, 0),
    ];
    let searches_dir = config.searches_dir(index_name);
    writer::flush_search_events(&events, &searches_dir).unwrap();

    let engine = AnalyticsQueryEngine::new(config);
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();

    let result = engine
        .no_results_rate(index_name, &today, &today)
        .await
        .unwrap();

    // 2 out of 4 = 0.5
    assert_eq!(result["rate"], 0.5);
    assert_eq!(result["count"], 4);
    assert_eq!(result["noResults"], 2);
}

#[tokio::test]
async fn no_results_searches_returns_zero_hit_queries() {
    let tmp = TempDir::new().unwrap();
    let config = test_config(tmp.path());
    let index_name = "products";

    let events = vec![
        make_search_event("found", index_name, 10),
        make_search_event("missing", index_name, 0),
        make_search_event("missing", index_name, 0),
        make_search_event("also_missing", index_name, 0),
    ];
    let searches_dir = config.searches_dir(index_name);
    writer::flush_search_events(&events, &searches_dir).unwrap();

    let engine = AnalyticsQueryEngine::new(config);
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();

    let result = engine
        .no_results_searches(index_name, &today, &today, 10)
        .await
        .unwrap();

    let searches = result["searches"].as_array().unwrap();
    assert_eq!(searches.len(), 2); // "missing" and "also_missing"
                                   // "missing" has count=2, should be first
    assert_eq!(searches[0]["search"], "missing");
    assert_eq!(searches[0]["count"], 2);
}

#[tokio::test]
async fn users_count_deduplicates() {
    let tmp = TempDir::new().unwrap();
    let config = test_config(tmp.path());
    let index_name = "products";

    let mut e1 = make_search_event("a", index_name, 1);
    e1.user_token = Some("alice".to_string());
    let mut e2 = make_search_event("b", index_name, 2);
    e2.user_token = Some("bob".to_string());
    let mut e3 = make_search_event("c", index_name, 3);
    e3.user_token = Some("alice".to_string()); // same as e1

    let searches_dir = config.searches_dir(index_name);
    writer::flush_search_events(&[e1, e2, e3], &searches_dir).unwrap();

    let engine = AnalyticsQueryEngine::new(config);
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();

    let result = engine
        .users_count(index_name, &today, &today)
        .await
        .unwrap();

    assert_eq!(result["count"], 2, "Should have 2 unique users");
}

#[tokio::test]
async fn query_empty_index_returns_empty() {
    let tmp = TempDir::new().unwrap();
    let config = test_config(tmp.path());
    let engine = AnalyticsQueryEngine::new(config);
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();

    let result = engine
        .top_searches("nonexistent", &today, &today, 10, false, None, None)
        .await
        .unwrap();

    assert_eq!(result["searches"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn status_reflects_config() {
    let tmp = TempDir::new().unwrap();
    let config = test_config(tmp.path());
    let engine = AnalyticsQueryEngine::new(config);

    let result = engine.status("products").await.unwrap();
    assert_eq!(result["enabled"], true);
    assert_eq!(result["hasData"], false);
    assert_eq!(result["retentionDays"], 7);
}

#[tokio::test]
async fn filters_query_returns_filter_data() {
    let tmp = TempDir::new().unwrap();
    let config = test_config(tmp.path());
    let index_name = "products";

    let mut e1 = make_search_event("laptop", index_name, 10);
    e1.filters = Some("brand:Apple".to_string());
    let mut e2 = make_search_event("phone", index_name, 5);
    e2.filters = Some("brand:Samsung".to_string());
    let mut e3 = make_search_event("tablet", index_name, 3);
    e3.filters = Some("brand:Apple".to_string());

    let searches_dir = config.searches_dir(index_name);
    writer::flush_search_events(&[e1, e2, e3], &searches_dir).unwrap();

    let engine = AnalyticsQueryEngine::new(config);
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();

    let result = engine
        .top_filters(index_name, &today, &today, 10)
        .await
        .unwrap();

    let filters = result["filters"].as_array().unwrap();
    assert_eq!(filters.len(), 2);
    // "brand:Apple" appears twice, should be first
    assert_eq!(filters[0]["attribute"], "brand:Apple");
    assert_eq!(filters[0]["count"], 2);
}

#[tokio::test]
async fn filter_values_extracts_attribute_values() {
    let tmp = TempDir::new().unwrap();
    let config = test_config(tmp.path());
    let index_name = "products";

    let mut e1 = make_search_event("q1", index_name, 10);
    e1.filters = Some("brand:Apple".to_string());
    let mut e2 = make_search_event("q2", index_name, 5);
    e2.filters = Some("brand:Samsung".to_string());
    let mut e3 = make_search_event("q3", index_name, 3);
    e3.filters = Some("brand:Apple".to_string());

    let searches_dir = config.searches_dir(index_name);
    writer::flush_search_events(&[e1, e2, e3], &searches_dir).unwrap();

    let engine = AnalyticsQueryEngine::new(config);
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();

    let result = engine
        .filter_values(index_name, "brand", &today, &today, 10)
        .await
        .unwrap();

    assert_eq!(result["attribute"], "brand");
    let values = result["values"].as_array().unwrap();
    assert_eq!(values.len(), 2);
    assert_eq!(values[0]["value"], "Apple");
    assert_eq!(values[0]["count"], 2);
}
