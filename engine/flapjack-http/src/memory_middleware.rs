use axum::{
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use std::sync::atomic::Ordering;
use std::sync::Arc;

use flapjack::{IndexManager, PressureLevel};

/// Middleware that rejects requests under memory pressure and sheds caches.
///
/// - `Normal`: all requests proceed; restore original cache cap.
/// - `Elevated`: reject writes (POST/PUT/DELETE), allow reads and health;
///   reduce facet cache cap to 50%.
/// - `Critical`: reject all except `/health` and `/internal/status`;
///   clear facet cache entirely.
pub async fn memory_pressure_guard(
    request: Request,
    next: Next,
    manager: &Arc<IndexManager>,
    default_facet_cache_cap: usize,
) -> Response {
    let observer = flapjack::MemoryObserver::global();
    let level = observer.pressure_level();

    // Cache shedding based on pressure level
    match level {
        PressureLevel::Normal => {
            manager
                .facet_cache_cap
                .store(default_facet_cache_cap, Ordering::Relaxed);
        }
        PressureLevel::Elevated => {
            manager
                .facet_cache_cap
                .store(default_facet_cache_cap / 2, Ordering::Relaxed);
        }
        PressureLevel::Critical => {
            manager.facet_cache.clear();
            manager.facet_cache_cap.store(0, Ordering::Relaxed);
        }
    }

    match level {
        PressureLevel::Normal => next.run(request).await,
        PressureLevel::Elevated => {
            let path = request.uri().path().to_string();
            let method = request.method().clone();

            if path == "/health" || path.starts_with("/internal/") {
                return next.run(request).await;
            }

            if method == axum::http::Method::GET {
                return next.run(request).await;
            }

            reject_memory_pressure(observer)
        }
        PressureLevel::Critical => {
            let path = request.uri().path().to_string();

            if path == "/health" || path == "/internal/status" {
                return next.run(request).await;
            }

            reject_memory_pressure(observer)
        }
    }
}

fn reject_memory_pressure(observer: &flapjack::MemoryObserver) -> Response {
    let stats = observer.stats();
    let allocated_mb = stats.heap_allocated_bytes / (1024 * 1024);
    let limit_mb = stats.system_limit_bytes / (1024 * 1024);

    let body = serde_json::json!({
        "error": "memory_pressure",
        "allocated_mb": allocated_mb,
        "limit_mb": limit_mb,
        "level": stats.pressure_level.to_string(),
    });

    let mut response = (StatusCode::SERVICE_UNAVAILABLE, Json(body)).into_response();
    response
        .headers_mut()
        .insert("Retry-After", "5".parse().unwrap());
    response
}
