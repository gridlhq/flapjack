use axum::{
    middleware,
    routing::{get, post},
    Router,
};
use std::sync::Arc;
use tempfile::TempDir;
use tokio::net::TcpListener;

#[allow(dead_code)]
pub async fn spawn_server() -> (String, TempDir) {
    spawn_server_with_key(None).await
}

pub async fn spawn_server_with_key(admin_key: Option<&str>) -> (String, TempDir) {
    let temp_dir = TempDir::new().unwrap();
    let manager = flapjack::IndexManager::new(temp_dir.path());

    let key_store = admin_key.map(|k| {
        let ks = Arc::new(flapjack_http::auth::KeyStore::load_or_create(
            temp_dir.path(),
            k,
        ));
        // Write admin key to .admin_key file for consistency
        std::fs::write(temp_dir.path().join(".admin_key"), k).ok();
        ks
    });

    let state = Arc::new(flapjack_http::handlers::AppState {
        manager,
        key_store: key_store.clone(),
        replication_manager: None,
        ssl_manager: None,
        analytics_engine: None,
        experiment_store: Some(Arc::new(
            flapjack::experiments::store::ExperimentStore::new(temp_dir.path()).unwrap(),
        )),
        metrics_state: None,
        usage_counters: std::sync::Arc::new(dashmap::DashMap::new()),
        paused_indexes: flapjack_http::pause_registry::PausedIndexes::new(),
        start_time: std::time::Instant::now(),
    });

    let key_routes = if let Some(ref ks) = key_store {
        Router::new()
            .route(
                "/1/keys",
                post(flapjack_http::handlers::create_key).get(flapjack_http::handlers::list_keys),
            )
            .route(
                "/1/keys/:key",
                get(flapjack_http::handlers::get_key)
                    .put(flapjack_http::handlers::update_key)
                    .delete(flapjack_http::handlers::delete_key),
            )
            .route(
                "/1/keys/:key/restore",
                post(flapjack_http::handlers::restore_key),
            )
            .with_state(ks.clone())
    } else {
        Router::new()
    };

    let health_route = Router::new()
        .route("/health", get(flapjack_http::handlers::health))
        .with_state(state.clone());

    let protected = Router::new()
        .route("/1/indexes", post(flapjack_http::handlers::create_index))
        .route("/1/indexes", get(flapjack_http::handlers::list_indices))
        .route(
            "/1/indexes/:indexName/batch",
            post(flapjack_http::handlers::add_documents),
        )
        .route(
            "/1/indexes/:indexName/query",
            post(flapjack_http::handlers::search),
        )
        .route(
            "/1/indexes/:indexName/queries",
            post(flapjack_http::handlers::batch_search),
        )
        .route(
            "/1/indexes/:indexName/settings",
            get(flapjack_http::handlers::get_settings)
                .post(flapjack_http::handlers::set_settings)
                .put(flapjack_http::handlers::set_settings),
        )
        .route(
            "/1/indexes/:indexName/objects",
            post(flapjack_http::handlers::get_objects),
        )
        .route(
            "/1/indexes/:indexName/deleteByQuery",
            post(flapjack_http::handlers::delete_by_query),
        )
        .route(
            "/1/indexes/:indexName/:objectID/partial",
            post(flapjack_http::handlers::partial_update_object),
        )
        .route(
            "/1/indexes/:indexName/:objectID",
            get(flapjack_http::handlers::get_object)
                .delete(flapjack_http::handlers::delete_object)
                .put(flapjack_http::handlers::put_object),
        )
        .route(
            "/1/indexes/:indexName",
            post(flapjack_http::handlers::add_record_auto_id)
                .delete(flapjack_http::handlers::delete_index),
        )
        .route(
            "/1/indexes/:indexName/browse",
            post(flapjack_http::handlers::browse_index),
        )
        .route(
            "/1/indexes/:indexName/clear",
            post(flapjack_http::handlers::clear_index),
        )
        .route(
            "/1/indexes/:indexName/facets/:facetName/query",
            post(flapjack_http::handlers::search_facet_values),
        )
        .route(
            "/1/indexes/:indexName/synonyms/:objectID",
            get(flapjack_http::handlers::get_synonym)
                .put(flapjack_http::handlers::save_synonym)
                .delete(flapjack_http::handlers::delete_synonym),
        )
        .route(
            "/1/indexes/:indexName/synonyms/batch",
            post(flapjack_http::handlers::save_synonyms),
        )
        .route(
            "/1/indexes/:indexName/synonyms/clear",
            post(flapjack_http::handlers::clear_synonyms),
        )
        .route(
            "/1/indexes/:indexName/synonyms/search",
            post(flapjack_http::handlers::search_synonyms),
        )
        .route(
            "/1/indexes/:indexName/rules/:objectID",
            get(flapjack_http::handlers::get_rule)
                .put(flapjack_http::handlers::save_rule)
                .delete(flapjack_http::handlers::delete_rule),
        )
        .route(
            "/1/indexes/:indexName/rules/batch",
            post(flapjack_http::handlers::save_rules),
        )
        .route(
            "/1/indexes/:indexName/rules/clear",
            post(flapjack_http::handlers::clear_rules),
        )
        .route(
            "/1/indexes/:indexName/rules/search",
            post(flapjack_http::handlers::search_rules),
        )
        .route(
            "/1/indexes/:indexName/operation",
            post(flapjack_http::handlers::operation_index),
        )
        .route("/1/tasks/:task_id", get(flapjack_http::handlers::get_task))
        .route(
            "/1/indexes/:indexName/task/:task_id",
            get(flapjack_http::handlers::get_task_for_index),
        )
        // Query Suggestions API
        .route(
            "/1/configs",
            get(flapjack_http::handlers::query_suggestions::list_configs)
                .post(flapjack_http::handlers::query_suggestions::create_config),
        )
        .route(
            "/1/configs/:indexName",
            get(flapjack_http::handlers::query_suggestions::get_config)
                .put(flapjack_http::handlers::query_suggestions::update_config)
                .delete(flapjack_http::handlers::query_suggestions::delete_config),
        )
        .route(
            "/1/configs/:indexName/status",
            get(flapjack_http::handlers::query_suggestions::get_status),
        )
        .route(
            "/1/configs/:indexName/build",
            post(flapjack_http::handlers::query_suggestions::trigger_build),
        )
        .route(
            "/1/logs/:indexName",
            get(flapjack_http::handlers::query_suggestions::get_logs),
        )
        .route(
            "/2/abtests",
            post(flapjack_http::handlers::experiments::create_experiment)
                .get(flapjack_http::handlers::experiments::list_experiments),
        )
        .route(
            "/2/abtests/:id",
            get(flapjack_http::handlers::experiments::get_experiment)
                .put(flapjack_http::handlers::experiments::update_experiment)
                .delete(flapjack_http::handlers::experiments::delete_experiment),
        )
        .route(
            "/2/abtests/:id/start",
            post(flapjack_http::handlers::experiments::start_experiment),
        )
        .route(
            "/2/abtests/:id/stop",
            post(flapjack_http::handlers::experiments::stop_experiment),
        )
        .route(
            "/2/abtests/:id/results",
            get(flapjack_http::handlers::experiments::get_experiment_results),
        )
        .with_state(state.clone());

    let ks_for_middleware = key_store.clone();
    let auth_middleware = middleware::from_fn(
        move |mut request: axum::extract::Request, next: middleware::Next| {
            let ks = ks_for_middleware.clone();
            async move {
                if let Some(ref store) = ks {
                    request.extensions_mut().insert(store.clone());
                }
                flapjack_http::auth::authenticate_and_authorize(request, next).await
            }
        },
    );

    let app = Router::new()
        .merge(health_route)
        .merge(key_routes)
        .merge(protected)
        .layer(auth_middleware);

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap().to_string();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    // Poll health endpoint instead of blind sleep
    let client = reqwest::Client::new();
    for _ in 0..100 {
        if client
            .get(format!("http://{}/health", addr))
            .send()
            .await
            .is_ok()
        {
            break;
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(5)).await;
    }

    (addr, temp_dir)
}

/// Spawn a test server with analytics seeded for `source_index_name`.
///
/// Use this for Query Suggestions tests that exercise the build pipeline.
/// Seeds 30 days of realistic search data for the given source index.
#[allow(dead_code)]
pub async fn spawn_server_with_qs_analytics(source_index_name: &str) -> (String, TempDir) {
    let temp_dir = TempDir::new().unwrap();

    let analytics_config = flapjack::analytics::AnalyticsConfig {
        enabled: true,
        data_dir: temp_dir.path().join("analytics"),
        flush_interval_secs: 3600,
        flush_size: 100_000,
        retention_days: 90,
    };

    // Seed 30 days of analytics directly to disk (no HTTP roundtrip needed)
    flapjack::analytics::seed::seed_analytics(&analytics_config, source_index_name, 30)
        .expect("Failed to seed analytics data");

    let analytics_engine = Arc::new(flapjack::analytics::AnalyticsQueryEngine::new(
        analytics_config,
    ));

    let manager = flapjack::IndexManager::new(temp_dir.path());

    let state = Arc::new(flapjack_http::handlers::AppState {
        manager,
        key_store: None,
        replication_manager: None,
        ssl_manager: None,
        analytics_engine: Some(analytics_engine),
        experiment_store: None,
        metrics_state: None,
        usage_counters: std::sync::Arc::new(dashmap::DashMap::new()),
        paused_indexes: flapjack_http::pause_registry::PausedIndexes::new(),
        start_time: std::time::Instant::now(),
    });

    let health_route = Router::new()
        .route("/health", get(flapjack_http::handlers::health))
        .with_state(state.clone());

    let protected = Router::new()
        .route("/1/indexes", post(flapjack_http::handlers::create_index))
        .route("/1/indexes", get(flapjack_http::handlers::list_indices))
        .route(
            "/1/indexes/:indexName/batch",
            post(flapjack_http::handlers::add_documents),
        )
        .route(
            "/1/indexes/:indexName/query",
            post(flapjack_http::handlers::search),
        )
        .route("/1/tasks/:task_id", get(flapjack_http::handlers::get_task))
        // Query Suggestions API
        .route(
            "/1/configs",
            get(flapjack_http::handlers::query_suggestions::list_configs)
                .post(flapjack_http::handlers::query_suggestions::create_config),
        )
        .route(
            "/1/configs/:indexName",
            get(flapjack_http::handlers::query_suggestions::get_config)
                .put(flapjack_http::handlers::query_suggestions::update_config)
                .delete(flapjack_http::handlers::query_suggestions::delete_config),
        )
        .route(
            "/1/configs/:indexName/status",
            get(flapjack_http::handlers::query_suggestions::get_status),
        )
        .route(
            "/1/configs/:indexName/build",
            post(flapjack_http::handlers::query_suggestions::trigger_build),
        )
        .route(
            "/1/logs/:indexName",
            get(flapjack_http::handlers::query_suggestions::get_logs),
        )
        .with_state(state);

    let app = Router::new().merge(health_route).merge(protected);

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap().to_string();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let client = reqwest::Client::new();
    for _ in 0..100 {
        if client
            .get(format!("http://{}/health", addr))
            .send()
            .await
            .is_ok()
        {
            break;
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(5)).await;
    }

    (addr, temp_dir)
}

/// Poll the task endpoint until the task reaches "published" status.
/// Use this instead of blind sleeps after batch/write operations.
#[allow(dead_code)]
pub async fn wait_for_task(client: &reqwest::Client, addr: &str, task_id: i64) {
    wait_for_task_authed(client, addr, task_id, None).await;
}

/// Like `wait_for_task` but sends authentication headers (for servers with auth enabled).
#[allow(dead_code)]
pub async fn wait_for_task_authed(
    client: &reqwest::Client,
    addr: &str,
    task_id: i64,
    api_key: Option<&str>,
) {
    for _ in 0..5000 {
        let mut req = client.get(format!("http://{}/1/tasks/{}", addr, task_id));
        if let Some(key) = api_key {
            req = req
                .header("x-algolia-api-key", key)
                .header("x-algolia-application-id", "test");
        }
        let resp = req.send().await.unwrap();
        let body: serde_json::Value = resp.json().await.unwrap();
        match body["status"].as_str().unwrap_or("pending") {
            "published" => return,
            "error" => panic!(
                "Task {} failed with error: {}",
                task_id,
                body.get("error")
                    .and_then(|e| e.as_str())
                    .unwrap_or("unknown")
            ),
            _ => tokio::time::sleep(tokio::time::Duration::from_millis(10)).await,
        }
    }
    panic!("Task {} did not complete within 50s timeout", task_id);
}

/// Extract taskID from a batch/write response body and wait for it to complete.
#[allow(dead_code)]
pub async fn wait_for_response_task(client: &reqwest::Client, addr: &str, resp: reqwest::Response) {
    wait_for_response_task_authed(client, addr, resp, None).await;
}

/// Like `wait_for_response_task` but sends authentication headers.
#[allow(dead_code)]
pub async fn wait_for_response_task_authed(
    client: &reqwest::Client,
    addr: &str,
    resp: reqwest::Response,
    api_key: Option<&str>,
) {
    let status = resp.status();
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(
        status.is_success(),
        "Expected 2xx response but got {}: {}",
        status,
        body
    );
    let task_id = body
        .get("taskID")
        .and_then(|v| v.as_i64())
        .unwrap_or_else(|| panic!("Response missing taskID field: {}", body));
    wait_for_task_authed(client, addr, task_id, api_key).await;
}

// ── Replication test helpers ──────────────────────────────────────────────────

/// Build an Axum router with core write/read handlers + all internal replication
/// endpoints. No auth, no analytics, no QS. Used by replication test helpers.
///
/// Internal routes mounted: /internal/replicate, /internal/ops,
/// /internal/status, /internal/cluster/status, /internal/analytics-rollup.
fn build_node_router(state: Arc<flapjack_http::handlers::AppState>) -> Router {
    let health = Router::new()
        .route("/health", get(flapjack_http::handlers::health))
        .with_state(state.clone());

    let internal = Router::new()
        .route(
            "/internal/replicate",
            post(flapjack_http::handlers::internal::replicate_ops),
        )
        .route(
            "/internal/ops",
            get(flapjack_http::handlers::internal::get_ops),
        )
        .route(
            "/internal/status",
            get(flapjack_http::handlers::internal::replication_status),
        )
        .route(
            "/internal/cluster/status",
            get(flapjack_http::handlers::internal::cluster_status),
        )
        .route(
            "/internal/analytics-rollup",
            post(flapjack_http::handlers::internal::receive_analytics_rollup),
        )
        .route(
            "/internal/rollup-cache",
            get(flapjack_http::handlers::internal::rollup_cache_status),
        )
        .with_state(state.clone());

    let docs = Router::new()
        .route(
            "/1/indexes",
            post(flapjack_http::handlers::create_index).get(flapjack_http::handlers::list_indices),
        )
        .route(
            "/1/indexes/:indexName/batch",
            post(flapjack_http::handlers::add_documents),
        )
        .route(
            "/1/indexes/:indexName/query",
            post(flapjack_http::handlers::search),
        )
        .route(
            "/1/indexes/:indexName/:objectID",
            get(flapjack_http::handlers::get_object)
                .delete(flapjack_http::handlers::delete_object)
                .put(flapjack_http::handlers::put_object),
        )
        .route("/1/tasks/:task_id", get(flapjack_http::handlers::get_task))
        .with_state(state);

    Router::new().merge(health).merge(internal).merge(docs)
}

/// Spawn a standalone server with all write endpoints + internal replication
/// endpoints. The replication_manager is None (standalone mode), but /internal/ops
/// still serves oplog entries — used for startup catch-up testing.
///
/// The `_node_id` parameter is unused for now but makes call-sites self-documenting.
#[allow(dead_code)]
pub async fn spawn_server_with_internal(_node_id: &str) -> (String, TempDir) {
    let temp_dir = TempDir::new().unwrap();
    let manager = flapjack::IndexManager::new(temp_dir.path());

    let state = Arc::new(flapjack_http::handlers::AppState {
        manager,
        key_store: None,
        replication_manager: None,
        ssl_manager: None,
        analytics_engine: None,
        experiment_store: None,
        metrics_state: None,
        usage_counters: std::sync::Arc::new(dashmap::DashMap::new()),
        paused_indexes: flapjack_http::pause_registry::PausedIndexes::new(),
        start_time: std::time::Instant::now(),
    });

    let app = build_node_router(state);
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap().to_string();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let client = reqwest::Client::new();
    for _ in 0..100 {
        if client
            .get(format!("http://{}/health", addr))
            .send()
            .await
            .is_ok()
        {
            break;
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(5)).await;
    }

    (addr, temp_dir)
}

/// Spawn two fully-wired replication nodes. Each node knows about the other and
/// replicates writes bidirectionally. Both serve all write endpoints + internal
/// replication endpoints.
///
/// Returns `(addr_a, addr_b, tmp_dir_a, tmp_dir_b)`. The TempDirs must be kept
/// alive for the duration of the test (dropped when the binding goes out of scope).
///
/// Example:
/// ```no_run
/// let (addr_a, addr_b, _tmp_a, _tmp_b) =
///     common::spawn_replication_pair("node-a", "node-b").await;
/// ```
#[allow(dead_code)]
pub async fn spawn_replication_pair(
    node_a_id: &str,
    node_b_id: &str,
) -> (String, String, TempDir, TempDir) {
    use flapjack_replication::{
        config::{NodeConfig, PeerConfig},
        manager::ReplicationManager,
    };

    // Bind both listeners first so we know the addresses before starting either server.
    let listener_a = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr_a = listener_a.local_addr().unwrap();
    let listener_b = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr_b = listener_b.local_addr().unwrap();

    let tmp_a = TempDir::new().unwrap();
    let tmp_b = TempDir::new().unwrap();

    // Each node's ReplicationManager points to the other.
    let repl_a = ReplicationManager::new(NodeConfig {
        node_id: node_a_id.to_string(),
        bind_addr: addr_a.to_string(),
        peers: vec![PeerConfig {
            node_id: node_b_id.to_string(),
            addr: format!("http://{}", addr_b),
        }],
    });
    let repl_b = ReplicationManager::new(NodeConfig {
        node_id: node_b_id.to_string(),
        bind_addr: addr_b.to_string(),
        peers: vec![PeerConfig {
            node_id: node_a_id.to_string(),
            addr: format!("http://{}", addr_a),
        }],
    });

    let state_a = Arc::new(flapjack_http::handlers::AppState {
        manager: flapjack::IndexManager::new(tmp_a.path()),
        key_store: None,
        replication_manager: Some(repl_a),
        ssl_manager: None,
        analytics_engine: None,
        experiment_store: None,
        metrics_state: None,
        usage_counters: std::sync::Arc::new(dashmap::DashMap::new()),
        paused_indexes: flapjack_http::pause_registry::PausedIndexes::new(),
        start_time: std::time::Instant::now(),
    });
    let state_b = Arc::new(flapjack_http::handlers::AppState {
        manager: flapjack::IndexManager::new(tmp_b.path()),
        key_store: None,
        replication_manager: Some(repl_b),
        ssl_manager: None,
        analytics_engine: None,
        experiment_store: None,
        metrics_state: None,
        usage_counters: std::sync::Arc::new(dashmap::DashMap::new()),
        paused_indexes: flapjack_http::pause_registry::PausedIndexes::new(),
        start_time: std::time::Instant::now(),
    });

    let app_a = build_node_router(state_a);
    let app_b = build_node_router(state_b);

    let addr_a_str = addr_a.to_string();
    let addr_b_str = addr_b.to_string();

    tokio::spawn(async move {
        axum::serve(listener_a, app_a).await.unwrap();
    });
    tokio::spawn(async move {
        axum::serve(listener_b, app_b).await.unwrap();
    });

    // Wait for both servers to be ready.
    let client = reqwest::Client::new();
    for _ in 0..200 {
        let a_ok = client
            .get(format!("http://{}/health", addr_a_str))
            .send()
            .await
            .is_ok();
        let b_ok = client
            .get(format!("http://{}/health", addr_b_str))
            .send()
            .await
            .is_ok();
        if a_ok && b_ok {
            break;
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(5)).await;
    }

    (addr_a_str, addr_b_str, tmp_a, tmp_b)
}
