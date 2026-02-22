//! Background rollup broadcaster (Phase 4b).
//!
//! Every `FLAPJACK_ROLLUP_INTERVAL_SECS` seconds (default 300), this task:
//!   1. Discovers all analytics indexes by listing subdirectories of the
//!      analytics data directory.
//!   2. For each index, calls the local AnalyticsQueryEngine to compute
//!      an AnalyticsRollup (top searches, search count, no-result searches).
//!   3. POSTs the rollup to every peer's /internal/analytics-rollup endpoint
//!      via AnalyticsClusterClient::push_rollup_to_peers().
//!
//! Peers cache the received rollup and use it to answer analytics queries
//! locally (Tier 2 path in maybe_fan_out), avoiding cross-region fan-out.

use crate::analytics_cluster::{AnalyticsClusterClient, AnalyticsRollup};
use flapjack::analytics::{AnalyticsConfig, AnalyticsQueryEngine};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

// ── Index discovery ───────────────────────────────────────────────────────────

/// Discover analytics index names by listing subdirectories of `data_dir`.
///
/// Each subdirectory name is treated as an index name. Non-directory entries
/// and names containing path separators are ignored.
pub async fn discover_indexes(data_dir: &Path) -> Vec<String> {
    if !data_dir.exists() {
        return Vec::new();
    }

    let mut indexes = Vec::new();
    if let Ok(entries) = std::fs::read_dir(data_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    indexes.push(name.to_string());
                }
            }
        }
    }
    indexes.sort(); // deterministic order for tests
    indexes
}

// ── Date helpers ──────────────────────────────────────────────────────────────

fn today_utc() -> String {
    chrono::Utc::now().format("%Y-%m-%d").to_string()
}

fn days_ago_utc(days: u32) -> String {
    let dt = chrono::Utc::now() - chrono::Duration::days(days as i64);
    dt.format("%Y-%m-%d").to_string()
}

// ── Rollup computation ────────────────────────────────────────────────────────

/// Compute an AnalyticsRollup for a single index using the last 30 days of data.
///
/// The rollup contains:
///   "searches"           → top_searches(limit=50)
///   "searches/count"     → search_count
///   "searches/noResults" → no_results_searches(limit=50)
///
/// If an analytics query fails (e.g. no data yet), the key is omitted.
pub async fn compute_rollup(
    engine: &AnalyticsQueryEngine,
    node_id: &str,
    index: &str,
) -> AnalyticsRollup {
    let now_secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let start = days_ago_utc(30);
    let end = today_utc();

    let mut results = HashMap::new();

    if let Ok(v) = engine
        .top_searches(index, &start, &end, 50, false, None, None)
        .await
    {
        results.insert("searches".to_string(), v);
    }

    if let Ok(v) = engine.search_count(index, &start, &end).await {
        results.insert("searches/count".to_string(), v);
    }

    if let Ok(v) = engine.no_results_searches(index, &start, &end, 50).await {
        results.insert("searches/noResults".to_string(), v);
    }

    AnalyticsRollup {
        node_id: node_id.to_string(),
        index: index.to_string(),
        generated_at_secs: now_secs,
        results,
    }
}

// ── Broadcast cycle ───────────────────────────────────────────────────────────

/// Run one complete broadcast cycle:
///   discover indexes → compute rollup for each → push to all peers.
///
/// This is the core logic extracted so unit tests can call it directly
/// without having to manage a spawned task.
pub async fn run_rollup_broadcast(
    engine: &AnalyticsQueryEngine,
    config: &AnalyticsConfig,
    cluster: &AnalyticsClusterClient,
    node_id: &str,
) {
    let indexes = discover_indexes(&config.data_dir).await;

    if indexes.is_empty() {
        tracing::debug!("[ROLLUP-BROADCAST] No analytics indexes found — nothing to broadcast");
        return;
    }

    tracing::debug!(
        "[ROLLUP-BROADCAST] Broadcasting rollups for {} indexes to {} peers",
        indexes.len(),
        cluster.peer_ids().len()
    );

    for index in &indexes {
        let rollup = compute_rollup(engine, node_id, index).await;
        cluster.push_rollup_to_peers(&rollup).await;
        tracing::info!(
            "[ROLLUP-BROADCAST] Pushed rollup node_id={} index={} result_keys={}",
            node_id,
            index,
            rollup.results.len()
        );
    }
}

// ── Background task ───────────────────────────────────────────────────────────

/// Spawn the background rollup broadcaster.
///
/// The first broadcast fires after `interval_secs` seconds (the initial
/// Tokio interval tick is consumed immediately and skipped to allow the
/// server to finish startup before the first push). After that, broadcasts
/// repeat every `interval_secs` seconds.
///
/// Configured via `FLAPJACK_ROLLUP_INTERVAL_SECS` env var (default 300).
/// Only called when both analytics AND cluster peers are configured.
pub fn spawn_rollup_broadcaster(
    engine: Arc<AnalyticsQueryEngine>,
    config: AnalyticsConfig,
    cluster: Arc<AnalyticsClusterClient>,
    node_id: String,
    interval_secs: u64,
) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(interval_secs));
        // MissedTickBehavior::Delay: if one cycle is slow, don't burst-catch-up
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        // Skip the immediate first tick — let the server finish starting up first
        interval.tick().await;

        loop {
            interval.tick().await;
            run_rollup_broadcast(&engine, &config, &cluster, &node_id).await;
        }
    });
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    // ── discover_indexes ──────────────────────────────────────────────────────

    #[tokio::test]
    async fn discover_indexes_empty_dir_returns_empty() {
        let dir = TempDir::new().unwrap();
        let result = discover_indexes(dir.path()).await;
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn discover_indexes_nonexistent_dir_returns_empty() {
        let result = discover_indexes(Path::new("/nonexistent/analytics/dir")).await;
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn discover_indexes_finds_subdirectories() {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir(dir.path().join("products")).unwrap();
        std::fs::create_dir(dir.path().join("orders")).unwrap();
        // A file (not a dir) should be ignored
        std::fs::write(dir.path().join("not-an-index.txt"), b"").unwrap();

        let mut result = discover_indexes(dir.path()).await;
        result.sort();
        assert_eq!(result, vec!["orders", "products"]);
    }

    #[tokio::test]
    async fn discover_indexes_sorted_output() {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir(dir.path().join("zzz")).unwrap();
        std::fs::create_dir(dir.path().join("aaa")).unwrap();
        std::fs::create_dir(dir.path().join("mmm")).unwrap();

        let result = discover_indexes(dir.path()).await;
        assert_eq!(result, vec!["aaa", "mmm", "zzz"]);
    }

    // ── compute_rollup ────────────────────────────────────────────────────────

    #[tokio::test]
    async fn compute_rollup_no_data_returns_valid_struct() {
        let dir = TempDir::new().unwrap();
        let config = flapjack::analytics::AnalyticsConfig {
            enabled: true,
            data_dir: dir.path().to_path_buf(),
            flush_interval_secs: 3600,
            flush_size: 100_000,
            retention_days: 90,
        };
        let engine = AnalyticsQueryEngine::new(config);

        let rollup = compute_rollup(&engine, "test-node", "empty-index").await;

        assert_eq!(rollup.node_id, "test-node");
        assert_eq!(rollup.index, "empty-index");
        assert!(rollup.generated_at_secs > 0, "timestamp must be positive");

        // The analytics engine always returns Ok for missing indexes (it creates
        // an empty in-memory table), so all 3 result keys must be present even
        // with no data on disk. A missing key here would mean compute_rollup
        // swallowed an unexpected Err from the engine.
        assert!(
            rollup.results.contains_key("searches"),
            "results must contain 'searches' key even for empty index; got: {:?}",
            rollup.results.keys().collect::<Vec<_>>()
        );
        assert!(
            rollup.results.contains_key("searches/count"),
            "results must contain 'searches/count' key even for empty index; got: {:?}",
            rollup.results.keys().collect::<Vec<_>>()
        );
        assert!(
            rollup.results.contains_key("searches/noResults"),
            "results must contain 'searches/noResults' key even for empty index; got: {:?}",
            rollup.results.keys().collect::<Vec<_>>()
        );
        assert_eq!(
            rollup.results.len(),
            3,
            "empty index rollup should have exactly 3 result keys; got: {:?}",
            rollup.results.keys().collect::<Vec<_>>()
        );
    }

    #[tokio::test]
    async fn compute_rollup_with_seeded_data_populates_searches_key() {
        let dir = TempDir::new().unwrap();
        let config = flapjack::analytics::AnalyticsConfig {
            enabled: true,
            data_dir: dir.path().join("analytics"),
            flush_interval_secs: 3600,
            flush_size: 100_000,
            retention_days: 90,
        };

        // Seed 1 day of analytics
        flapjack::analytics::seed::seed_analytics(&config, "products", 1)
            .expect("seed must succeed");

        let engine = AnalyticsQueryEngine::new(config.clone());
        let rollup = compute_rollup(&engine, "seeded-node", "products").await;

        assert_eq!(rollup.node_id, "seeded-node");
        assert_eq!(rollup.index, "products");
        // With seeded data the engine should populate at least "searches"
        assert!(
            rollup.results.contains_key("searches"),
            "Rollup should include 'searches' key; got: {:?}",
            rollup.results.keys().collect::<Vec<_>>()
        );
    }

    // ── run_rollup_broadcast + push_rollup_to_peers integration ──────────────

    /// Verify run_rollup_broadcast sends nothing when data_dir is empty.
    #[tokio::test]
    async fn run_rollup_broadcast_empty_analytics_dir_does_nothing() {
        use flapjack_replication::config::{NodeConfig, PeerConfig};

        let dir = TempDir::new().unwrap();
        let config = flapjack::analytics::AnalyticsConfig {
            enabled: true,
            data_dir: dir.path().to_path_buf(),
            flush_interval_secs: 3600,
            flush_size: 100_000,
            retention_days: 90,
        };
        let engine = Arc::new(AnalyticsQueryEngine::new(config.clone()));

        // Point at a non-existent peer — we expect zero HTTP calls so no error
        let node_cfg = NodeConfig {
            node_id: "local".to_string(),
            bind_addr: "127.0.0.1:0".to_string(),
            peers: vec![PeerConfig {
                node_id: "peer".to_string(),
                addr: "http://127.0.0.1:19999".to_string(), // nothing listening
            }],
        };
        let cluster = AnalyticsClusterClient::new(&node_cfg).expect("cluster client should build");

        // Should return without error even though peer is unreachable,
        // because with no indexes there's nothing to push.
        run_rollup_broadcast(&engine, &config, &cluster, "local").await;
        // If we reach here without panic, the test passes
    }
}
