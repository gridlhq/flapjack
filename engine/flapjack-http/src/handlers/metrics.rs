//! Prometheus `/metrics` endpoint.
//!
//! Exposes system-wide gauges (writers, memory, tenants, facet cache) and
//! per-tenant storage gauges in Prometheus text exposition format.

use axum::extract::State;
use axum::http::{header, StatusCode};
use axum::response::IntoResponse;
use prometheus::{Encoder, GaugeVec, Opts, Registry, TextEncoder};
use std::sync::Arc;

use super::AppState;

/// GET /metrics — returns Prometheus text exposition format.
///
/// Gauges are populated on each request from live AppState / IndexManager /
/// MemoryObserver values. Per-tenant storage gauges are updated by a background
/// poller (see `server.rs`) and stored in `MetricsState`.
pub async fn metrics_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let registry = Registry::new();

    // --- System-wide gauges ---
    let budget = flapjack::get_global_budget();
    register_gauge(
        &registry,
        "flapjack_active_writers",
        "Number of active index writers",
        budget.active_writers() as f64,
    );
    register_gauge(
        &registry,
        "flapjack_max_concurrent_writers",
        "Maximum concurrent writers allowed",
        budget.max_concurrent_writers() as f64,
    );

    let observer = flapjack::MemoryObserver::global();
    let mem_stats = observer.stats();
    register_gauge(
        &registry,
        "flapjack_memory_heap_bytes",
        "Heap allocated bytes",
        mem_stats.heap_allocated_bytes as f64,
    );
    register_gauge(
        &registry,
        "flapjack_memory_limit_bytes",
        "System memory limit bytes",
        mem_stats.system_limit_bytes as f64,
    );
    let pressure_level: f64 = match mem_stats.pressure_level {
        flapjack::PressureLevel::Normal => 0.0,
        flapjack::PressureLevel::Elevated => 1.0,
        flapjack::PressureLevel::Critical => 2.0,
    };
    register_gauge(
        &registry,
        "flapjack_memory_pressure_level",
        "Memory pressure level (0=normal, 1=elevated, 2=critical)",
        pressure_level,
    );

    register_gauge(
        &registry,
        "flapjack_facet_cache_entries",
        "Number of entries in the facet cache",
        state.manager.facet_cache.len() as f64,
    );
    register_gauge(
        &registry,
        "flapjack_tenants_loaded",
        "Number of loaded tenant indexes",
        state.manager.loaded_count() as f64,
    );

    // --- Replication gauges ---
    match &state.replication_manager {
        Some(repl_mgr) => {
            register_gauge(
                &registry,
                "flapjack_replication_enabled",
                "Whether replication is enabled (1=yes, 0=no)",
                1.0,
            );
            let peer_gauge = GaugeVec::new(
                Opts::new(
                    "flapjack_peer_status",
                    "Peer health status (1=healthy, 0=unhealthy)",
                ),
                &["peer_id"],
            )
            .unwrap();
            registry.register(Box::new(peer_gauge.clone())).unwrap();
            for ps in repl_mgr.peer_statuses() {
                let value = if ps.status == "healthy" { 1.0 } else { 0.0 };
                peer_gauge.with_label_values(&[&ps.peer_id]).set(value);
            }
        }
        None => {
            register_gauge(
                &registry,
                "flapjack_replication_enabled",
                "Whether replication is enabled (1=yes, 0=no)",
                0.0,
            );
        }
    }

    // --- Per-tenant storage gauges (computed inline from loaded tenants) ---
    {
        let storage_gauge = GaugeVec::new(
            Opts::new("flapjack_storage_bytes", "Per-tenant disk storage in bytes"),
            &["index"],
        )
        .unwrap();
        registry.register(Box::new(storage_gauge.clone())).unwrap();
        for (tid, bytes) in state.manager.all_tenant_storage() {
            storage_gauge.with_label_values(&[&tid]).set(bytes as f64);
        }
    }

    // --- Per-index usage counters (from request counting middleware + handlers) ---
    {
        let search_gauge = GaugeVec::new(
            Opts::new(
                "flapjack_search_requests_total",
                "Total search requests per index",
            ),
            &["index"],
        )
        .unwrap();
        let write_gauge = GaugeVec::new(
            Opts::new(
                "flapjack_write_operations_total",
                "Total write operations per index",
            ),
            &["index"],
        )
        .unwrap();
        let read_gauge = GaugeVec::new(
            Opts::new(
                "flapjack_read_requests_total",
                "Total read requests per index",
            ),
            &["index"],
        )
        .unwrap();
        let bytes_in_gauge = GaugeVec::new(
            Opts::new("flapjack_bytes_in_total", "Total bytes ingested per index"),
            &["index"],
        )
        .unwrap();
        let search_results_gauge = GaugeVec::new(
            Opts::new(
                "flapjack_search_results_total",
                "Total search results returned per index",
            ),
            &["index"],
        )
        .unwrap();
        let docs_indexed_gauge = GaugeVec::new(
            Opts::new(
                "flapjack_documents_indexed_total",
                "Total documents indexed per index",
            ),
            &["index"],
        )
        .unwrap();
        let docs_deleted_gauge = GaugeVec::new(
            Opts::new(
                "flapjack_documents_deleted_total",
                "Total documents deleted per index",
            ),
            &["index"],
        )
        .unwrap();
        registry.register(Box::new(search_gauge.clone())).unwrap();
        registry.register(Box::new(write_gauge.clone())).unwrap();
        registry.register(Box::new(read_gauge.clone())).unwrap();
        registry.register(Box::new(bytes_in_gauge.clone())).unwrap();
        registry
            .register(Box::new(search_results_gauge.clone()))
            .unwrap();
        registry
            .register(Box::new(docs_indexed_gauge.clone()))
            .unwrap();
        registry
            .register(Box::new(docs_deleted_gauge.clone()))
            .unwrap();

        for entry in state.usage_counters.iter() {
            let idx = entry.key();
            let counters = entry.value();
            search_gauge.with_label_values(&[idx]).set(
                counters
                    .search_count
                    .load(std::sync::atomic::Ordering::Relaxed) as f64,
            );
            write_gauge.with_label_values(&[idx]).set(
                counters
                    .write_count
                    .load(std::sync::atomic::Ordering::Relaxed) as f64,
            );
            read_gauge.with_label_values(&[idx]).set(
                counters
                    .read_count
                    .load(std::sync::atomic::Ordering::Relaxed) as f64,
            );
            bytes_in_gauge
                .with_label_values(&[idx])
                .set(counters.bytes_in.load(std::sync::atomic::Ordering::Relaxed) as f64);
            search_results_gauge.with_label_values(&[idx]).set(
                counters
                    .search_results_total
                    .load(std::sync::atomic::Ordering::Relaxed) as f64,
            );
            docs_indexed_gauge.with_label_values(&[idx]).set(
                counters
                    .documents_indexed_total
                    .load(std::sync::atomic::Ordering::Relaxed) as f64,
            );
            docs_deleted_gauge.with_label_values(&[idx]).set(
                counters
                    .documents_deleted_total
                    .load(std::sync::atomic::Ordering::Relaxed) as f64,
            );
        }
    }

    // --- Per-tenant document count gauges (live from IndexManager) ---
    {
        let doc_count_gauge = GaugeVec::new(
            Opts::new(
                "flapjack_documents_count",
                "Number of documents per tenant index",
            ),
            &["index"],
        )
        .unwrap();
        registry
            .register(Box::new(doc_count_gauge.clone()))
            .unwrap();
        for tid in state.manager.loaded_tenant_ids() {
            if let Some(count) = state.manager.tenant_doc_count(&tid) {
                doc_count_gauge.with_label_values(&[&tid]).set(count as f64);
            }
        }
    }

    // --- Per-tenant oplog sequence gauges ---
    {
        let oplog_seq_gauge = GaugeVec::new(
            Opts::new(
                "flapjack_oplog_current_seq",
                "Current oplog sequence number per tenant",
            ),
            &["index"],
        )
        .unwrap();
        registry
            .register(Box::new(oplog_seq_gauge.clone()))
            .unwrap();
        for (tid, seq) in state.manager.all_tenant_oplog_seqs() {
            oplog_seq_gauge.with_label_values(&[&tid]).set(seq as f64);
        }
    }

    // Encode to text
    let encoder = TextEncoder::new();
    let metric_families = registry.gather();
    let mut buffer = Vec::new();
    if let Err(e) = encoder.encode(&metric_families, &mut buffer) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("metrics encode error: {}", e),
        )
            .into_response();
    }

    (
        StatusCode::OK,
        [(
            header::CONTENT_TYPE,
            "text/plain; version=0.0.4; charset=utf-8",
        )],
        buffer,
    )
        .into_response()
}

fn register_gauge(registry: &Registry, name: &str, help: &str, value: f64) {
    let gauge = prometheus::Gauge::new(name, help).unwrap();
    registry.register(Box::new(gauge.clone())).unwrap();
    gauge.set(value);
}

/// Shared state for metrics updated by background tasks.
///
/// The storage background poller writes per-tenant byte counts here;
/// the `/metrics` handler reads them.
#[derive(Clone)]
pub struct MetricsState {
    pub storage_gauges: Arc<dashmap::DashMap<String, u64>>,
}

impl MetricsState {
    pub fn new() -> Self {
        MetricsState {
            storage_gauges: Arc::new(dashmap::DashMap::new()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use axum::routing::get;
    use axum::Router;
    use flapjack::IndexManager;
    use tempfile::TempDir;
    use tower::ServiceExt;

    fn make_test_state(tmp: &TempDir) -> Arc<AppState> {
        let manager = IndexManager::new(tmp.path());
        Arc::new(AppState {
            manager,
            key_store: None,
            replication_manager: None,
            ssl_manager: None,
            analytics_engine: None,
            experiment_store: None,
            metrics_state: Some(MetricsState::new()),
            usage_counters: std::sync::Arc::new(dashmap::DashMap::new()),
            paused_indexes: crate::pause_registry::PausedIndexes::new(),
            start_time: std::time::Instant::now(),
            #[cfg(feature = "vector-search")]
            embedder_store: std::sync::Arc::new(crate::embedder_store::EmbedderStore::new()),
        })
    }

    #[tokio::test]
    async fn metrics_returns_200_with_prometheus_format() {
        let tmp = TempDir::new().unwrap();
        let state = make_test_state(&tmp);

        let app = Router::new()
            .route("/metrics", get(metrics_handler))
            .with_state(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/metrics")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let content_type = response
            .headers()
            .get("content-type")
            .unwrap()
            .to_str()
            .unwrap();
        assert!(
            content_type.contains("text/plain"),
            "should be text/plain, got: {}",
            content_type
        );

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let text = String::from_utf8(body.to_vec()).unwrap();

        // Check key gauges are present
        assert!(
            text.contains("flapjack_active_writers"),
            "missing flapjack_active_writers"
        );
        assert!(
            text.contains("flapjack_max_concurrent_writers"),
            "missing flapjack_max_concurrent_writers"
        );
        assert!(
            text.contains("flapjack_memory_heap_bytes"),
            "missing flapjack_memory_heap_bytes"
        );
        assert!(
            text.contains("flapjack_memory_limit_bytes"),
            "missing flapjack_memory_limit_bytes"
        );
        assert!(
            text.contains("flapjack_memory_pressure_level"),
            "missing flapjack_memory_pressure_level"
        );
        assert!(
            text.contains("flapjack_facet_cache_entries"),
            "missing flapjack_facet_cache_entries"
        );
        assert!(
            text.contains("flapjack_tenants_loaded"),
            "missing flapjack_tenants_loaded"
        );
        assert!(
            text.contains("flapjack_replication_enabled"),
            "missing flapjack_replication_enabled"
        );
    }

    #[tokio::test]
    async fn metrics_reflects_actual_tenant_count() {
        let tmp = TempDir::new().unwrap();
        let state = make_test_state(&tmp);

        // Create two tenants
        state.manager.create_tenant("idx1").unwrap();
        state.manager.create_tenant("idx2").unwrap();

        let app = Router::new()
            .route("/metrics", get(metrics_handler))
            .with_state(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/metrics")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let text = String::from_utf8(body.to_vec()).unwrap();

        // Find the flapjack_tenants_loaded line and verify value is 2
        let line = text
            .lines()
            .find(|l| l.starts_with("flapjack_tenants_loaded "))
            .unwrap();
        assert!(
            line.ends_with(" 2"),
            "tenants_loaded should be 2, got: {}",
            line
        );
    }

    #[tokio::test]
    async fn metrics_shows_storage_gauges_after_poller_update() {
        let tmp = TempDir::new().unwrap();
        let state = make_test_state(&tmp);

        // Create a tenant so it has some storage
        state.manager.create_tenant("store1").unwrap();

        // Simulate the background poller: call all_tenant_storage() and populate MetricsState
        let ms = state.metrics_state.as_ref().unwrap();
        let storage = state.manager.all_tenant_storage();
        ms.storage_gauges.clear();
        for (tid, bytes) in storage {
            ms.storage_gauges.insert(tid, bytes);
        }

        let app = Router::new()
            .route("/metrics", get(metrics_handler))
            .with_state(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/metrics")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let text = String::from_utf8(body.to_vec()).unwrap();

        // The storage gauge should appear with the tenant label
        assert!(
            text.contains("flapjack_storage_bytes"),
            "should contain flapjack_storage_bytes gauge"
        );
        assert!(
            text.contains("store1"),
            "should contain tenant label 'store1'"
        );

        // Verify the value is non-zero (tantivy creates meta files)
        let line = text
            .lines()
            .find(|l| l.contains("store1") && l.contains("flapjack_storage_bytes"))
            .unwrap();
        let value: f64 = line.split_whitespace().last().unwrap().parse().unwrap();
        assert!(
            value > 0.0,
            "storage bytes for store1 should be > 0, got: {}",
            value
        );
    }

    #[tokio::test]
    async fn metrics_includes_per_index_usage_counters() {
        let tmp = TempDir::new().unwrap();
        let state = make_test_state(&tmp);

        // Simulate some usage counter data
        {
            let counters = crate::usage_middleware::TenantUsageCounters::new();
            counters
                .search_count
                .store(5, std::sync::atomic::Ordering::Relaxed);
            counters
                .write_count
                .store(3, std::sync::atomic::Ordering::Relaxed);
            counters
                .read_count
                .store(2, std::sync::atomic::Ordering::Relaxed);
            counters
                .bytes_in
                .store(1024, std::sync::atomic::Ordering::Relaxed);
            counters
                .search_results_total
                .store(42, std::sync::atomic::Ordering::Relaxed);
            counters
                .documents_indexed_total
                .store(10, std::sync::atomic::Ordering::Relaxed);
            counters
                .documents_deleted_total
                .store(1, std::sync::atomic::Ordering::Relaxed);
            state
                .usage_counters
                .insert("test_index".to_string(), counters);
        }

        let app = Router::new()
            .route("/metrics", get(metrics_handler))
            .with_state(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/metrics")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let text = String::from_utf8(body.to_vec()).unwrap();

        // Verify all 7 per-index counter gauges appear with correct values
        let find_value = |metric_name: &str, index: &str| -> f64 {
            text.lines()
                .find(|l| l.contains(metric_name) && l.contains(index) && !l.starts_with('#'))
                .unwrap_or_else(|| {
                    panic!(
                        "metric {}{{index={}}} not found in:\n{}",
                        metric_name, index, text
                    )
                })
                .split_whitespace()
                .last()
                .unwrap()
                .parse()
                .unwrap()
        };

        assert_eq!(
            find_value("flapjack_search_requests_total", "test_index"),
            5.0
        );
        assert_eq!(
            find_value("flapjack_write_operations_total", "test_index"),
            3.0
        );
        assert_eq!(
            find_value("flapjack_read_requests_total", "test_index"),
            2.0
        );
        assert_eq!(find_value("flapjack_bytes_in_total", "test_index"), 1024.0);
        assert_eq!(
            find_value("flapjack_search_results_total", "test_index"),
            42.0
        );
        assert_eq!(
            find_value("flapjack_documents_indexed_total", "test_index"),
            10.0
        );
        assert_eq!(
            find_value("flapjack_documents_deleted_total", "test_index"),
            1.0
        );
    }

    #[tokio::test]
    async fn metrics_counter_values_match_known_operations() {
        let tmp = TempDir::new().unwrap();
        let state = make_test_state(&tmp);

        // Simulate two indexes with different counter values
        {
            let c1 = crate::usage_middleware::TenantUsageCounters::new();
            c1.search_count
                .store(10, std::sync::atomic::Ordering::Relaxed);
            c1.documents_indexed_total
                .store(100, std::sync::atomic::Ordering::Relaxed);
            state.usage_counters.insert("idx_a".to_string(), c1);

            let c2 = crate::usage_middleware::TenantUsageCounters::new();
            c2.write_count
                .store(7, std::sync::atomic::Ordering::Relaxed);
            c2.documents_deleted_total
                .store(3, std::sync::atomic::Ordering::Relaxed);
            state.usage_counters.insert("idx_b".to_string(), c2);
        }

        let app = Router::new()
            .route("/metrics", get(metrics_handler))
            .with_state(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/metrics")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let text = String::from_utf8(body.to_vec()).unwrap();

        // idx_a counters
        assert!(text.contains("idx_a"), "should contain idx_a label");
        assert!(text.contains("idx_b"), "should contain idx_b label");

        // Verify specific values per index
        let find = |metric: &str, idx: &str| -> f64 {
            text.lines()
                .find(|l| l.contains(metric) && l.contains(idx) && !l.starts_with('#'))
                .unwrap_or_else(|| panic!("{} for {} not found", metric, idx))
                .split_whitespace()
                .last()
                .unwrap()
                .parse()
                .unwrap()
        };

        assert_eq!(find("flapjack_search_requests_total", "idx_a"), 10.0);
        assert_eq!(find("flapjack_documents_indexed_total", "idx_a"), 100.0);
        assert_eq!(find("flapjack_write_operations_total", "idx_b"), 7.0);
        assert_eq!(find("flapjack_documents_deleted_total", "idx_b"), 3.0);
        // idx_a should have 0 writes, idx_b should have 0 searches
        assert_eq!(find("flapjack_write_operations_total", "idx_a"), 0.0);
        assert_eq!(find("flapjack_search_requests_total", "idx_b"), 0.0);
    }

    #[tokio::test]
    async fn metrics_includes_documents_count_gauge() {
        let tmp = TempDir::new().unwrap();
        let state = make_test_state(&tmp);

        state.manager.create_tenant("docs_idx").unwrap();
        let docs = vec![
            flapjack::types::Document {
                id: "d1".to_string(),
                fields: std::collections::HashMap::from([(
                    "name".to_string(),
                    flapjack::types::FieldValue::Text("Alice".to_string()),
                )]),
            },
            flapjack::types::Document {
                id: "d2".to_string(),
                fields: std::collections::HashMap::from([(
                    "name".to_string(),
                    flapjack::types::FieldValue::Text("Bob".to_string()),
                )]),
            },
        ];
        state
            .manager
            .add_documents_sync("docs_idx", docs)
            .await
            .unwrap();

        let app = Router::new()
            .route("/metrics", get(metrics_handler))
            .with_state(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/metrics")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let text = String::from_utf8(body.to_vec()).unwrap();

        let line = text
            .lines()
            .find(|l| {
                l.contains("flapjack_documents_count")
                    && l.contains("docs_idx")
                    && !l.starts_with('#')
            })
            .unwrap_or_else(|| {
                panic!(
                    "flapjack_documents_count for docs_idx not found in:\n{}",
                    text
                )
            });
        let value: f64 = line.split_whitespace().last().unwrap().parse().unwrap();
        assert_eq!(value, 2.0, "should have 2 docs in the gauge");
    }

    #[tokio::test]
    async fn metrics_includes_oplog_current_seq_gauge() {
        let tmp = TempDir::new().unwrap();
        let state = make_test_state(&tmp);

        state.manager.create_tenant("oplog_idx").unwrap();
        let docs = vec![flapjack::types::Document {
            id: "d1".to_string(),
            fields: std::collections::HashMap::from([(
                "name".to_string(),
                flapjack::types::FieldValue::Text("Alice".to_string()),
            )]),
        }];
        state
            .manager
            .add_documents_sync("oplog_idx", docs)
            .await
            .unwrap();

        let app = Router::new()
            .route("/metrics", get(metrics_handler))
            .with_state(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/metrics")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let text = String::from_utf8(body.to_vec()).unwrap();

        let line = text
            .lines()
            .find(|l| {
                l.contains("flapjack_oplog_current_seq")
                    && l.contains("oplog_idx")
                    && !l.starts_with('#')
            })
            .unwrap_or_else(|| {
                panic!(
                    "flapjack_oplog_current_seq for oplog_idx not found in:\n{}",
                    text
                )
            });
        let value: f64 = line.split_whitespace().last().unwrap().parse().unwrap();
        assert!(
            value > 0.0,
            "oplog seq should be > 0 after a write, got: {}",
            value
        );
    }
}
