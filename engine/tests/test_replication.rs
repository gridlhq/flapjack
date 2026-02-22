//! Consolidated replication tests.
//!
//! Merged from (all deleted):
//!   - test_replication_internal.rs  (no-auth enforcement for internal endpoints)
//!   - test_replication_safety.rs    (oplog integrity: body, delete payload, sequences)
//!   - test_replication_phase5.rs    (apply_ops helper, cluster/status, startup catch-up, peer_statuses)
//!
//! Two-node E2E tests (closes known gap #6 from TESTING.md):
//!   test_two_node_write_replicates_to_peer
//!   test_two_node_delete_propagates_to_peer
//!   test_two_node_bidirectional_replication
//!   test_two_node_startup_catchup_via_get_ops
//!
//! Phase 4 analytics rollup exchange tests:
//!   test_analytics_rollup_exchange_endpoint_accepts_rollup
//!
//! Phase 4b rollup broadcaster tests:
//!   test_rollup_cache_status_endpoint_empty
//!   test_rollup_cache_status_reflects_stored_rollup
//!   test_run_rollup_broadcast_sends_to_peer
//!   test_rollup_broadcaster_integration_periodic

mod common;

use flapjack::types::Document;
use flapjack::IndexManager;
use std::sync::Arc;
use tempfile::TempDir;

// ============================================================
// From test_replication_internal.rs
// Internal endpoints must work WITHOUT authentication
// ============================================================

// test_internal_status_no_auth_required removed — redundant with smoke_internal_endpoint in test_smoke.rs

#[tokio::test]
async fn test_internal_replicate_no_auth_required() {
    use axum::{
        body::Body,
        http::{Request, StatusCode},
        Router,
    };
    use tower::ServiceExt;

    let temp_dir = TempDir::new().unwrap();
    let manager = IndexManager::new(temp_dir.path());

    let state = Arc::new(flapjack_http::handlers::AppState {
        manager,
        key_store: None,
        replication_manager: None,
        ssl_manager: None,
        analytics_engine: None,
        metrics_state: None,
        usage_counters: std::sync::Arc::new(dashmap::DashMap::new()),
        paused_indexes: flapjack_http::pause_registry::PausedIndexes::new(),
        start_time: std::time::Instant::now(),
        experiment_store: None,
    });

    let internal = Router::new()
        .route(
            "/internal/replicate",
            axum::routing::post(flapjack_http::handlers::internal::replicate_ops),
        )
        .with_state(state);

    let body = serde_json::json!({"tenant_id": "test", "ops": []});

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

    assert_eq!(
        response.status(),
        StatusCode::OK,
        "/internal/replicate should return 200 OK without authentication"
    );
}

#[tokio::test]
async fn test_internal_get_ops_no_auth_required() {
    use axum::{
        body::Body,
        http::{Request, StatusCode},
        Router,
    };
    use tower::ServiceExt;

    let temp_dir = TempDir::new().unwrap();
    let manager = IndexManager::new(temp_dir.path());

    manager.create_tenant("test").unwrap();
    let doc = Document::from_json(&serde_json::json!({"_id": "1", "title": "Test"})).unwrap();
    manager.add_documents_sync("test", vec![doc]).await.unwrap();

    let state = Arc::new(flapjack_http::handlers::AppState {
        manager,
        key_store: None,
        replication_manager: None,
        ssl_manager: None,
        analytics_engine: None,
        metrics_state: None,
        usage_counters: std::sync::Arc::new(dashmap::DashMap::new()),
        paused_indexes: flapjack_http::pause_registry::PausedIndexes::new(),
        start_time: std::time::Instant::now(),
        experiment_store: None,
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

    assert_eq!(
        response.status(),
        StatusCode::OK,
        "/internal/ops should return 200 OK without authentication"
    );
}

#[tokio::test]
async fn test_internal_tenant_isolation() {
    use axum::{
        body::Body,
        http::{Request, StatusCode},
        Router,
    };
    use tower::ServiceExt;

    let temp_dir = TempDir::new().unwrap();
    let manager = IndexManager::new(temp_dir.path());

    manager.create_tenant("tenant-a").unwrap();
    let doc = Document::from_json(&serde_json::json!({"_id": "1", "title": "Secret A"})).unwrap();
    manager
        .add_documents_sync("tenant-a", vec![doc])
        .await
        .unwrap();

    let state = Arc::new(flapjack_http::handlers::AppState {
        manager,
        key_store: None,
        replication_manager: None,
        ssl_manager: None,
        analytics_engine: None,
        metrics_state: None,
        usage_counters: std::sync::Arc::new(dashmap::DashMap::new()),
        paused_indexes: flapjack_http::pause_registry::PausedIndexes::new(),
        start_time: std::time::Instant::now(),
        experiment_store: None,
    });

    let internal = Router::new()
        .route(
            "/internal/ops",
            axum::routing::get(flapjack_http::handlers::internal::get_ops),
        )
        .with_state(state);

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

    let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(json["tenant_id"], "tenant-a");
    assert!(!json["ops"].as_array().unwrap().is_empty());
}

// ============================================================
// From test_replication_safety.rs
// Oplog integrity: full body, delete payload, monotonic seqs
// ============================================================

#[tokio::test]
async fn test_oplog_contains_full_document_body() {
    let temp_dir = TempDir::new().unwrap();
    let manager = IndexManager::new(temp_dir.path());

    manager.create_tenant("test").unwrap();
    let doc = Document::from_json(
        &serde_json::json!({"_id": "1", "title": "Test Document", "price": 99}),
    )
    .unwrap();
    manager.add_documents_sync("test", vec![doc]).await.unwrap();

    let oplog = manager
        .get_or_create_oplog("test")
        .expect("OpLog should exist");
    let ops = oplog.read_since(0).unwrap();

    assert!(!ops.is_empty(), "OpLog should have at least one entry");
    let first_op = &ops[0];
    assert_eq!(first_op.op_type, "upsert", "First op should be an upsert");

    let body = first_op
        .payload
        .get("body")
        .expect("Payload should have 'body' field");
    assert_eq!(
        body.get("title").and_then(|v| v.as_str()),
        Some("Test Document")
    );
    assert_eq!(body.get("price").and_then(|v| v.as_u64()), Some(99));
}

#[tokio::test]
async fn test_oplog_delete_includes_object_id() {
    let temp_dir = TempDir::new().unwrap();
    let manager = IndexManager::new(temp_dir.path());

    manager.create_tenant("test").unwrap();
    let doc =
        Document::from_json(&serde_json::json!({"_id": "delete-me", "title": "Temp"})).unwrap();
    manager.add_documents_sync("test", vec![doc]).await.unwrap();
    manager
        .delete_documents_sync("test", vec!["delete-me".to_string()])
        .await
        .unwrap();

    let oplog = manager
        .get_or_create_oplog("test")
        .expect("OpLog should exist");
    let ops = oplog.read_since(0).unwrap();

    let delete_op = ops
        .iter()
        .find(|op| op.op_type == "delete")
        .expect("Should have a delete operation in oplog");

    assert!(
        delete_op.payload.get("objectID").is_some(),
        "Delete operation MUST include objectID"
    );
    assert_eq!(
        delete_op.payload.get("objectID").and_then(|v| v.as_str()),
        Some("delete-me")
    );
}

#[tokio::test]
async fn test_concurrent_tenant_oplog_isolation() {
    let temp_dir = TempDir::new().unwrap();
    let manager = Arc::new(IndexManager::new(temp_dir.path()));

    manager.create_tenant("tenant-a").unwrap();
    manager.create_tenant("tenant-b").unwrap();

    let mgr_a = Arc::clone(&manager);
    let mgr_b = Arc::clone(&manager);

    let handle_a = tokio::spawn(async move {
        for i in 0..10 {
            let doc =
                Document::from_json(&serde_json::json!({"_id": format!("a-{}", i), "tenant": "A"}))
                    .unwrap();
            mgr_a
                .add_documents_sync("tenant-a", vec![doc])
                .await
                .unwrap();
        }
    });

    let handle_b = tokio::spawn(async move {
        for i in 0..10 {
            let doc =
                Document::from_json(&serde_json::json!({"_id": format!("b-{}", i), "tenant": "B"}))
                    .unwrap();
            mgr_b
                .add_documents_sync("tenant-b", vec![doc])
                .await
                .unwrap();
        }
    });

    handle_a.await.unwrap();
    handle_b.await.unwrap();

    let ops_a = manager
        .get_or_create_oplog("tenant-a")
        .unwrap()
        .read_since(0)
        .unwrap();
    let ops_b = manager
        .get_or_create_oplog("tenant-b")
        .unwrap()
        .read_since(0)
        .unwrap();

    assert_eq!(
        ops_a.len(),
        10,
        "Tenant A should have exactly 10 operations"
    );
    assert_eq!(
        ops_b.len(),
        10,
        "Tenant B should have exactly 10 operations"
    );

    for op in &ops_a {
        if let Some(body) = op.payload.get("body") {
            assert_eq!(
                body.get("tenant").and_then(|v| v.as_str()),
                Some("A"),
                "Tenant A oplog should only contain tenant A documents"
            );
        }
    }
    for op in &ops_b {
        if let Some(body) = op.payload.get("body") {
            assert_eq!(
                body.get("tenant").and_then(|v| v.as_str()),
                Some("B"),
                "Tenant B oplog should only contain tenant B documents"
            );
        }
    }
}

#[tokio::test]
async fn test_oplog_sequence_numbers_monotonic() {
    let temp_dir = TempDir::new().unwrap();
    let manager = IndexManager::new(temp_dir.path());

    manager.create_tenant("test").unwrap();
    for i in 1..=5 {
        let doc =
            Document::from_json(&serde_json::json!({"_id": i.to_string(), "value": i})).unwrap();
        manager.add_documents_sync("test", vec![doc]).await.unwrap();
    }

    let ops = manager
        .get_or_create_oplog("test")
        .unwrap()
        .read_since(0)
        .unwrap();
    assert_eq!(ops.len(), 5, "Should have 5 operations");

    let mut prev_seq = 0u64;
    for (idx, op) in ops.iter().enumerate() {
        assert!(
            op.seq > prev_seq,
            "Sequence {} (op {}) should be greater than previous {}",
            op.seq,
            idx,
            prev_seq
        );
        prev_seq = op.seq;
    }
}

#[tokio::test]
async fn test_oplog_read_since_boundary() {
    let temp_dir = TempDir::new().unwrap();
    let manager = IndexManager::new(temp_dir.path());

    manager.create_tenant("test").unwrap();
    for i in 1..=10 {
        let doc =
            Document::from_json(&serde_json::json!({"_id": i.to_string(), "value": i})).unwrap();
        manager.add_documents_sync("test", vec![doc]).await.unwrap();
    }

    let oplog = manager.get_or_create_oplog("test").unwrap();
    let all_ops = oplog.read_since(0).unwrap();
    assert_eq!(all_ops.len(), 10);

    let fifth_seq = all_ops[4].seq;
    let ops_after = oplog.read_since(fifth_seq).unwrap();

    assert_eq!(
        ops_after.len(),
        5,
        "read_since({}) should return 5 remaining ops, not {}",
        fifth_seq,
        ops_after.len()
    );
    for op in &ops_after {
        assert!(
            op.seq > fifth_seq,
            "read_since returned op with seq {} <= {}",
            op.seq,
            fifth_seq
        );
    }
}

#[tokio::test]
async fn test_replicate_ops_handler_applies_correctly() {
    use axum::{
        body::Body,
        http::{Request, StatusCode},
        Router,
    };
    use tower::ServiceExt;

    let temp_dir = TempDir::new().unwrap();
    let manager = IndexManager::new(temp_dir.path());

    let state = Arc::new(flapjack_http::handlers::AppState {
        manager: manager.clone(),
        key_store: None,
        replication_manager: None,
        ssl_manager: None,
        analytics_engine: None,
        metrics_state: None,
        usage_counters: std::sync::Arc::new(dashmap::DashMap::new()),
        paused_indexes: flapjack_http::pause_registry::PausedIndexes::new(),
        start_time: std::time::Instant::now(),
        experiment_store: None,
    });

    let internal = Router::new()
        .route(
            "/internal/replicate",
            axum::routing::post(flapjack_http::handlers::internal::replicate_ops),
        )
        .with_state(state);

    let req_body = serde_json::json!({
        "tenant_id": "test",
        "ops": [
            {"seq": 1, "timestamp_ms": 1000, "node_id": "node-a", "tenant_id": "test", "op_type": "upsert", "payload": {"body": {"_id": "1", "title": "First"}}},
            {"seq": 2, "timestamp_ms": 2000, "node_id": "node-a", "tenant_id": "test", "op_type": "upsert", "payload": {"body": {"_id": "2", "title": "Second"}}},
            {"seq": 3, "timestamp_ms": 3000, "node_id": "node-a", "tenant_id": "test", "op_type": "delete", "payload": {"objectID": "1"}}
        ]
    });

    let response = internal
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/internal/replicate")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&req_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        StatusCode::OK,
        "Replication should succeed"
    );

    assert_eq!(
        manager
            .search("test", "First", None, None, 10)
            .unwrap()
            .total,
        0,
        "Document 1 should be deleted"
    );
    assert_eq!(
        manager
            .search("test", "Second", None, None, 10)
            .unwrap()
            .total,
        1,
        "Document 2 should exist"
    );
}

// ============================================================
// From test_replication_phase5.rs
// apply_ops_to_manager helper, cluster/status, startup catch-up, peer_statuses
// ============================================================

#[tokio::test]
async fn test_apply_ops_to_manager_upsert() {
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use axum::Router;
    use tower::ServiceExt;

    let temp = TempDir::new().unwrap();
    let manager = IndexManager::new(temp.path());

    let state = Arc::new(flapjack_http::handlers::AppState {
        manager: manager.clone(),
        key_store: None,
        replication_manager: None,
        ssl_manager: None,
        analytics_engine: None,
        metrics_state: None,
        usage_counters: std::sync::Arc::new(dashmap::DashMap::new()),
        paused_indexes: flapjack_http::pause_registry::PausedIndexes::new(),
        start_time: std::time::Instant::now(),
        experiment_store: None,
    });

    let router = Router::new()
        .route(
            "/internal/replicate",
            axum::routing::post(flapjack_http::handlers::internal::replicate_ops),
        )
        .with_state(state);

    let body = serde_json::json!({
        "tenant_id": "test-apply",
        "ops": [
            {
                "seq": 1, "timestamp_ms": 1000, "node_id": "node-a",
                "tenant_id": "test-apply", "op_type": "upsert",
                "payload": {"body": {"_id": "doc1", "title": "Alpha"}}
            },
            {
                "seq": 2, "timestamp_ms": 2000, "node_id": "node-a",
                "tenant_id": "test-apply", "op_type": "upsert",
                "payload": {"body": {"_id": "doc2", "title": "Beta"}}
            }
        ]
    });

    let resp = router
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

    assert_eq!(resp.status(), StatusCode::OK);

    // Poll until the write queue processes and commits the documents.
    for _ in 0..500 {
        if manager
            .search("test-apply", "", None, None, 10)
            .map(|r| r.total)
            .unwrap_or(0)
            >= 2
        {
            break;
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    }

    let results = manager
        .search("test-apply", "Alpha", None, None, 10)
        .unwrap();
    assert_eq!(results.total, 1, "doc1 should be indexed");
    let results2 = manager
        .search("test-apply", "Beta", None, None, 10)
        .unwrap();
    assert_eq!(results2.total, 1, "doc2 should be indexed");
}

// test_apply_ops_to_manager_returns_max_seq removed — redundant with apply_ops_returns_max_seq in internal.rs
// test_apply_ops_empty_batch_is_ok removed — redundant with apply_ops_unknown_type_skipped + direct tests in internal.rs

#[tokio::test]
async fn test_cluster_status_no_replication() {
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use axum::Router;
    use tower::ServiceExt;

    let temp = TempDir::new().unwrap();
    let manager = IndexManager::new(temp.path());

    let state = Arc::new(flapjack_http::handlers::AppState {
        manager: manager.clone(),
        key_store: None,
        replication_manager: None,
        ssl_manager: None,
        analytics_engine: None,
        metrics_state: None,
        usage_counters: std::sync::Arc::new(dashmap::DashMap::new()),
        paused_indexes: flapjack_http::pause_registry::PausedIndexes::new(),
        start_time: std::time::Instant::now(),
        experiment_store: None,
    });

    let router = Router::new()
        .route(
            "/internal/cluster/status",
            axum::routing::get(flapjack_http::handlers::internal::cluster_status),
        )
        .with_state(state);

    let resp = router
        .oneshot(
            Request::builder()
                .uri("/internal/cluster/status")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);

    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json["replication_enabled"], false);
    assert!(json["peers"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_cluster_status_with_peers() {
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use axum::Router;
    use flapjack_replication::config::{NodeConfig, PeerConfig};
    use flapjack_replication::manager::ReplicationManager;
    use tower::ServiceExt;

    let temp = TempDir::new().unwrap();
    let manager = IndexManager::new(temp.path());

    let repl_mgr = ReplicationManager::new(NodeConfig {
        node_id: "node-a".to_string(),
        bind_addr: "0.0.0.0:7700".to_string(),
        peers: vec![
            PeerConfig {
                node_id: "node-b".to_string(),
                addr: "http://node-b:7700".to_string(),
            },
            PeerConfig {
                node_id: "node-c".to_string(),
                addr: "http://node-c:7700".to_string(),
            },
        ],
    });

    let state = Arc::new(flapjack_http::handlers::AppState {
        manager: manager.clone(),
        key_store: None,
        replication_manager: Some(repl_mgr),
        ssl_manager: None,
        analytics_engine: None,
        metrics_state: None,
        usage_counters: std::sync::Arc::new(dashmap::DashMap::new()),
        paused_indexes: flapjack_http::pause_registry::PausedIndexes::new(),
        start_time: std::time::Instant::now(),
        experiment_store: None,
    });

    let router = Router::new()
        .route(
            "/internal/cluster/status",
            axum::routing::get(flapjack_http::handlers::internal::cluster_status),
        )
        .with_state(state);

    let resp = router
        .oneshot(
            Request::builder()
                .uri("/internal/cluster/status")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);

    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();

    assert_eq!(json["node_id"], "node-a");
    assert_eq!(json["replication_enabled"], true);
    assert_eq!(json["peers_total"], 2);

    let peers = json["peers"].as_array().unwrap();
    assert_eq!(peers.len(), 2);

    assert_eq!(peers[0]["peer_id"], "node-b");
    assert_eq!(peers[0]["status"], "never_contacted");
    assert!(peers[0]["last_success_secs_ago"].is_null());

    assert_eq!(peers[1]["peer_id"], "node-c");
    assert_eq!(peers[1]["status"], "never_contacted");
}

#[tokio::test]
async fn test_startup_catchup_noop_without_replication() {
    let temp = TempDir::new().unwrap();
    let manager = IndexManager::new(temp.path());

    // Write a doc so we can verify catch-up doesn't corrupt existing data
    manager.create_tenant("existing").unwrap();
    let doc =
        Document::from_json(&serde_json::json!({"_id": "1", "title": "Pre-existing"})).unwrap();
    manager
        .add_documents_sync("existing", vec![doc])
        .await
        .unwrap();

    let state = Arc::new(flapjack_http::handlers::AppState {
        manager: manager.clone(),
        key_store: None,
        replication_manager: None, // No replication configured
        ssl_manager: None,
        analytics_engine: None,
        metrics_state: None,
        usage_counters: std::sync::Arc::new(dashmap::DashMap::new()),
        paused_indexes: flapjack_http::pause_registry::PausedIndexes::new(),
        start_time: std::time::Instant::now(),
        experiment_store: None,
    });

    // Should complete without error and not corrupt existing data
    flapjack_http::startup_catchup::run_startup_catchup(state).await;

    // Verify existing data is untouched
    let result = manager
        .search("existing", "Pre-existing", None, None, 10)
        .unwrap();
    assert_eq!(
        result.total, 1,
        "Startup catchup with no replication should not corrupt existing data"
    );
}

// test_peer_statuses_never_contacted removed — exact duplicate of test_peer_statuses_initially_never_contacted in manager.rs
// test_peer_statuses_no_peers removed — exact duplicate of test_peer_statuses_no_peers_returns_empty in manager.rs

#[tokio::test]
async fn test_apply_ops_upsert_then_delete_ordering() {
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use axum::Router;
    use tower::ServiceExt;

    let temp = TempDir::new().unwrap();
    let manager = IndexManager::new(temp.path());

    let state = Arc::new(flapjack_http::handlers::AppState {
        manager: manager.clone(),
        key_store: None,
        replication_manager: None,
        ssl_manager: None,
        analytics_engine: None,
        metrics_state: None,
        usage_counters: std::sync::Arc::new(dashmap::DashMap::new()),
        paused_indexes: flapjack_http::pause_registry::PausedIndexes::new(),
        start_time: std::time::Instant::now(),
        experiment_store: None,
    });

    let router = Router::new()
        .route(
            "/internal/replicate",
            axum::routing::post(flapjack_http::handlers::internal::replicate_ops),
        )
        .with_state(state);

    let body = serde_json::json!({
        "tenant_id": "test-order",
        "ops": [
            {"seq": 1, "timestamp_ms": 1000, "node_id": "n", "tenant_id": "test-order",
             "op_type": "upsert", "payload": {"body": {"_id": "keep", "title": "Keep this"}}},
            {"seq": 2, "timestamp_ms": 2000, "node_id": "n", "tenant_id": "test-order",
             "op_type": "upsert", "payload": {"body": {"_id": "remove", "title": "Remove this"}}},
            {"seq": 3, "timestamp_ms": 3000, "node_id": "n", "tenant_id": "test-order",
             "op_type": "delete", "payload": {"objectID": "remove"}}
        ]
    });

    let resp = router
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

    assert_eq!(resp.status(), StatusCode::OK);

    let keep = manager
        .search("test-order", "Keep", None, None, 10)
        .unwrap();
    assert_eq!(keep.total, 1, "'keep' doc should be searchable");

    let remove = manager
        .search("test-order", "Remove", None, None, 10)
        .unwrap();
    assert_eq!(remove.total, 0, "'remove' doc should be deleted");
}

// ============================================================
// Two-node E2E integration tests (closes known gap #6 in TESTING.md).
//
// All tests above use oneshot requests or direct function calls.
// These exercise the complete replication stack:
//   HTTP write → trigger_replication() → peer POST /internal/replicate
//   → peer applies ops → peer search returns result.
//
// Phase 3 note: write forwarding (proxying writes from one node to another)
// is intentionally NOT implemented. In the current full-mesh architecture:
//   1. Every node accepts writes directly.
//   2. trigger_replication() propagates every write to all peers.
//   3. A load balancer (nginx round-robin) distributes writes across nodes.
// test_two_node_bidirectional_replication below demonstrates that writes on
// any node reach all other nodes, which is what write forwarding would achieve
// at the proxy layer. Explicit write forwarding is therefore unnecessary.
// ============================================================

/// Write to node-a via HTTP; doc must appear on node-b within 2 seconds.
#[tokio::test]
async fn test_two_node_write_replicates_to_peer() {
    let (addr_a, addr_b, _tmp_a, _tmp_b) = common::spawn_replication_pair("node-a", "node-b").await;
    let client = reqwest::Client::new();

    let resp = client
        .post(format!("http://{}/1/indexes/repltest/batch", addr_a))
        .json(&serde_json::json!({
            "requests": [{"action": "addObject", "body": {"_id": "doc1", "title": "Saffron Pancakes"}}]
        }))
        .send()
        .await
        .unwrap();
    assert!(
        resp.status().is_success(),
        "batch write failed: {}",
        resp.status()
    );

    // Poll node-b for up to 2 seconds (replication is async).
    for _ in 0..200 {
        let r = client
            .post(format!("http://{}/1/indexes/repltest/query", addr_b))
            .json(&serde_json::json!({"query": "Saffron"}))
            .send()
            .await
            .unwrap()
            .json::<serde_json::Value>()
            .await
            .unwrap();
        if r["nbHits"].as_u64().unwrap_or(0) >= 1 {
            return; // replication confirmed
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    }
    panic!("'Saffron Pancakes' did not replicate from node-a to node-b within 2s");
}

/// Delete on node-a must propagate to node-b within 2 seconds.
#[tokio::test]
async fn test_two_node_delete_propagates_to_peer() {
    let (addr_a, addr_b, _tmp_a, _tmp_b) = common::spawn_replication_pair("node-a", "node-b").await;
    let client = reqwest::Client::new();

    // First write the doc and wait for it to appear on B.
    let resp = client
        .post(format!("http://{}/1/indexes/repltest/batch", addr_a))
        .json(&serde_json::json!({
            "requests": [{"action": "addObject", "body": {"_id": "to-delete", "title": "Lavender Coffee"}}]
        }))
        .send()
        .await
        .unwrap();
    assert!(resp.status().is_success());

    for _ in 0..200 {
        let r = client
            .post(format!("http://{}/1/indexes/repltest/query", addr_b))
            .json(&serde_json::json!({"query": "Lavender"}))
            .send()
            .await
            .unwrap()
            .json::<serde_json::Value>()
            .await
            .unwrap();
        if r["nbHits"].as_u64().unwrap_or(0) >= 1 {
            break;
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    }

    // Delete on node-a.
    let del = client
        .delete(format!("http://{}/1/indexes/repltest/to-delete", addr_a))
        .send()
        .await
        .unwrap();
    assert!(del.status().is_success());

    // Poll node-b until the document is gone.
    for _ in 0..200 {
        let r = client
            .post(format!("http://{}/1/indexes/repltest/query", addr_b))
            .json(&serde_json::json!({"query": "Lavender"}))
            .send()
            .await
            .unwrap()
            .json::<serde_json::Value>()
            .await
            .unwrap();
        if r["nbHits"].as_u64().unwrap_or(0) == 0 {
            return; // deletion propagated
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    }
    panic!("Delete did not propagate from node-a to node-b within 2s");
}

/// Write to node-b; doc must appear on node-a (bidirectional replication).
/// Also demonstrates why Phase 3 write forwarding is unnecessary: writes
/// on any node already propagate to all peers via trigger_replication().
#[tokio::test]
async fn test_two_node_bidirectional_replication() {
    let (addr_a, addr_b, _tmp_a, _tmp_b) = common::spawn_replication_pair("node-a", "node-b").await;
    let client = reqwest::Client::new();

    let resp = client
        .post(format!("http://{}/1/indexes/bidir/batch", addr_b))
        .json(&serde_json::json!({
            "requests": [{"action": "addObject", "body": {"_id": "b1", "title": "Cardamom Croissant"}}]
        }))
        .send()
        .await
        .unwrap();
    assert!(resp.status().is_success());

    for _ in 0..200 {
        let r = client
            .post(format!("http://{}/1/indexes/bidir/query", addr_a))
            .json(&serde_json::json!({"query": "Cardamom"}))
            .send()
            .await
            .unwrap()
            .json::<serde_json::Value>()
            .await
            .unwrap();
        if r["nbHits"].as_u64().unwrap_or(0) >= 1 {
            return;
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    }
    panic!("Bidirectional: doc written on node-b did not appear on node-a within 2s");
}

/// Startup catch-up: node-b fetches missed ops from node-a on startup via GET /internal/ops.
///
/// Tests ReplicationManager::catch_up_from_peer directly (bypasses the 3s startup
/// delay in spawn_startup_catchup). Verifies the full HTTP round-trip:
///   node-b calls /internal/ops on node-a → receives oplog entries →
///   apply_ops_to_manager → docs become searchable on node-b.
#[tokio::test]
async fn test_two_node_startup_catchup_via_get_ops() {
    use flapjack_replication::{
        config::{NodeConfig, PeerConfig},
        manager::ReplicationManager,
    };
    use tempfile::TempDir;

    // Only node-a runs as a server (serves /internal/ops).
    let (addr_a, _tmp_a) = common::spawn_server_with_internal("node-a").await;
    let client = reqwest::Client::new();

    // Write two docs to node-a.
    let resp = client
        .post(format!("http://{}/1/indexes/catchup/batch", addr_a))
        .json(&serde_json::json!({
            "requests": [
                {"action": "addObject", "body": {"_id": "c1", "title": "Matcha Waffles"}},
                {"action": "addObject", "body": {"_id": "c2", "title": "Turmeric Toast"}}
            ]
        }))
        .send()
        .await
        .unwrap();
    // Wait for write queue to commit so the oplog has entries (no blind sleep).
    common::wait_for_response_task(&client, &addr_a, resp).await;

    // Node-b: starts fresh. Catch up from node-a using the replication manager.
    let tmp_b = TempDir::new().unwrap();
    let manager_b = flapjack::IndexManager::new(tmp_b.path());
    let repl_mgr_b = ReplicationManager::new(NodeConfig {
        node_id: "node-b".to_string(),
        bind_addr: "0.0.0.0:0".to_string(),
        peers: vec![PeerConfig {
            node_id: "node-a".to_string(),
            addr: format!("http://{}", addr_a),
        }],
    });

    let ops = repl_mgr_b
        .catch_up_from_peer("catchup", 0)
        .await
        .expect("catch_up_from_peer should succeed: node-a is reachable");

    assert!(
        !ops.is_empty(),
        "Should have received oplog entries from node-a"
    );

    flapjack_http::handlers::internal::apply_ops_to_manager(&manager_b, "catchup", &ops)
        .await
        .unwrap();

    // Poll until write queue commits (no blind sleep).
    let mut matcha_ok = false;
    let mut turmeric_ok = false;
    for _ in 0..200 {
        if !matcha_ok {
            matcha_ok = manager_b
                .search("catchup", "Matcha", None, None, 10)
                .map(|r| r.total >= 1)
                .unwrap_or(false);
        }
        if !turmeric_ok {
            turmeric_ok = manager_b
                .search("catchup", "Turmeric", None, None, 10)
                .map(|r| r.total >= 1)
                .unwrap_or(false);
        }
        if matcha_ok && turmeric_ok {
            break;
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    }
    assert!(
        matcha_ok,
        "node-b should have 'Matcha Waffles' after startup catch-up"
    );
    assert!(
        turmeric_ok,
        "node-b should have 'Turmeric Toast' after startup catch-up"
    );
}

// ============================================================
// Phase 4: Analytics Rollup Exchange integration test.
//
// Unit tests for AnalyticsRollup / RollupCache live inline in
// analytics_cluster.rs (the rollup_tests module). This test
// covers the HTTP exchange endpoint end-to-end.
// ============================================================

/// POST /internal/analytics-rollup is accessible and stores the rollup.
/// Verifies: endpoint accepts the payload, returns 200, no auth required.
#[tokio::test]
async fn test_analytics_rollup_exchange_endpoint_accepts_rollup() {
    use std::time::{SystemTime, UNIX_EPOCH};

    let (addr, _tmp) = common::spawn_server_with_internal("node-a").await;
    let client = reqwest::Client::new();

    let now_secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let rollup = serde_json::json!({
        "node_id": "peer-node",
        "index": "my-index",
        "generated_at_secs": now_secs,
        "results": {
            "searches": {"searches": [], "total": 0}
        }
    });

    let resp = client
        .post(format!("http://{}/internal/analytics-rollup", addr))
        .json(&rollup)
        .send()
        .await
        .unwrap();

    assert_eq!(
        resp.status(),
        200,
        "Exchange endpoint should return 200 OK (no auth required)"
    );
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["status"], "ok");
}

// ============================================================
// Phase 4b: Rollup Broadcaster tests.
//
// Tests the background rollup push task: computes analytics
// locally and POSTs AnalyticsRollup to each peer's
// /internal/analytics-rollup endpoint every N seconds.
//
// NEW endpoints covered:
//   GET /internal/rollup-cache  → inspect cached rollups (diagnostic)
//
// NEW functions covered:
//   rollup_broadcaster::run_rollup_broadcast()
//   rollup_broadcaster::discover_indexes()
//   rollup_broadcaster::spawn_rollup_broadcaster()
//   AnalyticsClusterClient::push_rollup_to_peers()
// ============================================================

/// GET /internal/rollup-cache returns 200 with count=0 on a fresh node.
/// RED: Fails until /internal/rollup-cache route is registered.
#[tokio::test]
async fn test_rollup_cache_status_endpoint_empty() {
    // Clear global cache to prevent state leakage from other tests
    flapjack_http::analytics_cluster::get_global_rollup_cache().clear();

    let (addr, _tmp) = common::spawn_server_with_internal("node-cache-empty").await;
    let client = reqwest::Client::new();

    let resp = client
        .get(format!("http://{}/internal/rollup-cache", addr))
        .send()
        .await
        .unwrap();

    assert_eq!(
        resp.status(),
        200,
        "/internal/rollup-cache should return 200 on a fresh node"
    );
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(
        body["count"],
        serde_json::json!(0),
        "Fresh node should have 0 cached rollups; got: {}",
        body
    );
    assert!(
        body["entries"].is_array(),
        "Response should have an 'entries' array"
    );
}

/// POST to /internal/analytics-rollup stores the rollup; GET /internal/rollup-cache reflects it.
/// RED: Fails (404) until /internal/rollup-cache route is registered.
#[tokio::test]
async fn test_rollup_cache_status_reflects_stored_rollup() {
    use std::time::{SystemTime, UNIX_EPOCH};

    // Clear global cache to prevent state leakage from other tests
    flapjack_http::analytics_cluster::get_global_rollup_cache().clear();

    let (addr, _tmp) = common::spawn_server_with_internal("node-cache-reflect").await;
    let client = reqwest::Client::new();

    // POST a rollup
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let rollup = serde_json::json!({
        "node_id": "peer-broadcaster",
        "index": "my-index",
        "generated_at_secs": now,
        "results": {
            "searches": {"searches": [{"search": "hat", "count": 5, "nbHits": 10}], "total": 1}
        }
    });
    let post_resp = client
        .post(format!("http://{}/internal/analytics-rollup", addr))
        .json(&rollup)
        .send()
        .await
        .unwrap();
    assert_eq!(
        post_resp.status(),
        200,
        "POST /internal/analytics-rollup should return 200"
    );

    // GET rollup cache — must see the stored rollup
    let get_resp = client
        .get(format!("http://{}/internal/rollup-cache", addr))
        .send()
        .await
        .unwrap();
    assert_eq!(
        get_resp.status(),
        200,
        "/internal/rollup-cache should return 200"
    );
    let body: serde_json::Value = get_resp.json().await.unwrap();

    assert_eq!(
        body["count"],
        serde_json::json!(1),
        "Cache should contain 1 entry after POST; got: {}",
        body
    );
    let entries = body["entries"]
        .as_array()
        .expect("entries must be an array");
    assert_eq!(entries[0]["node_id"], "peer-broadcaster");
    assert_eq!(entries[0]["index"], "my-index");
}

/// run_rollup_broadcast() discovers a seeded index and pushes an AnalyticsRollup to the peer.
/// RED: Fails to compile until rollup_broadcaster module exists.
#[tokio::test]
async fn test_run_rollup_broadcast_sends_to_peer() {
    use flapjack_replication::config::{NodeConfig, PeerConfig};

    // Start a peer node that can receive rollups
    let (addr_b, _tmp_b) = common::spawn_server_with_internal("node-b-recv").await;

    // Set up node-a with a real analytics directory
    let tmp_a = TempDir::new().unwrap();
    let analytics_config = flapjack::analytics::AnalyticsConfig {
        enabled: true,
        data_dir: tmp_a.path().join("analytics"),
        flush_interval_secs: 3600,
        flush_size: 100_000,
        retention_days: 90,
    };

    // Seed analytics data so discover_indexes() finds "products"
    flapjack::analytics::seed::seed_analytics(&analytics_config, "products", 1)
        .expect("seed_analytics must succeed");

    let engine = Arc::new(flapjack::analytics::AnalyticsQueryEngine::new(
        analytics_config.clone(),
    ));

    // Build a cluster client pointing at node-b
    let node_cfg = NodeConfig {
        node_id: "node-a-send".to_string(),
        bind_addr: "127.0.0.1:0".to_string(),
        peers: vec![PeerConfig {
            node_id: "node-b-recv".to_string(),
            addr: format!("http://{}", addr_b),
        }],
    };
    let cluster = flapjack_http::analytics_cluster::AnalyticsClusterClient::new(&node_cfg)
        .expect("Should build cluster client with one peer");

    // Run one broadcast cycle (synchronous, no spawn needed for unit testing)
    flapjack_http::rollup_broadcaster::run_rollup_broadcast(
        &engine,
        &analytics_config,
        &cluster,
        "node-a-send",
    )
    .await;

    // Verify node-b has the rollup in its cache
    let client = reqwest::Client::new();
    let resp = client
        .get(format!("http://{}/internal/rollup-cache", addr_b))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        200,
        "/internal/rollup-cache should return 200"
    );
    let body: serde_json::Value = resp.json().await.unwrap();
    let count = body["count"].as_u64().unwrap_or(0);
    assert!(
        count > 0,
        "node-b rollup cache should have ≥1 entry after broadcast; got: {}",
        body
    );

    // Verify the rollup is from node-a for index=products
    let entries = body["entries"].as_array().expect("entries must be array");
    assert!(
        entries
            .iter()
            .any(|e| e["node_id"] == "node-a-send" && e["index"] == "products"),
        "Expected a rollup from node-a-send for index 'products'; got: {}",
        body
    );
}

/// spawn_rollup_broadcaster periodically pushes rollups to peers.
/// RED: Fails to compile until rollup_broadcaster module exists.
#[tokio::test]
async fn test_rollup_broadcaster_integration_periodic() {
    use flapjack_replication::config::{NodeConfig, PeerConfig};

    // Start the receiving peer
    let (addr_b, _tmp_b) = common::spawn_server_with_internal("node-b-periodic").await;

    // Set up node-a with seeded analytics
    let tmp_a = TempDir::new().unwrap();
    let analytics_config = flapjack::analytics::AnalyticsConfig {
        enabled: true,
        data_dir: tmp_a.path().join("analytics"),
        flush_interval_secs: 3600,
        flush_size: 100_000,
        retention_days: 90,
    };
    flapjack::analytics::seed::seed_analytics(&analytics_config, "widgets", 1)
        .expect("seed_analytics must succeed");

    let engine = Arc::new(flapjack::analytics::AnalyticsQueryEngine::new(
        analytics_config.clone(),
    ));

    let node_cfg = NodeConfig {
        node_id: "node-a-periodic".to_string(),
        bind_addr: "127.0.0.1:0".to_string(),
        peers: vec![PeerConfig {
            node_id: "node-b-periodic".to_string(),
            addr: format!("http://{}", addr_b),
        }],
    };
    let cluster = flapjack_http::analytics_cluster::AnalyticsClusterClient::new(&node_cfg)
        .expect("Should build cluster client");

    // Spawn broadcaster with a 1-second interval
    flapjack_http::rollup_broadcaster::spawn_rollup_broadcaster(
        Arc::clone(&engine),
        analytics_config,
        cluster,
        "node-a-periodic".to_string(),
        1, // 1s interval for test speed
    );

    // Wait up to 4 seconds for the broadcaster to fire at least once
    let client = reqwest::Client::new();
    let mut count = 0u64;
    for _ in 0..40 {
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        let resp = client
            .get(format!("http://{}/internal/rollup-cache", addr_b))
            .send()
            .await
            .unwrap();
        if resp.status() == 200 {
            let body: serde_json::Value = resp.json().await.unwrap();
            count = body["count"].as_u64().unwrap_or(0);
            if count > 0 {
                break;
            }
        }
    }
    assert!(
        count > 0,
        "Broadcaster should have fired within 4s and pushed rollup to node-b"
    );
}

// ============================================================
// P0: Periodic Anti-Entropy Sync tests
//
// Verifies that the periodic sync mechanism pulls missed ops from
// peers, closing the network partition gap identified in
// HA_HARDENING_HANDOFF.md.
//
// These tests exercise:
//   run_periodic_catchup()   — one-shot catch-up for all tenants
//   spawn_periodic_sync()    — background task that fires on a timer
//
// RED phase: tests fail because the stub functions are empty.
// GREEN phase: tests pass after implementing the real logic.
// ============================================================

/// Core periodic sync test: node-b pulls missed ops from node-a without restart.
/// Simulates a partition gap: node-a has docs that node-b missed.
/// run_periodic_catchup() must detect the gap and fill it.
///
/// RED: Fails because stub run_periodic_catchup() does nothing.
#[tokio::test]
async fn test_periodic_sync_pulls_missed_ops_from_peer() {
    use flapjack_replication::{
        config::{NodeConfig, PeerConfig},
        manager::ReplicationManager,
    };

    // 1. Start node-a as a server (serves /internal/ops)
    let (addr_a, _tmp_a) = common::spawn_server_with_internal("node-a-sync").await;
    let client = reqwest::Client::new();

    // 2. Write docs to node-a (simulating writes during a partition)
    let resp = client
        .post(format!("http://{}/1/indexes/sync-test/batch", addr_a))
        .json(&serde_json::json!({
            "requests": [
                {"action": "addObject", "body": {"_id": "s1", "title": "Espresso Martini"}},
                {"action": "addObject", "body": {"_id": "s2", "title": "Cold Brew Float"}}
            ]
        }))
        .send()
        .await
        .unwrap();
    // Wait for writes to commit to oplog (no blind sleep).
    common::wait_for_response_task(&client, &addr_a, resp).await;

    // 3. Set up node-b: has the tenant dir but seq=0 (missed node-a's writes)
    let tmp_b = tempfile::TempDir::new().unwrap();
    let manager_b = flapjack::IndexManager::new(tmp_b.path());
    manager_b.create_tenant("sync-test").unwrap();

    let repl_mgr_b = ReplicationManager::new(NodeConfig {
        node_id: "node-b-sync".to_string(),
        bind_addr: "0.0.0.0:0".to_string(),
        peers: vec![PeerConfig {
            node_id: "node-a-sync".to_string(),
            addr: format!("http://{}", addr_a),
        }],
    });

    let state_b = Arc::new(flapjack_http::handlers::AppState {
        manager: manager_b.clone(),
        key_store: None,
        replication_manager: Some(repl_mgr_b),
        ssl_manager: None,
        analytics_engine: None,
        metrics_state: None,
        usage_counters: std::sync::Arc::new(dashmap::DashMap::new()),
        paused_indexes: flapjack_http::pause_registry::PausedIndexes::new(),
        start_time: std::time::Instant::now(),
        experiment_store: None,
    });

    // 4. Run periodic catchup — should pull missed ops from node-a
    flapjack_http::startup_catchup::run_periodic_catchup(Arc::clone(&state_b)).await;

    // 5. Poll until node-b has both docs (write queue is async)
    let mut doc1_ok = false;
    let mut doc2_ok = false;
    for _ in 0..200 {
        if !doc1_ok {
            doc1_ok = manager_b
                .search("sync-test", "Espresso", None, None, 10)
                .map(|r| r.total >= 1)
                .unwrap_or(false);
        }
        if !doc2_ok {
            doc2_ok = manager_b
                .search("sync-test", "Cold Brew", None, None, 10)
                .map(|r| r.total >= 1)
                .unwrap_or(false);
        }
        if doc1_ok && doc2_ok {
            break;
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    }
    assert!(
        doc1_ok,
        "node-b should have 'Espresso Martini' after periodic sync"
    );
    assert!(
        doc2_ok,
        "node-b should have 'Cold Brew Float' after periodic sync"
    );
}

/// Periodic sync works across multiple tenants — not just the first one found.
///
/// RED: Fails because stub run_periodic_catchup() does nothing.
#[tokio::test]
async fn test_periodic_sync_catches_up_multiple_tenants() {
    use flapjack_replication::{
        config::{NodeConfig, PeerConfig},
        manager::ReplicationManager,
    };

    let (addr_a, _tmp_a) = common::spawn_server_with_internal("node-a-multi").await;
    let client = reqwest::Client::new();

    // Write docs to two different tenants on node-a
    let resp1 = client
        .post(format!("http://{}/1/indexes/tenant-alpha/batch", addr_a))
        .json(&serde_json::json!({
            "requests": [{"action": "addObject", "body": {"_id": "a1", "title": "Maple Syrup"}}]
        }))
        .send()
        .await
        .unwrap();
    // Wait for writes to commit to oplog (no blind sleep).
    common::wait_for_response_task(&client, &addr_a, resp1).await;

    let resp2 = client
        .post(format!("http://{}/1/indexes/tenant-beta/batch", addr_a))
        .json(&serde_json::json!({
            "requests": [{"action": "addObject", "body": {"_id": "b1", "title": "Vanilla Extract"}}]
        }))
        .send()
        .await
        .unwrap();
    // Wait for writes to commit to oplog (no blind sleep).
    common::wait_for_response_task(&client, &addr_a, resp2).await;

    // Node-b has both tenant dirs but no docs
    let tmp_b = tempfile::TempDir::new().unwrap();
    let manager_b = flapjack::IndexManager::new(tmp_b.path());
    manager_b.create_tenant("tenant-alpha").unwrap();
    manager_b.create_tenant("tenant-beta").unwrap();

    let repl_mgr_b = ReplicationManager::new(NodeConfig {
        node_id: "node-b-multi".to_string(),
        bind_addr: "0.0.0.0:0".to_string(),
        peers: vec![PeerConfig {
            node_id: "node-a-multi".to_string(),
            addr: format!("http://{}", addr_a),
        }],
    });

    let state_b = Arc::new(flapjack_http::handlers::AppState {
        manager: manager_b.clone(),
        key_store: None,
        replication_manager: Some(repl_mgr_b),
        ssl_manager: None,
        analytics_engine: None,
        metrics_state: None,
        usage_counters: std::sync::Arc::new(dashmap::DashMap::new()),
        paused_indexes: flapjack_http::pause_registry::PausedIndexes::new(),
        start_time: std::time::Instant::now(),
        experiment_store: None,
    });

    flapjack_http::startup_catchup::run_periodic_catchup(Arc::clone(&state_b)).await;

    // Poll until both tenants have their docs (write queue is async)
    let mut alpha_ok = false;
    let mut beta_ok = false;
    for _ in 0..200 {
        if !alpha_ok {
            alpha_ok = manager_b
                .search("tenant-alpha", "Maple", None, None, 10)
                .map(|r| r.total >= 1)
                .unwrap_or(false);
        }
        if !beta_ok {
            beta_ok = manager_b
                .search("tenant-beta", "Vanilla", None, None, 10)
                .map(|r| r.total >= 1)
                .unwrap_or(false);
        }
        if alpha_ok && beta_ok {
            break;
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    }
    assert!(
        alpha_ok,
        "tenant-alpha should have 'Maple Syrup' after periodic sync"
    );
    assert!(
        beta_ok,
        "tenant-beta should have 'Vanilla Extract' after periodic sync"
    );
}

/// spawn_periodic_sync fires the sync task within the configured interval.
/// Uses a 1s interval and verifies data is pulled within 4s.
///
/// RED: Fails because stub spawn_periodic_sync() does nothing.
#[tokio::test]
async fn test_spawn_periodic_sync_fires_within_interval() {
    use flapjack_replication::{
        config::{NodeConfig, PeerConfig},
        manager::ReplicationManager,
    };

    // Start node-a with a doc
    let (addr_a, _tmp_a) = common::spawn_server_with_internal("node-a-spawn").await;
    let client = reqwest::Client::new();

    let resp = client
        .post(format!("http://{}/1/indexes/spawn-test/batch", addr_a))
        .json(&serde_json::json!({
            "requests": [{"action": "addObject", "body": {"_id": "p1", "title": "Pistachio Latte"}}]
        }))
        .send()
        .await
        .unwrap();
    // Wait for writes to commit to oplog (no blind sleep).
    common::wait_for_response_task(&client, &addr_a, resp).await;

    // Node-b: tenant exists but no docs
    let tmp_b = tempfile::TempDir::new().unwrap();
    let manager_b = flapjack::IndexManager::new(tmp_b.path());
    manager_b.create_tenant("spawn-test").unwrap();

    let repl_mgr_b = ReplicationManager::new(NodeConfig {
        node_id: "node-b-spawn".to_string(),
        bind_addr: "0.0.0.0:0".to_string(),
        peers: vec![PeerConfig {
            node_id: "node-a-spawn".to_string(),
            addr: format!("http://{}", addr_a),
        }],
    });

    let state_b = Arc::new(flapjack_http::handlers::AppState {
        manager: manager_b.clone(),
        key_store: None,
        replication_manager: Some(repl_mgr_b),
        ssl_manager: None,
        analytics_engine: None,
        metrics_state: None,
        usage_counters: std::sync::Arc::new(dashmap::DashMap::new()),
        paused_indexes: flapjack_http::pause_registry::PausedIndexes::new(),
        start_time: std::time::Instant::now(),
        experiment_store: None,
    });

    // Spawn periodic sync with 1s interval
    flapjack_http::startup_catchup::spawn_periodic_sync(Arc::clone(&state_b), 1);

    // Poll node-b for up to 5s — the spawned task should fire and pull docs
    let mut found = false;
    for _ in 0..50 {
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        if manager_b
            .search("spawn-test", "Pistachio", None, None, 10)
            .map(|r| r.total)
            .unwrap_or(0)
            >= 1
        {
            found = true;
            break;
        }
    }
    assert!(
        found,
        "spawn_periodic_sync should have fired within 5s and pulled 'Pistachio Latte' to node-b"
    );
}
