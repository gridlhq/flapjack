use axum::{middleware, routing::get, Router};
use std::sync::Arc;
use tempfile::TempDir;
use tokio::net::TcpListener;
use tower_http::cors::CorsLayer;

mod common;

/// Spawn a server with the same CORS + middleware layers as production.
async fn spawn_server_with_cors() -> (String, TempDir) {
    let temp_dir = TempDir::new().unwrap();
    let manager = flapjack::IndexManager::new(temp_dir.path());

    let state = Arc::new(flapjack_http::handlers::AppState {
        manager,
        key_store: None,
        replication_manager: None,
        ssl_manager: None,
    });

    let app = Router::new()
        .route("/health", get(flapjack_http::handlers::health))
        .route(
            "/1/indexes/:indexName/query",
            axum::routing::post(flapjack_http::handlers::search),
        )
        .with_state(state)
        .layer(middleware::from_fn(
            flapjack_http::middleware::normalize_content_type,
        ))
        .layer(CorsLayer::very_permissive().max_age(std::time::Duration::from_secs(86400)))
        .layer(middleware::from_fn(
            flapjack_http::middleware::allow_private_network,
        ));

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap().to_string();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    (addr, temp_dir)
}

#[tokio::test]
async fn test_cors_preflight_returns_max_age() {
    let (addr, _temp) = spawn_server_with_cors().await;
    let client = reqwest::Client::new();

    let response = client
        .request(
            reqwest::Method::OPTIONS,
            format!("http://{}/1/indexes/test/query", addr),
        )
        .header("Origin", "https://demo.flapjack.foo")
        .header("Access-Control-Request-Method", "POST")
        .header(
            "Access-Control-Request-Headers",
            "content-type, x-algolia-api-key, x-algolia-application-id",
        )
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);

    let max_age = response
        .headers()
        .get("access-control-max-age")
        .expect("Missing Access-Control-Max-Age header");
    assert_eq!(max_age, "86400", "Max-Age should be 86400 (24 hours)");

    assert!(
        response
            .headers()
            .get("access-control-allow-origin")
            .is_some(),
        "Missing Access-Control-Allow-Origin"
    );
    assert!(
        response
            .headers()
            .get("access-control-allow-methods")
            .is_some(),
        "Missing Access-Control-Allow-Methods"
    );
    assert!(
        response
            .headers()
            .get("access-control-allow-headers")
            .is_some(),
        "Missing Access-Control-Allow-Headers"
    );
}

#[tokio::test]
async fn test_cors_regular_post_has_allow_origin() {
    let (addr, _temp) = spawn_server_with_cors().await;
    let client = reqwest::Client::new();

    // Search on a non-existent index returns 404, but CORS headers should still be present
    let response = client
        .post(format!("http://{}/1/indexes/test/query", addr))
        .header("Origin", "https://demo.flapjack.foo")
        .header("x-algolia-api-key", "test")
        .header("x-algolia-application-id", "test")
        .json(&serde_json::json!({"query": "hello"}))
        .send()
        .await
        .unwrap();

    assert!(
        response
            .headers()
            .get("access-control-allow-origin")
            .is_some(),
        "POST response should include Access-Control-Allow-Origin"
    );
}

#[tokio::test]
async fn test_cors_private_network_access() {
    let (addr, _temp) = spawn_server_with_cors().await;
    let client = reqwest::Client::new();

    let response = client
        .request(
            reqwest::Method::OPTIONS,
            format!("http://{}/1/indexes/test/query", addr),
        )
        .header("Origin", "https://demo.flapjack.foo")
        .header("Access-Control-Request-Method", "POST")
        .header("Access-Control-Request-Private-Network", "true")
        .send()
        .await
        .unwrap();

    let pna = response
        .headers()
        .get("access-control-allow-private-network")
        .expect("Missing Access-Control-Allow-Private-Network header");
    assert_eq!(pna, "true");
}
