//! Tests for the analytics cleanup logic.
//!
//! Verifies that orphaned analytics directories (for indexes that have been
//! deleted) are correctly identified and removed, while active index analytics
//! are left untouched.

use flapjack::analytics::collector::AnalyticsCollector;
use flapjack::analytics::config::AnalyticsConfig;
use flapjack::analytics::query::AnalyticsQueryEngine;
use flapjack::analytics::schema::SearchEvent;
use std::collections::HashSet;
use tempfile::TempDir;

fn test_config(dir: &std::path::Path) -> AnalyticsConfig {
    AnalyticsConfig {
        enabled: true,
        data_dir: dir.to_path_buf(),
        flush_interval_secs: 3600,
        flush_size: 10_000,
        retention_days: 90,
    }
}

fn search_event(query: &str, index: &str) -> SearchEvent {
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
    }
}

/// Simulate the cleanup logic from the handler: diff analytics indices vs active
/// indices and delete orphaned analytics directories.
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

/// Test 1: No orphaned indexes — all analytics dirs correspond to active indexes.
#[tokio::test]
async fn cleanup_no_orphans() {
    let analytics_dir = TempDir::new().unwrap();
    let index_dir = TempDir::new().unwrap();
    let config = test_config(analytics_dir.path());
    let collector = AnalyticsCollector::new(config.clone());
    let engine = AnalyticsQueryEngine::new(config);

    // Create analytics data for "products" and "movies"
    collector.record_search(search_event("laptop", "products"));
    collector.record_search(search_event("action", "movies"));
    collector.flush_all();

    // Create matching active index directories
    std::fs::create_dir_all(index_dir.path().join("products")).unwrap();
    std::fs::create_dir_all(index_dir.path().join("movies")).unwrap();

    let removed = run_cleanup(&engine, index_dir.path());
    assert!(removed.is_empty(), "No orphans should be found");

    // Verify analytics data still exists
    assert!(analytics_dir.path().join("products").exists());
    assert!(analytics_dir.path().join("movies").exists());
}

/// Test 2: All indexes orphaned — no active indexes exist.
#[tokio::test]
async fn cleanup_all_orphaned() {
    let analytics_dir = TempDir::new().unwrap();
    let index_dir = TempDir::new().unwrap();
    let config = test_config(analytics_dir.path());
    let collector = AnalyticsCollector::new(config.clone());
    let engine = AnalyticsQueryEngine::new(config);

    // Create analytics data for two indexes
    collector.record_search(search_event("laptop", "products"));
    collector.record_search(search_event("action", "movies"));
    collector.flush_all();

    // No active index directories — index_dir is empty

    let removed = run_cleanup(&engine, index_dir.path());
    assert_eq!(removed.len(), 2, "Both indexes should be orphaned");
    assert!(removed.contains(&"products".to_string()));
    assert!(removed.contains(&"movies".to_string()));

    // Verify analytics dirs were deleted
    assert!(!analytics_dir.path().join("products").exists());
    assert!(!analytics_dir.path().join("movies").exists());
}

/// Test 3: Mixed — some active, some orphaned.
#[tokio::test]
async fn cleanup_mixed() {
    let analytics_dir = TempDir::new().unwrap();
    let index_dir = TempDir::new().unwrap();
    let config = test_config(analytics_dir.path());
    let collector = AnalyticsCollector::new(config.clone());
    let engine = AnalyticsQueryEngine::new(config);

    // Create analytics data for three indexes
    collector.record_search(search_event("laptop", "products"));
    collector.record_search(search_event("action", "movies"));
    collector.record_search(search_event("query", "old-deleted-index"));
    collector.flush_all();

    // Only "products" and "movies" are active
    std::fs::create_dir_all(index_dir.path().join("products")).unwrap();
    std::fs::create_dir_all(index_dir.path().join("movies")).unwrap();

    let removed = run_cleanup(&engine, index_dir.path());
    assert_eq!(removed.len(), 1, "Only one orphan");
    assert_eq!(removed[0], "old-deleted-index");

    // Active index analytics still exist
    assert!(analytics_dir.path().join("products").exists());
    assert!(analytics_dir.path().join("movies").exists());

    // Orphaned analytics deleted
    assert!(!analytics_dir.path().join("old-deleted-index").exists());
}

/// Test 4: No analytics data at all — empty dir.
#[tokio::test]
async fn cleanup_no_analytics_data() {
    let analytics_dir = TempDir::new().unwrap();
    let index_dir = TempDir::new().unwrap();
    let config = test_config(analytics_dir.path());
    let engine = AnalyticsQueryEngine::new(config);

    // Create active indexes but no analytics data
    std::fs::create_dir_all(index_dir.path().join("products")).unwrap();

    let removed = run_cleanup(&engine, index_dir.path());
    assert!(removed.is_empty(), "Nothing to clean up");
}

/// Test 5: Analytics dir doesn't exist — cleanup returns cleanly.
#[tokio::test]
async fn cleanup_analytics_dir_missing() {
    let analytics_dir = TempDir::new().unwrap();
    let index_dir = TempDir::new().unwrap();

    // Point to a nonexistent subdirectory
    let config = AnalyticsConfig {
        enabled: true,
        data_dir: analytics_dir.path().join("nonexistent"),
        flush_interval_secs: 3600,
        flush_size: 10_000,
        retention_days: 90,
    };
    let engine = AnalyticsQueryEngine::new(config);

    let removed = run_cleanup(&engine, index_dir.path());
    assert!(
        removed.is_empty(),
        "No analytics dir means nothing to clean"
    );
}
