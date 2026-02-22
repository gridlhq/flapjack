//! Request counting middleware for per-index usage metrics.
//!
//! Tracks search, write, and read request counts plus bytes ingested,
//! per index name. Counters are exposed via the `/metrics` endpoint.

use axum::{extract::Request, http::Method, middleware::Next, response::Response};
use dashmap::DashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Per-index usage counters. All fields are atomically updated so the
/// struct can be shared across request handlers without locking.
pub struct TenantUsageCounters {
    pub search_count: AtomicU64,
    pub write_count: AtomicU64,
    pub read_count: AtomicU64,
    pub bytes_in: AtomicU64,
    pub search_results_total: AtomicU64,
    pub documents_indexed_total: AtomicU64,
    pub documents_deleted_total: AtomicU64,
}

impl TenantUsageCounters {
    pub fn new() -> Self {
        Self {
            search_count: AtomicU64::new(0),
            write_count: AtomicU64::new(0),
            read_count: AtomicU64::new(0),
            bytes_in: AtomicU64::new(0),
            search_results_total: AtomicU64::new(0),
            documents_indexed_total: AtomicU64::new(0),
            documents_deleted_total: AtomicU64::new(0),
        }
    }
}

impl Default for TenantUsageCounters {
    fn default() -> Self {
        Self::new()
    }
}

/// Classification of an index request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestKind {
    Search,
    Write,
    Read,
}

/// Extract the index name from a `/1/indexes/:indexName/...` URL path.
///
/// Returns `None` for paths that don't match the index pattern.
pub fn extract_index_name(path: &str) -> Option<String> {
    let mut segments = path.split('/').filter(|s| !s.is_empty());
    if segments.next()? != "1" {
        return None;
    }
    if segments.next()? != "indexes" {
        return None;
    }
    let name = segments.next()?;
    if name.is_empty() {
        return None;
    }
    Some(name.to_string())
}

/// Classify a request as Search, Write, or Read based on HTTP method and
/// the path segment after the index name.
pub fn classify_request(method: &Method, path: &str) -> Option<RequestKind> {
    let index_name = extract_index_name(path)?;
    let suffix = path
        .strip_prefix(&format!("/1/indexes/{}", index_name))
        .unwrap_or("");
    let suffix = suffix.strip_prefix('/').unwrap_or(suffix);
    let first_segment = suffix.split('/').next().unwrap_or("");

    match (method, first_segment) {
        // Search operations
        (&Method::POST, "query") | (&Method::POST, "queries") => Some(RequestKind::Search),

        // Read operations
        (&Method::POST, "objects") | (&Method::POST, "browse") => Some(RequestKind::Read),
        (&Method::GET, seg) if !seg.is_empty() => Some(RequestKind::Read),

        // Write operations
        (&Method::POST, "batch") | (&Method::POST, "deleteByQuery") => Some(RequestKind::Write),
        (&Method::PUT, _) | (&Method::DELETE, _) => Some(RequestKind::Write),
        // POST to index root (add_record_auto_id)
        (&Method::POST, "") => Some(RequestKind::Write),

        _ => None,
    }
}

/// Axum middleware that counts requests per index.
pub async fn usage_counting_layer(
    request: Request,
    next: Next,
    counters: &Arc<DashMap<String, TenantUsageCounters>>,
) -> Response {
    let path = request.uri().path().to_string();
    let method = request.method().clone();

    if let Some(index_name) = extract_index_name(&path) {
        let content_length: u64 = request
            .headers()
            .get("content-length")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);

        let entry = counters
            .entry(index_name)
            .or_insert_with(TenantUsageCounters::new);

        if content_length > 0 {
            entry.bytes_in.fetch_add(content_length, Ordering::Relaxed);
        }

        if let Some(kind) = classify_request(&method, &path) {
            match kind {
                RequestKind::Search => {
                    entry.search_count.fetch_add(1, Ordering::Relaxed);
                }
                RequestKind::Write => {
                    entry.write_count.fetch_add(1, Ordering::Relaxed);
                }
                RequestKind::Read => {
                    entry.read_count.fetch_add(1, Ordering::Relaxed);
                }
            }
        }
    }

    next.run(request).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::Method;

    // ── extract_index_name ──

    #[test]
    fn extract_from_query_path() {
        assert_eq!(
            extract_index_name("/1/indexes/products/query"),
            Some("products".to_string())
        );
    }

    #[test]
    fn extract_from_batch_path() {
        assert_eq!(
            extract_index_name("/1/indexes/my-idx/batch"),
            Some("my-idx".to_string())
        );
    }

    #[test]
    fn extract_from_object_id_path() {
        assert_eq!(
            extract_index_name("/1/indexes/products/abc123"),
            Some("products".to_string())
        );
    }

    #[test]
    fn extract_from_index_root() {
        assert_eq!(
            extract_index_name("/1/indexes/products"),
            Some("products".to_string())
        );
    }

    #[test]
    fn extract_none_for_health() {
        assert_eq!(extract_index_name("/health"), None);
    }

    #[test]
    fn extract_none_for_metrics() {
        assert_eq!(extract_index_name("/metrics"), None);
    }

    #[test]
    fn extract_none_for_keys() {
        assert_eq!(extract_index_name("/1/keys"), None);
    }

    #[test]
    fn extract_none_for_internal() {
        assert_eq!(extract_index_name("/internal/status"), None);
    }

    #[test]
    fn extract_none_for_analytics() {
        assert_eq!(extract_index_name("/2/searches"), None);
    }

    // ── classify_request ──

    #[test]
    fn classify_search_query() {
        assert_eq!(
            classify_request(&Method::POST, "/1/indexes/products/query"),
            Some(RequestKind::Search)
        );
    }

    #[test]
    fn classify_search_queries() {
        assert_eq!(
            classify_request(&Method::POST, "/1/indexes/products/queries"),
            Some(RequestKind::Search)
        );
    }

    #[test]
    fn classify_write_batch() {
        assert_eq!(
            classify_request(&Method::POST, "/1/indexes/products/batch"),
            Some(RequestKind::Write)
        );
    }

    #[test]
    fn classify_write_put_object() {
        assert_eq!(
            classify_request(&Method::PUT, "/1/indexes/products/abc123"),
            Some(RequestKind::Write)
        );
    }

    #[test]
    fn classify_write_delete_object() {
        assert_eq!(
            classify_request(&Method::DELETE, "/1/indexes/products/abc123"),
            Some(RequestKind::Write)
        );
    }

    #[test]
    fn classify_write_delete_by_query() {
        assert_eq!(
            classify_request(&Method::POST, "/1/indexes/products/deleteByQuery"),
            Some(RequestKind::Write)
        );
    }

    #[test]
    fn classify_write_post_to_index_root() {
        assert_eq!(
            classify_request(&Method::POST, "/1/indexes/products"),
            Some(RequestKind::Write)
        );
    }

    #[test]
    fn classify_read_get_object() {
        assert_eq!(
            classify_request(&Method::GET, "/1/indexes/products/abc123"),
            Some(RequestKind::Read)
        );
    }

    #[test]
    fn classify_read_post_objects() {
        assert_eq!(
            classify_request(&Method::POST, "/1/indexes/products/objects"),
            Some(RequestKind::Read)
        );
    }

    #[test]
    fn classify_read_post_browse() {
        assert_eq!(
            classify_request(&Method::POST, "/1/indexes/products/browse"),
            Some(RequestKind::Read)
        );
    }

    #[test]
    fn classify_none_for_non_index() {
        assert_eq!(classify_request(&Method::GET, "/health"), None);
    }

    #[test]
    fn classify_none_for_keys() {
        assert_eq!(classify_request(&Method::GET, "/1/keys"), None);
    }

    // ── middleware unit tests ──

    #[tokio::test]
    async fn middleware_increments_search_count() {
        let counters = Arc::new(DashMap::new());
        let c = counters.clone();

        let handler = || async { "ok" };
        let app = axum::Router::new()
            .route("/1/indexes/:idx/query", axum::routing::post(handler))
            .layer(axum::middleware::from_fn(move |req, next| {
                let c = c.clone();
                async move { usage_counting_layer(req, next, &c).await }
            }));

        let req = axum::http::Request::builder()
            .method("POST")
            .uri("/1/indexes/products/query")
            .header("content-type", "application/json")
            .body(axum::body::Body::from("{}"))
            .unwrap();

        tower::ServiceExt::oneshot(app, req).await.unwrap();

        let entry = counters
            .get("products")
            .expect("counter entry should exist");
        assert_eq!(entry.search_count.load(Ordering::Relaxed), 1);
        assert_eq!(entry.write_count.load(Ordering::Relaxed), 0);
    }

    #[tokio::test]
    async fn middleware_increments_write_count() {
        let counters = Arc::new(DashMap::new());
        let c = counters.clone();

        let handler = || async { "ok" };
        let app = axum::Router::new()
            .route("/1/indexes/:idx/batch", axum::routing::post(handler))
            .layer(axum::middleware::from_fn(move |req, next| {
                let c = c.clone();
                async move { usage_counting_layer(req, next, &c).await }
            }));

        let req = axum::http::Request::builder()
            .method("POST")
            .uri("/1/indexes/products/batch")
            .header("content-type", "application/json")
            .body(axum::body::Body::from("{}"))
            .unwrap();

        tower::ServiceExt::oneshot(app, req).await.unwrap();

        let entry = counters
            .get("products")
            .expect("counter entry should exist");
        assert_eq!(entry.write_count.load(Ordering::Relaxed), 1);
    }

    #[tokio::test]
    async fn middleware_tracks_bytes_in() {
        let counters = Arc::new(DashMap::new());
        let c = counters.clone();

        let handler = || async { "ok" };
        let app = axum::Router::new()
            .route("/1/indexes/:idx/batch", axum::routing::post(handler))
            .layer(axum::middleware::from_fn(move |req, next| {
                let c = c.clone();
                async move { usage_counting_layer(req, next, &c).await }
            }));

        let body = r#"{"requests":[]}"#;
        let req = axum::http::Request::builder()
            .method("POST")
            .uri("/1/indexes/products/batch")
            .header("content-type", "application/json")
            .header("content-length", body.len().to_string())
            .body(axum::body::Body::from(body))
            .unwrap();

        tower::ServiceExt::oneshot(app, req).await.unwrap();

        let entry = counters
            .get("products")
            .expect("counter entry should exist");
        assert_eq!(entry.bytes_in.load(Ordering::Relaxed), body.len() as u64);
    }

    #[tokio::test]
    async fn middleware_increments_read_count() {
        let counters = Arc::new(DashMap::new());
        let c = counters.clone();

        let handler = || async { "ok" };
        let app = axum::Router::new()
            .route("/1/indexes/:idx/:objectID", axum::routing::get(handler))
            .layer(axum::middleware::from_fn(move |req, next| {
                let c = c.clone();
                async move { usage_counting_layer(req, next, &c).await }
            }));

        let req = axum::http::Request::builder()
            .method("GET")
            .uri("/1/indexes/products/abc123")
            .body(axum::body::Body::empty())
            .unwrap();

        tower::ServiceExt::oneshot(app, req).await.unwrap();

        let entry = counters
            .get("products")
            .expect("counter entry should exist");
        assert_eq!(entry.read_count.load(Ordering::Relaxed), 1);
        assert_eq!(entry.search_count.load(Ordering::Relaxed), 0);
        assert_eq!(entry.write_count.load(Ordering::Relaxed), 0);
    }

    #[tokio::test]
    async fn middleware_ignores_non_index_routes() {
        let counters = Arc::new(DashMap::new());
        let c = counters.clone();

        let handler = || async { "ok" };
        let app = axum::Router::new()
            .route("/health", axum::routing::get(handler))
            .layer(axum::middleware::from_fn(move |req, next| {
                let c = c.clone();
                async move { usage_counting_layer(req, next, &c).await }
            }));

        let req = axum::http::Request::builder()
            .uri("/health")
            .body(axum::body::Body::empty())
            .unwrap();

        tower::ServiceExt::oneshot(app, req).await.unwrap();

        assert!(
            counters.is_empty(),
            "no counter entries for non-index routes"
        );
    }

    // ── handler-level counter integration tests ──

    fn make_app_state(tmp: &tempfile::TempDir) -> std::sync::Arc<crate::handlers::AppState> {
        std::sync::Arc::new(crate::handlers::AppState {
            manager: flapjack::IndexManager::new(tmp.path()),
            key_store: None,
            replication_manager: None,
            ssl_manager: None,
            analytics_engine: None,
            experiment_store: None,
            metrics_state: None,
            usage_counters: Arc::new(DashMap::new()),
            paused_indexes: crate::pause_registry::PausedIndexes::new(),
            start_time: std::time::Instant::now(),
            #[cfg(feature = "vector-search")]
            embedder_store: std::sync::Arc::new(crate::embedder_store::EmbedderStore::new()),
        })
    }

    #[tokio::test]
    async fn handler_documents_indexed_total_increments_on_put() {
        let tmp = tempfile::TempDir::new().unwrap();
        let state = make_app_state(&tmp);
        state.manager.create_tenant("test_idx").unwrap();

        let app = axum::Router::new()
            .route(
                "/1/indexes/:indexName/:objectID",
                axum::routing::put(crate::handlers::put_object),
            )
            .with_state(state.clone());

        let req = axum::http::Request::builder()
            .method("PUT")
            .uri("/1/indexes/test_idx/doc1")
            .header("content-type", "application/json")
            .body(axum::body::Body::from(r#"{"name":"Alice"}"#))
            .unwrap();

        let resp = tower::ServiceExt::oneshot(app, req).await.unwrap();
        assert_eq!(resp.status().as_u16(), 200);

        let entry = state
            .usage_counters
            .get("test_idx")
            .expect("counter entry should exist");
        assert_eq!(
            entry.documents_indexed_total.load(Ordering::Relaxed),
            1,
            "put_object should increment documents_indexed_total by 1"
        );
    }

    #[tokio::test]
    async fn handler_documents_indexed_total_increments_on_batch() {
        let tmp = tempfile::TempDir::new().unwrap();
        let state = make_app_state(&tmp);

        let app = axum::Router::new()
            .route(
                "/1/indexes/:indexName/batch",
                axum::routing::post(crate::handlers::add_documents),
            )
            .with_state(state.clone());

        let body = serde_json::json!({
            "requests": [
                {"action": "addObject", "body": {"objectID": "a", "name": "Alice"}},
                {"action": "addObject", "body": {"objectID": "b", "name": "Bob"}},
                {"action": "addObject", "body": {"objectID": "c", "name": "Carol"}}
            ]
        });

        let req = axum::http::Request::builder()
            .method("POST")
            .uri("/1/indexes/batch_idx/batch")
            .header("content-type", "application/json")
            .body(axum::body::Body::from(
                serde_json::to_string(&body).unwrap(),
            ))
            .unwrap();

        let resp = tower::ServiceExt::oneshot(app, req).await.unwrap();
        assert_eq!(resp.status().as_u16(), 200);

        let entry = state
            .usage_counters
            .get("batch_idx")
            .expect("counter entry should exist");
        assert_eq!(
            entry.documents_indexed_total.load(Ordering::Relaxed),
            3,
            "batch of 3 addObject should increment documents_indexed_total by 3"
        );
    }

    #[tokio::test]
    async fn handler_search_results_total_increments_on_search() {
        let tmp = tempfile::TempDir::new().unwrap();
        let state = make_app_state(&tmp);

        // Add documents first so search has something to find
        state.manager.create_tenant("search_idx").unwrap();
        let docs = vec![
            flapjack::types::Document {
                id: "1".to_string(),
                fields: {
                    let mut m = std::collections::HashMap::new();
                    m.insert(
                        "name".to_string(),
                        flapjack::types::FieldValue::Text("hello world".to_string()),
                    );
                    m
                },
            },
            flapjack::types::Document {
                id: "2".to_string(),
                fields: {
                    let mut m = std::collections::HashMap::new();
                    m.insert(
                        "name".to_string(),
                        flapjack::types::FieldValue::Text("hello universe".to_string()),
                    );
                    m
                },
            },
        ];
        state
            .manager
            .add_documents_sync("search_idx", docs)
            .await
            .unwrap();

        let app = axum::Router::new()
            .route(
                "/1/indexes/:indexName/query",
                axum::routing::post(crate::handlers::search),
            )
            .with_state(state.clone());

        let body = serde_json::json!({"query": "hello"});
        let req = axum::http::Request::builder()
            .method("POST")
            .uri("/1/indexes/search_idx/query")
            .header("content-type", "application/json")
            .body(axum::body::Body::from(
                serde_json::to_string(&body).unwrap(),
            ))
            .unwrap();

        let resp = tower::ServiceExt::oneshot(app, req).await.unwrap();
        assert_eq!(resp.status().as_u16(), 200);

        let resp_body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&resp_body).unwrap();
        let nb_hits = json["nbHits"].as_u64().unwrap();

        let entry = state
            .usage_counters
            .get("search_idx")
            .expect("counter entry should exist");
        assert_eq!(
            entry.search_results_total.load(Ordering::Relaxed),
            nb_hits,
            "search_results_total should match nbHits from response"
        );
        assert!(
            nb_hits > 0,
            "search should return at least 1 hit for 'hello'"
        );
    }

    // ── concurrent correctness ──

    #[tokio::test]
    async fn concurrent_requests_no_lost_increments() {
        let counters = Arc::new(DashMap::new());
        let total_requests = 100;

        let mut handles = Vec::new();
        for _ in 0..total_requests {
            let c = counters.clone();
            let handle = tokio::spawn(async move {
                let handler = || async { "ok" };
                let c2 = c.clone();
                let app = axum::Router::new()
                    .route("/1/indexes/:idx/query", axum::routing::post(handler))
                    .layer(axum::middleware::from_fn(move |req, next| {
                        let c3 = c2.clone();
                        async move { usage_counting_layer(req, next, &c3).await }
                    }));

                let req = axum::http::Request::builder()
                    .method("POST")
                    .uri("/1/indexes/concurrent_test/query")
                    .header("content-type", "application/json")
                    .header("content-length", "2")
                    .body(axum::body::Body::from("{}"))
                    .unwrap();

                tower::ServiceExt::oneshot(app, req).await.unwrap();
            });
            handles.push(handle);
        }

        for h in handles {
            h.await.unwrap();
        }

        let entry = counters
            .get("concurrent_test")
            .expect("counter entry should exist");
        assert_eq!(
            entry.search_count.load(Ordering::Relaxed),
            total_requests,
            "all {} search increments should be counted",
            total_requests,
        );
        assert_eq!(
            entry.bytes_in.load(Ordering::Relaxed),
            total_requests * 2,
            "all bytes_in should be counted (2 bytes per request)",
        );
    }
}
