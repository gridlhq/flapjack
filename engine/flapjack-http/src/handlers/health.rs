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
    }))
}
