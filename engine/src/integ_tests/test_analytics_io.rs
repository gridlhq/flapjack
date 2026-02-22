//! Consolidated analytics I/O tests — collector, writer, and cleanup.
//!
//! Covers:
//!   - AnalyticsCollector (collector.rs): buffering, flushing, queryID cache
//!   - writer.rs: Parquet write + read roundtrip via AnalyticsQueryEngine
//!   - cleanup logic: orphaned analytics directories

use crate::analytics::collector::AnalyticsCollector;
use crate::analytics::config::AnalyticsConfig;
use crate::analytics::query::AnalyticsQueryEngine;
use crate::analytics::schema::{InsightEvent, SearchEvent};
use crate::analytics::writer;
use std::collections::HashSet;
use tempfile::TempDir;

// ─── Config helpers ───────────────────────────────────────────────────────────

fn collector_config(dir: &std::path::Path, flush_size: usize) -> AnalyticsConfig {
    AnalyticsConfig {
        enabled: true,
        data_dir: dir.to_path_buf(),
        flush_interval_secs: 3600,
        flush_size,
        retention_days: 7,
    }
}

fn writer_config(dir: &std::path::Path) -> AnalyticsConfig {
    AnalyticsConfig {
        enabled: true,
        data_dir: dir.to_path_buf(),
        flush_interval_secs: 1,
        flush_size: 100,
        retention_days: 7,
    }
}

fn cleanup_config(dir: &std::path::Path) -> AnalyticsConfig {
    AnalyticsConfig {
        enabled: true,
        data_dir: dir.to_path_buf(),
        flush_interval_secs: 3600,
        flush_size: 10_000,
        retention_days: 90,
    }
}

// ─── Event builders ───────────────────────────────────────────────────────────

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
        experiment_id: None,
        variant_id: None,
        assignment_method: None,
    }
}

fn make_search_ev(query: &str, index: &str, nb_hits: u32) -> SearchEvent {
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
        experiment_id: None,
        variant_id: None,
        assignment_method: None,
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
        interleaving_team: None,
    }
}

fn make_insight_ev(event_type: &str, index: &str, query_id: Option<&str>) -> InsightEvent {
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
        interleaving_team: None,
    }
}

// ─── Cleanup helper ───────────────────────────────────────────────────────────

fn run_cleanup(engine: &AnalyticsQueryEngine, active_index_dir: &std::path::Path) -> Vec<String> {
    let analytics_indices = engine.list_analytics_indices().unwrap();
    let mut active_indices: HashSet<String> = HashSet::new();
    if active_index_dir.exists() {
        if let Ok(entries) = std::fs::read_dir(active_index_dir) {
            for entry in entries.flatten() {
                if entry.path().is_dir() {
                    if let Some(name) = entry.file_name().to_str() {
                        active_indices.insert(name.to_string());
                    }
                }
            }
        }
    }
    let orphaned: Vec<String> = analytics_indices
        .into_iter()
        .filter(|name| !active_indices.contains(name))
        .collect();
    let config = engine.config();
    for index_name in &orphaned {
        let index_dir = config.data_dir.join(index_name);
        if index_dir.exists() {
            let _ = std::fs::remove_dir_all(&index_dir);
        }
    }
    orphaned
}

// ─── AnalyticsCollector tests ──────────────────────────────────────────────────

#[test]
fn record_search_stores_in_buffer() {
    let tmp = TempDir::new().unwrap();
    let collector = AnalyticsCollector::new(collector_config(tmp.path(), 1000));
    collector.record_search(make_search("laptop", "products", None));
    collector.record_search(make_search("phone", "products", None));
    collector.flush_all();
    assert!(tmp.path().join("products").join("searches").exists());
}

#[test]
fn auto_flush_at_threshold() {
    let tmp = TempDir::new().unwrap();
    let collector = AnalyticsCollector::new(collector_config(tmp.path(), 3));
    collector.record_search(make_search("a", "products", None));
    collector.record_search(make_search("b", "products", None));
    collector.record_search(make_search("c", "products", None));
    assert!(tmp.path().join("products").join("searches").exists());
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
    let entries: Vec<_> = std::fs::read_dir(tmp.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .collect();
    assert_eq!(entries.len(), 0);
}

#[test]
fn query_id_cache_stores_and_retrieves() {
    let tmp = TempDir::new().unwrap();
    let collector = AnalyticsCollector::new(collector_config(tmp.path(), 1000));
    let qid = "a".repeat(32);
    collector.record_search(make_search("laptop", "products", Some(&qid)));
    let entry = collector.lookup_query_id(&qid).unwrap();
    assert_eq!(entry.query, "laptop");
    assert_eq!(entry.index_name, "products");
}

#[test]
fn query_id_cache_returns_none_for_unknown() {
    let tmp = TempDir::new().unwrap();
    let collector = AnalyticsCollector::new(collector_config(tmp.path(), 1000));
    assert!(collector.lookup_query_id("nonexistent").is_none());
}

#[test]
fn insight_events_flush_to_parquet() {
    let tmp = TempDir::new().unwrap();
    let collector = AnalyticsCollector::new(collector_config(tmp.path(), 1000));
    collector.record_insight(make_insight("click", "products"));
    collector.record_insight(make_insight("conversion", "products"));
    collector.flush_all();
    assert!(tmp.path().join("products").join("events").exists());
}

#[test]
fn events_grouped_by_index() {
    let tmp = TempDir::new().unwrap();
    let collector = AnalyticsCollector::new(collector_config(tmp.path(), 1000));
    collector.record_search(make_search("a", "index1", None));
    collector.record_search(make_search("b", "index2", None));
    collector.flush_all();
    assert!(tmp.path().join("index1").join("searches").exists());
    assert!(tmp.path().join("index2").join("searches").exists());
}

#[test]
fn flush_all_empties_both_buffers() {
    let tmp = TempDir::new().unwrap();
    let collector = AnalyticsCollector::new(collector_config(tmp.path(), 1000));
    collector.record_search(make_search("a", "products", None));
    collector.record_insight(make_insight("click", "products"));
    collector.flush_all();
    collector.flush_all(); // should not panic
}

// ─── Writer / AnalyticsQueryEngine tests ──────────────────────────────────────

#[test]
fn write_search_events_creates_parquet_file() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path().join("searches");
    let events = vec![
        make_search_ev("laptop", "products", 42),
        make_search_ev("phone", "products", 18),
    ];
    writer::flush_search_events(&events, &dir).unwrap();
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let partition_dir = dir.join(format!("date={}", today));
    assert!(partition_dir.exists());
    let parquet_count = std::fs::read_dir(&partition_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .map(|x| x == "parquet")
                .unwrap_or(false)
        })
        .count();
    assert_eq!(parquet_count, 1);
}

#[test]
fn write_insight_events_creates_parquet_file() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path().join("events");
    let events = vec![
        make_insight_ev("click", "products", Some(&"a".repeat(32))),
        make_insight_ev("conversion", "products", Some(&"b".repeat(32))),
        make_insight_ev("view", "products", None),
    ];
    writer::flush_insight_events(&events, &dir).unwrap();
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    assert!(dir.join(format!("date={}", today)).exists());
}

#[test]
fn empty_events_is_noop() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path().join("searches");
    writer::flush_search_events(&[], &dir).unwrap();
    assert!(!dir.exists());
}

#[tokio::test]
async fn search_events_roundtrip_via_query_engine() {
    let tmp = TempDir::new().unwrap();
    let config = writer_config(tmp.path());
    let events = vec![
        make_search_ev("laptop", "products", 42),
        make_search_ev("laptop", "products", 35),
        make_search_ev("phone", "products", 18),
        make_search_ev("nonexistent", "products", 0),
    ];
    let searches_dir = config.searches_dir("products");
    writer::flush_search_events(&events, &searches_dir).unwrap();
    let engine = AnalyticsQueryEngine::new(config);
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let result = engine
        .top_searches("products", &today, &today, 10, false, None, None)
        .await
        .unwrap();
    let searches = result["searches"].as_array().unwrap();
    assert_eq!(searches.len(), 3);
    assert_eq!(searches[0]["search"], "laptop");
    assert_eq!(searches[0]["count"], 2);
}

#[tokio::test]
async fn search_count_with_daily_breakdown() {
    let tmp = TempDir::new().unwrap();
    let config = writer_config(tmp.path());
    let events = vec![
        make_search_ev("a", "products", 1),
        make_search_ev("b", "products", 2),
        make_search_ev("c", "products", 3),
    ];
    let searches_dir = config.searches_dir("products");
    writer::flush_search_events(&events, &searches_dir).unwrap();
    let engine = AnalyticsQueryEngine::new(config);
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let result = engine
        .search_count("products", &today, &today)
        .await
        .unwrap();
    assert_eq!(result["count"], 3);
    let dates = result["dates"].as_array().unwrap();
    assert_eq!(dates.len(), 1);
    assert_eq!(dates[0]["count"], 3);
}

#[tokio::test]
async fn no_results_rate_calculation() {
    let tmp = TempDir::new().unwrap();
    let config = writer_config(tmp.path());
    let events = vec![
        make_search_ev("found", "products", 10),
        make_search_ev("found2", "products", 5),
        make_search_ev("notfound1", "products", 0),
        make_search_ev("notfound2", "products", 0),
    ];
    let searches_dir = config.searches_dir("products");
    writer::flush_search_events(&events, &searches_dir).unwrap();
    let engine = AnalyticsQueryEngine::new(config);
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let result = engine
        .no_results_rate("products", &today, &today)
        .await
        .unwrap();
    assert_eq!(result["rate"], 0.5);
    assert_eq!(result["count"], 4);
    assert_eq!(result["noResults"], 2);
}

#[tokio::test]
async fn no_results_searches_returns_zero_hit_queries() {
    let tmp = TempDir::new().unwrap();
    let config = writer_config(tmp.path());
    let events = vec![
        make_search_ev("found", "products", 10),
        make_search_ev("missing", "products", 0),
        make_search_ev("missing", "products", 0),
        make_search_ev("also_missing", "products", 0),
    ];
    let searches_dir = config.searches_dir("products");
    writer::flush_search_events(&events, &searches_dir).unwrap();
    let engine = AnalyticsQueryEngine::new(config);
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let result = engine
        .no_results_searches("products", &today, &today, 10)
        .await
        .unwrap();
    let searches = result["searches"].as_array().unwrap();
    assert_eq!(searches.len(), 2);
    assert_eq!(searches[0]["search"], "missing");
    assert_eq!(searches[0]["count"], 2);
}

#[tokio::test]
async fn users_count_deduplicates() {
    let tmp = TempDir::new().unwrap();
    let config = writer_config(tmp.path());
    let mut e1 = make_search_ev("a", "products", 1);
    e1.user_token = Some("alice".to_string());
    let mut e2 = make_search_ev("b", "products", 2);
    e2.user_token = Some("bob".to_string());
    let mut e3 = make_search_ev("c", "products", 3);
    e3.user_token = Some("alice".to_string());
    let searches_dir = config.searches_dir("products");
    writer::flush_search_events(&[e1, e2, e3], &searches_dir).unwrap();
    let engine = AnalyticsQueryEngine::new(config);
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let result = engine
        .users_count("products", &today, &today)
        .await
        .unwrap();
    assert_eq!(result["count"], 2);
}

#[tokio::test]
async fn query_empty_index_returns_empty() {
    let tmp = TempDir::new().unwrap();
    let config = writer_config(tmp.path());
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
    let config = writer_config(tmp.path());
    let engine = AnalyticsQueryEngine::new(config);
    let result = engine.status("products").await.unwrap();
    assert_eq!(result["enabled"], true);
    assert_eq!(result["hasData"], false);
    assert_eq!(result["retentionDays"], 7);
}

#[tokio::test]
async fn filters_query_returns_filter_data() {
    let tmp = TempDir::new().unwrap();
    let config = writer_config(tmp.path());
    let mut e1 = make_search_ev("laptop", "products", 10);
    e1.filters = Some("brand:Apple".to_string());
    let mut e2 = make_search_ev("phone", "products", 5);
    e2.filters = Some("brand:Samsung".to_string());
    let mut e3 = make_search_ev("tablet", "products", 3);
    e3.filters = Some("brand:Apple".to_string());
    let searches_dir = config.searches_dir("products");
    writer::flush_search_events(&[e1, e2, e3], &searches_dir).unwrap();
    let engine = AnalyticsQueryEngine::new(config);
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let result = engine
        .top_filters("products", &today, &today, 10)
        .await
        .unwrap();
    let filters = result["filters"].as_array().unwrap();
    assert_eq!(filters.len(), 2);
    assert_eq!(filters[0]["attribute"], "brand:Apple");
    assert_eq!(filters[0]["count"], 2);
}

#[tokio::test]
async fn filter_values_extracts_attribute_values() {
    let tmp = TempDir::new().unwrap();
    let config = writer_config(tmp.path());
    let mut e1 = make_search_ev("q1", "products", 10);
    e1.filters = Some("brand:Apple".to_string());
    let mut e2 = make_search_ev("q2", "products", 5);
    e2.filters = Some("brand:Samsung".to_string());
    let mut e3 = make_search_ev("q3", "products", 3);
    e3.filters = Some("brand:Apple".to_string());
    let searches_dir = config.searches_dir("products");
    writer::flush_search_events(&[e1, e2, e3], &searches_dir).unwrap();
    let engine = AnalyticsQueryEngine::new(config);
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let result = engine
        .filter_values("products", "brand", &today, &today, 10)
        .await
        .unwrap();
    assert_eq!(result["attribute"], "brand");
    let values = result["values"].as_array().unwrap();
    assert_eq!(values.len(), 2);
    assert_eq!(values[0]["value"], "Apple");
    assert_eq!(values[0]["count"], 2);
}

// ─── Experiment fields writer roundtrip tests ──────────────────────────────────

#[test]
fn writer_roundtrip_with_experiment_fields() {
    use crate::analytics::schema::search_event_schema;
    use arrow::array::Array;
    use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
    use std::fs::File;

    let tmp = TempDir::new().unwrap();
    let dir = tmp.path().join("searches");
    let event = SearchEvent {
        timestamp_ms: 1000,
        query: "laptop".to_string(),
        query_id: Some("a".repeat(32)),
        index_name: "products".to_string(),
        nb_hits: 42,
        processing_time_ms: 5,
        user_token: Some("user1".to_string()),
        user_ip: None,
        filters: None,
        facets: None,
        analytics_tags: None,
        page: 0,
        hits_per_page: 20,
        has_results: true,
        country: None,
        region: None,
        experiment_id: Some("exp-1".to_string()),
        variant_id: Some("variant".to_string()),
        assignment_method: Some("user_token".to_string()),
    };
    writer::flush_search_events(&[event], &dir).unwrap();

    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let partition_dir = dir.join(format!("date={}", today));
    let parquet_file = std::fs::read_dir(&partition_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .find(|e| {
            e.path()
                .extension()
                .map(|x| x == "parquet")
                .unwrap_or(false)
        })
        .unwrap();
    let file = File::open(parquet_file.path()).unwrap();
    let reader = ParquetRecordBatchReaderBuilder::try_new(file)
        .unwrap()
        .build()
        .unwrap();
    let batches: Vec<_> = reader.collect::<Result<_, _>>().unwrap();
    assert_eq!(batches.len(), 1);
    let batch = &batches[0];

    let schema = search_event_schema();
    let exp_idx = schema.index_of("experiment_id").unwrap();
    let var_idx = schema.index_of("variant_id").unwrap();
    let method_idx = schema.index_of("assignment_method").unwrap();

    let exp_col = batch
        .column(exp_idx)
        .as_any()
        .downcast_ref::<arrow::array::StringArray>()
        .unwrap();
    assert_eq!(exp_col.value(0), "exp-1");

    let var_col = batch
        .column(var_idx)
        .as_any()
        .downcast_ref::<arrow::array::StringArray>()
        .unwrap();
    assert_eq!(var_col.value(0), "variant");

    let method_col = batch
        .column(method_idx)
        .as_any()
        .downcast_ref::<arrow::array::StringArray>()
        .unwrap();
    assert_eq!(method_col.value(0), "user_token");
}

#[test]
fn writer_roundtrip_with_null_experiment_fields() {
    use crate::analytics::schema::search_event_schema;
    use arrow::array::Array;
    use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
    use std::fs::File;

    let tmp = TempDir::new().unwrap();
    let dir = tmp.path().join("searches");
    let event = SearchEvent {
        timestamp_ms: 1000,
        query: "laptop".to_string(),
        query_id: None,
        index_name: "products".to_string(),
        nb_hits: 10,
        processing_time_ms: 3,
        user_token: None,
        user_ip: None,
        filters: None,
        facets: None,
        analytics_tags: None,
        page: 0,
        hits_per_page: 20,
        has_results: true,
        country: None,
        region: None,
        experiment_id: None,
        variant_id: None,
        assignment_method: None,
    };
    writer::flush_search_events(&[event], &dir).unwrap();

    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let partition_dir = dir.join(format!("date={}", today));
    let parquet_file = std::fs::read_dir(&partition_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .find(|e| {
            e.path()
                .extension()
                .map(|x| x == "parquet")
                .unwrap_or(false)
        })
        .unwrap();
    let file = File::open(parquet_file.path()).unwrap();
    let reader = ParquetRecordBatchReaderBuilder::try_new(file)
        .unwrap()
        .build()
        .unwrap();
    let batches: Vec<_> = reader.collect::<Result<_, _>>().unwrap();
    assert_eq!(batches.len(), 1);
    let batch = &batches[0];

    let schema = search_event_schema();
    let exp_idx = schema.index_of("experiment_id").unwrap();
    let var_idx = schema.index_of("variant_id").unwrap();
    let method_idx = schema.index_of("assignment_method").unwrap();

    let exp_col = batch
        .column(exp_idx)
        .as_any()
        .downcast_ref::<arrow::array::StringArray>()
        .unwrap();
    assert!(exp_col.is_null(0));

    let var_col = batch
        .column(var_idx)
        .as_any()
        .downcast_ref::<arrow::array::StringArray>()
        .unwrap();
    assert!(var_col.is_null(0));

    let method_col = batch
        .column(method_idx)
        .as_any()
        .downcast_ref::<arrow::array::StringArray>()
        .unwrap();
    assert!(method_col.is_null(0));
}

// ─── Cleanup tests ─────────────────────────────────────────────────────────────

fn cleanup_search_event(query: &str, index: &str) -> SearchEvent {
    SearchEvent {
        timestamp_ms: chrono::Utc::now().timestamp_millis(),
        query: query.to_string(),
        query_id: None,
        index_name: index.to_string(),
        nb_hits: 10,
        processing_time_ms: 5,
        user_token: Some("testuser".to_string()),
        user_ip: Some("10.0.0.1".to_string()),
        filters: None,
        facets: None,
        analytics_tags: None,
        page: 0,
        hits_per_page: 20,
        has_results: true,
        country: None,
        region: None,
        experiment_id: None,
        variant_id: None,
        assignment_method: None,
    }
}

#[tokio::test]
async fn cleanup_no_orphans() {
    let analytics_dir = TempDir::new().unwrap();
    let index_dir = TempDir::new().unwrap();
    let config = cleanup_config(analytics_dir.path());
    let collector = AnalyticsCollector::new(config.clone());
    let engine = AnalyticsQueryEngine::new(config);
    collector.record_search(cleanup_search_event("laptop", "products"));
    collector.record_search(cleanup_search_event("action", "movies"));
    collector.flush_all();
    std::fs::create_dir_all(index_dir.path().join("products")).unwrap();
    std::fs::create_dir_all(index_dir.path().join("movies")).unwrap();
    let removed = run_cleanup(&engine, index_dir.path());
    assert!(removed.is_empty());
}

#[tokio::test]
async fn cleanup_all_orphaned() {
    let analytics_dir = TempDir::new().unwrap();
    let index_dir = TempDir::new().unwrap();
    let config = cleanup_config(analytics_dir.path());
    let collector = AnalyticsCollector::new(config.clone());
    let engine = AnalyticsQueryEngine::new(config);
    collector.record_search(cleanup_search_event("laptop", "products"));
    collector.record_search(cleanup_search_event("action", "movies"));
    collector.flush_all();
    let removed = run_cleanup(&engine, index_dir.path());
    assert_eq!(removed.len(), 2);
    assert!(!analytics_dir.path().join("products").exists());
    assert!(!analytics_dir.path().join("movies").exists());
}

#[tokio::test]
async fn cleanup_mixed() {
    let analytics_dir = TempDir::new().unwrap();
    let index_dir = TempDir::new().unwrap();
    let config = cleanup_config(analytics_dir.path());
    let collector = AnalyticsCollector::new(config.clone());
    let engine = AnalyticsQueryEngine::new(config);
    collector.record_search(cleanup_search_event("laptop", "products"));
    collector.record_search(cleanup_search_event("action", "movies"));
    collector.record_search(cleanup_search_event("query", "old-deleted-index"));
    collector.flush_all();
    std::fs::create_dir_all(index_dir.path().join("products")).unwrap();
    std::fs::create_dir_all(index_dir.path().join("movies")).unwrap();
    let removed = run_cleanup(&engine, index_dir.path());
    assert_eq!(removed.len(), 1);
    assert_eq!(removed[0], "old-deleted-index");
    assert!(analytics_dir.path().join("products").exists());
    assert!(!analytics_dir.path().join("old-deleted-index").exists());
}

#[tokio::test]
async fn cleanup_no_analytics_data() {
    let analytics_dir = TempDir::new().unwrap();
    let index_dir = TempDir::new().unwrap();
    let config = cleanup_config(analytics_dir.path());
    let engine = AnalyticsQueryEngine::new(config);
    std::fs::create_dir_all(index_dir.path().join("products")).unwrap();
    let removed = run_cleanup(&engine, index_dir.path());
    assert!(removed.is_empty());
}

#[test]
fn insight_event_interleaving_team_roundtrip() {
    use crate::analytics::schema::insight_event_schema;
    use arrow::array::Array;
    use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
    use std::fs::File;

    let tmp = TempDir::new().unwrap();
    let dir = tmp.path().join("events");
    let mut event = make_insight_ev("click", "products", Some(&"a".repeat(32)));
    event.interleaving_team = Some("control".to_string());
    writer::flush_insight_events(&[event], &dir).unwrap();

    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let partition_dir = dir.join(format!("date={}", today));
    let parquet_file = std::fs::read_dir(&partition_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .find(|e| {
            e.path()
                .extension()
                .map(|x| x == "parquet")
                .unwrap_or(false)
        })
        .unwrap();
    let file = File::open(parquet_file.path()).unwrap();
    let reader = ParquetRecordBatchReaderBuilder::try_new(file)
        .unwrap()
        .build()
        .unwrap();
    let batches: Vec<_> = reader.collect::<Result<_, _>>().unwrap();
    assert_eq!(batches.len(), 1);
    let batch = &batches[0];

    let schema = insight_event_schema();
    let team_idx = schema.index_of("interleaving_team").unwrap();
    let team_col = batch
        .column(team_idx)
        .as_any()
        .downcast_ref::<arrow::array::StringArray>()
        .unwrap();
    assert_eq!(team_col.value(0), "control");
}

#[test]
fn insight_event_null_interleaving_team_roundtrip() {
    use crate::analytics::schema::insight_event_schema;
    use arrow::array::Array;
    use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
    use std::fs::File;

    let tmp = TempDir::new().unwrap();
    let dir = tmp.path().join("events");
    let event = make_insight_ev("click", "products", Some(&"a".repeat(32)));
    // interleaving_team is None by default
    writer::flush_insight_events(&[event], &dir).unwrap();

    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let partition_dir = dir.join(format!("date={}", today));
    let parquet_file = std::fs::read_dir(&partition_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .find(|e| {
            e.path()
                .extension()
                .map(|x| x == "parquet")
                .unwrap_or(false)
        })
        .unwrap();
    let file = File::open(parquet_file.path()).unwrap();
    let reader = ParquetRecordBatchReaderBuilder::try_new(file)
        .unwrap()
        .build()
        .unwrap();
    let batches: Vec<_> = reader.collect::<Result<_, _>>().unwrap();
    assert_eq!(batches.len(), 1);
    let batch = &batches[0];

    let schema = insight_event_schema();
    let team_idx = schema.index_of("interleaving_team").unwrap();
    let team_col = batch
        .column(team_idx)
        .as_any()
        .downcast_ref::<arrow::array::StringArray>()
        .unwrap();
    assert!(
        team_col.is_null(0),
        "interleaving_team should be null when not set"
    );
}

#[tokio::test]
async fn cleanup_analytics_dir_missing() {
    let analytics_dir = TempDir::new().unwrap();
    let index_dir = TempDir::new().unwrap();
    let config = AnalyticsConfig {
        enabled: true,
        data_dir: analytics_dir.path().join("nonexistent"),
        flush_interval_secs: 3600,
        flush_size: 10_000,
        retention_days: 90,
    };
    let engine = AnalyticsQueryEngine::new(config);
    let removed = run_cleanup(&engine, index_dir.path());
    assert!(removed.is_empty());
}
