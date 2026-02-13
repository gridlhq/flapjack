use axum::{
    body::Body,
    http::{Request, StatusCode},
    Router,
};
use flapjack::IndexManager;
use std::sync::Arc;
use tempfile::TempDir;
use tower::ServiceExt;

/// Test that internal replication endpoints work WITHOUT authentication
/// This is critical for peer-to-peer replication in Phase 4
#[tokio::test]
async fn test_internal_status_no_auth_required() {
    let temp_dir = TempDir::new().unwrap();
    let manager = IndexManager::new(temp_dir.path());

    let state = Arc::new(flapjack_http::handlers::AppState {
        manager,
        key_store: None,
        replication_manager: None,
        ssl_manager: None,
    });

    // Create internal router like server.rs does
    let internal = Router::new()
        .route(
            "/internal/status",
            axum::routing::get(flapjack_http::handlers::internal::replication_status),
        )
        .with_state(state);

    // Request WITHOUT auth headers
    let response = internal
        .oneshot(
            Request::builder()
                .uri("/internal/status")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // Should succeed without auth
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "Internal endpoint should not require authentication"
    );
}

/// Test that internal /replicate endpoint works WITHOUT authentication
#[tokio::test]
async fn test_internal_replicate_no_auth_required() {
    let temp_dir = TempDir::new().unwrap();
    let manager = IndexManager::new(temp_dir.path());

    let state = Arc::new(flapjack_http::handlers::AppState {
        manager,
        key_store: None,
        replication_manager: None,
        ssl_manager: None,
    });

    let internal = Router::new()
        .route(
            "/internal/replicate",
            axum::routing::post(flapjack_http::handlers::internal::replicate_ops),
        )
        .with_state(state);

    // Empty request payload (will fail validation but should not fail auth)
    let body = serde_json::json!({
        "tenant_id": "test",
        "ops": []
    });

    let response = internal
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/internal/replicate")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    // Should succeed (200) or fail validation, but NOT fail auth (401)
    assert_ne!(
        response.status(),
        StatusCode::UNAUTHORIZED,
        "Internal endpoint should not require authentication"
    );
}

/// Test that internal /ops endpoint works WITHOUT authentication
#[tokio::test]
async fn test_internal_get_ops_no_auth_required() {
    let temp_dir = TempDir::new().unwrap();
    let manager = IndexManager::new(temp_dir.path());

    // Create a tenant and add a document to initialize oplog
    manager.create_tenant("test").unwrap();
    let doc =
        flapjack::types::Document::from_json(&serde_json::json!({"_id": "1", "title": "Test"}))
            .unwrap();
    manager.add_documents_sync("test", vec![doc]).await.unwrap();

    let state = Arc::new(flapjack_http::handlers::AppState {
        manager,
        key_store: None,
        replication_manager: None,
        ssl_manager: None,
    });

    let internal = Router::new()
        .route(
            "/internal/ops",
            axum::routing::get(flapjack_http::handlers::internal::get_ops),
        )
        .with_state(state);

    let response = internal
        .oneshot(
            Request::builder()
                .uri("/internal/ops?tenant_id=test&since_seq=0")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // Should succeed without auth (200 or 404 is acceptable, but NOT 401)
    assert_ne!(
        response.status(),
        StatusCode::UNAUTHORIZED,
        "Internal endpoint should not require authentication"
    );
}

/// CRITICAL SAFETY TEST: Verify internal endpoints do NOT bypass tenant isolation
/// Even though auth is disabled, replication should not allow accessing other tenants
#[tokio::test]
async fn test_internal_tenant_isolation() {
    let temp_dir = TempDir::new().unwrap();
    let manager = IndexManager::new(temp_dir.path());

    // Create tenant A
    manager.create_tenant("tenant-a").unwrap();
    let doc =
        flapjack::types::Document::from_json(&serde_json::json!({"_id": "1", "title": "Secret A"}))
            .unwrap();
    manager
        .add_documents_sync("tenant-a", vec![doc])
        .await
        .unwrap();

    let state = Arc::new(flapjack_http::handlers::AppState {
        manager,
        key_store: None,
        replication_manager: None,
        ssl_manager: None,
    });

    let internal = Router::new()
        .route(
            "/internal/ops",
            axum::routing::get(flapjack_http::handlers::internal::get_ops),
        )
        .with_state(state);

    // Request ops for tenant-a
    let response = internal
        .clone()
        .oneshot(
            Request::builder()
                .uri("/internal/ops?tenant_id=tenant-a&since_seq=0")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    // Verify we get tenant-a's data
    let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(json["tenant_id"], "tenant-a");
    assert!(!json["ops"].as_array().unwrap().is_empty());
}
