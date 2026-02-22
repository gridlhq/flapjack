use axum::{extract::State, Json};
use std::sync::Arc;

use super::AppState;

/// Health check endpoint
#[utoipa::path(
    get,
    path = "/health",
    tag = "health",
    responses(
        (status = 200, description = "Server is healthy", body = serde_json::Value)
    )
)]
pub async fn health(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let budget = flapjack::get_global_budget();
    let observer = flapjack::MemoryObserver::global();
    let mem_stats = observer.stats();

    Json(serde_json::json!({
        "status": "ok",
        "active_writers": budget.active_writers(),
        "max_concurrent_writers": budget.max_concurrent_writers(),
        "facet_cache_entries": state.manager.facet_cache.len(),
        "facet_cache_cap": state.manager.facet_cache_cap.load(std::sync::atomic::Ordering::Relaxed),
        "heap_allocated_mb": mem_stats.heap_allocated_bytes / (1024 * 1024),
        "system_limit_mb": mem_stats.system_limit_bytes / (1024 * 1024),
        "pressure_level": mem_stats.pressure_level.to_string(),
        "allocator": mem_stats.allocator,
        "build_profile": if cfg!(debug_assertions) { "debug" } else { "release" },
        "tenants_loaded": state.manager.loaded_count(),
        "uptime_secs": state.start_time.elapsed().as_secs(),
        "version": env!("CARGO_PKG_VERSION"),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handlers::metrics::MetricsState;
    use axum::body::Body;
    use axum::http::Request;
    use axum::routing::get;
    use axum::Router;
    use flapjack::IndexManager;
    use tempfile::TempDir;
    use tower::ServiceExt;

    fn make_health_state(tmp: &TempDir) -> Arc<AppState> {
        Arc::new(AppState {
            manager: IndexManager::new(tmp.path()),
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
    async fn health_includes_tenants_loaded() {
        let tmp = TempDir::new().unwrap();
        let state = make_health_state(&tmp);
        state.manager.create_tenant("t1").unwrap();
        state.manager.create_tenant("t2").unwrap();

        let app = Router::new()
            .route("/health", get(health))
            .with_state(state);

        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(json["tenants_loaded"].as_u64().unwrap(), 2);
    }

    #[tokio::test]
    async fn health_includes_uptime_secs() {
        let tmp = TempDir::new().unwrap();
        let state = make_health_state(&tmp);

        let app = Router::new()
            .route("/health", get(health))
            .with_state(state);

        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert!(
            json["uptime_secs"].as_u64().is_some(),
            "should have uptime_secs field"
        );
    }

    #[tokio::test]
    async fn health_includes_version() {
        let tmp = TempDir::new().unwrap();
        let state = make_health_state(&tmp);

        let app = Router::new()
            .route("/health", get(health))
            .with_state(state);

        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

        let version = json["version"].as_str().unwrap();
        assert_eq!(
            version,
            env!("CARGO_PKG_VERSION"),
            "version should match CARGO_PKG_VERSION"
        );
    }
}
