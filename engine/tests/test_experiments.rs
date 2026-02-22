use axum::{
    body::Body,
    http::{Method, Request, StatusCode},
    routing::{get, post},
    Router,
};
use flapjack::IndexManager;
use serde_json::{json, Value};
use std::sync::Arc;
use tempfile::TempDir;
use tower::ServiceExt;

fn make_state(tmp: &TempDir) -> Arc<flapjack_http::handlers::AppState> {
    Arc::new(flapjack_http::handlers::AppState {
        manager: IndexManager::new(tmp.path()),
        key_store: None,
        replication_manager: None,
        ssl_manager: None,
        analytics_engine: None,
        experiment_store: Some(Arc::new(
            flapjack::experiments::store::ExperimentStore::new(tmp.path()).unwrap(),
        )),
        metrics_state: None,
        usage_counters: Arc::new(dashmap::DashMap::new()),
        paused_indexes: flapjack_http::pause_registry::PausedIndexes::new(),
        start_time: std::time::Instant::now(),
    })
}

fn app_router(state: Arc<flapjack_http::handlers::AppState>) -> Router {
    Router::new()
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
        .with_state(state)
}

fn create_experiment_body(index_name: &str) -> Value {
    json!({
        "name": format!("Ranking Test {index_name}"),
        "indexName": index_name,
        "trafficSplit": 0.5,
        "control": {
            "name": "control"
        },
        "variant": {
            "name": "variant",
            "queryOverrides": {
                "enableSynonyms": false
            }
        },
        "primaryMetric": "ctr",
        "minimumDays": 14
    })
}

async fn send_json_request(
    app: &Router,
    method: Method,
    uri: &str,
    body: Value,
) -> axum::http::Response<Body> {
    app.clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(uri)
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap()
}

async fn send_empty_request(app: &Router, method: Method, uri: &str) -> axum::http::Response<Body> {
    app.clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(uri)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
}

async fn body_json(resp: axum::http::Response<Body>) -> Value {
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

async fn create_experiment(app: &Router, index_name: &str) -> Value {
    let response = send_json_request(
        app,
        Method::POST,
        "/2/abtests",
        create_experiment_body(index_name),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    body_json(response).await
}

#[tokio::test]
async fn test_create_and_get_experiment() {
    let tmp = TempDir::new().unwrap();
    let app = app_router(make_state(&tmp));

    let created = create_experiment(&app, "products").await;
    let experiment_id = created["id"].as_str().unwrap().to_string();

    let response =
        send_empty_request(&app, Method::GET, &format!("/2/abtests/{experiment_id}")).await;
    assert_eq!(response.status(), StatusCode::OK);
    let fetched = body_json(response).await;
    assert_eq!(fetched["id"], experiment_id);
    assert_eq!(fetched["status"], "draft");
}

#[tokio::test]
async fn test_list_experiments() {
    let tmp = TempDir::new().unwrap();
    let app = app_router(make_state(&tmp));

    create_experiment(&app, "products").await;

    let response = send_empty_request(&app, Method::GET, "/2/abtests").await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = body_json(response).await;
    let abtests = body["abtests"].as_array().unwrap();
    assert_eq!(abtests.len(), 1);
    assert_eq!(body["count"], 1);
    assert_eq!(body["total"], 1);
}

#[tokio::test]
async fn test_list_experiments_filters_by_index() {
    let tmp = TempDir::new().unwrap();
    let app = app_router(make_state(&tmp));

    create_experiment(&app, "products").await;
    create_experiment(&app, "products_alt").await;

    let response = send_empty_request(&app, Method::GET, "/2/abtests?index=products").await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = body_json(response).await;
    let abtests = body["abtests"].as_array().unwrap();
    assert_eq!(abtests.len(), 1);
    assert_eq!(body["count"], 1);
    assert_eq!(body["total"], 1);
    assert_eq!(abtests[0]["indexName"], "products");
}

#[tokio::test]
async fn test_start_stop_lifecycle() {
    let tmp = TempDir::new().unwrap();
    let app = app_router(make_state(&tmp));

    let created = create_experiment(&app, "products").await;
    let experiment_id = created["id"].as_str().unwrap();

    let start_response = send_empty_request(
        &app,
        Method::POST,
        &format!("/2/abtests/{experiment_id}/start"),
    )
    .await;
    assert_eq!(start_response.status(), StatusCode::OK);
    let started = body_json(start_response).await;
    assert_eq!(started["status"], "running");

    let stop_response = send_empty_request(
        &app,
        Method::POST,
        &format!("/2/abtests/{experiment_id}/stop"),
    )
    .await;
    assert_eq!(stop_response.status(), StatusCode::OK);
    let stopped = body_json(stop_response).await;
    assert_eq!(stopped["status"], "stopped");
}

#[tokio::test]
async fn test_delete_draft() {
    let tmp = TempDir::new().unwrap();
    let app = app_router(make_state(&tmp));

    let created = create_experiment(&app, "products").await;
    let experiment_id = created["id"].as_str().unwrap();

    let delete_response =
        send_empty_request(&app, Method::DELETE, &format!("/2/abtests/{experiment_id}")).await;
    assert_eq!(delete_response.status(), StatusCode::NO_CONTENT);

    let get_response =
        send_empty_request(&app, Method::GET, &format!("/2/abtests/{experiment_id}")).await;
    assert_eq!(get_response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_results_response_structure() {
    let tmp = TempDir::new().unwrap();
    let app = app_router(make_state(&tmp));

    let created = create_experiment(&app, "products").await;
    let experiment_id = created["id"].as_str().unwrap();

    let response = send_empty_request(
        &app,
        Method::GET,
        &format!("/2/abtests/{experiment_id}/results"),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = body_json(response).await;
    assert_eq!(body["experimentID"], experiment_id);
    assert_eq!(body["name"], "Ranking Test products");
    assert_eq!(body["status"], "draft");
    assert_eq!(body["indexName"], "products");
    assert!(body["gate"].is_object());
    assert_eq!(body["gate"]["readyToRead"], false);
    assert_eq!(body["control"]["searches"], 0);
    assert_eq!(body["variant"]["searches"], 0);
    assert!(body["significance"].is_null());
    assert_eq!(body["sampleRatioMismatch"], false);
}
