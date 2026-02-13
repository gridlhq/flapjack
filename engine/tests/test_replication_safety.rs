use flapjack::types::Document;
/// Replication Safety Tests
/// Tests for data integrity, concurrency, and edge cases in peer replication
use flapjack::IndexManager;
use std::sync::Arc;
use tempfile::TempDir;

/// Test that oplog entries contain full document body (not just objectID)
/// This is critical for replication - without full body, peers can't reconstruct the document
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

    // Read oplog and verify it contains the full document body
    let oplog = manager
        .get_or_create_oplog("test")
        .expect("OpLog should exist");
    let ops = oplog.read_since(0).unwrap();

    assert!(!ops.is_empty(), "OpLog should have at least one entry");

    let first_op = &ops[0];
    assert_eq!(first_op.op_type, "upsert", "First op should be an upsert");

    // Verify payload contains the full document
    let payload = &first_op.payload;
    assert!(
        payload.get("body").is_some(),
        "Payload should have 'body' field"
    );

    let body = payload.get("body").unwrap();
    assert_eq!(
        body.get("title").and_then(|v| v.as_str()),
        Some("Test Document"),
        "OpLog should contain full document body, not just objectID"
    );
    assert_eq!(
        body.get("price").and_then(|v| v.as_u64()),
        Some(99),
        "OpLog should preserve all document fields"
    );
}

/// Test that delete operations include objectID in payload
/// Bug discovered in handoff #168 - deletes were missing objectID
#[tokio::test]
async fn test_oplog_delete_includes_object_id() {
    let temp_dir = TempDir::new().unwrap();
    let manager = IndexManager::new(temp_dir.path());

    manager.create_tenant("test").unwrap();

    // Add then delete a document
    let doc =
        Document::from_json(&serde_json::json!({"_id": "delete-me", "title": "Temp"})).unwrap();
    manager.add_documents_sync("test", vec![doc]).await.unwrap();
    manager
        .delete_documents_sync("test", vec!["delete-me".to_string()])
        .await
        .unwrap();

    // Check oplog for delete entry
    let oplog = manager
        .get_or_create_oplog("test")
        .expect("OpLog should exist");
    let ops = oplog.read_since(0).unwrap();

    // Find the delete operation
    let delete_op = ops
        .iter()
        .find(|op| op.op_type == "delete")
        .expect("Should have a delete operation in oplog");

    // Verify it has objectID in payload
    assert!(
        delete_op.payload.get("objectID").is_some(),
        "Delete operation MUST include objectID in payload for replication"
    );
    assert_eq!(
        delete_op.payload.get("objectID").and_then(|v| v.as_str()),
        Some("delete-me"),
        "Delete payload should contain the correct objectID"
    );
}

/// Test concurrent writes to different tenants don't interfere with each other's oplogs
/// This tests for sequence number collision bugs (handoff #169)
#[tokio::test]
async fn test_concurrent_tenant_oplog_isolation() {
    let temp_dir = TempDir::new().unwrap();
    let manager = Arc::new(IndexManager::new(temp_dir.path()));

    // Create two tenants
    manager.create_tenant("tenant-a").unwrap();
    manager.create_tenant("tenant-b").unwrap();

    // Spawn concurrent writes to both tenants
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

    // Wait for both to complete
    handle_a.await.unwrap();
    handle_b.await.unwrap();

    // Verify each oplog has exactly 10 entries and they're distinct
    let oplog_a = manager
        .get_or_create_oplog("tenant-a")
        .expect("Tenant A oplog should exist");
    let ops_a = oplog_a.read_since(0).unwrap();

    let oplog_b = manager
        .get_or_create_oplog("tenant-b")
        .expect("Tenant B oplog should exist");
    let ops_b = oplog_b.read_since(0).unwrap();

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

    // Verify tenant A ops only reference tenant A
    for op in &ops_a {
        let body = op.payload.get("body");
        if let Some(body_obj) = body {
            assert_eq!(
                body_obj.get("tenant").and_then(|v| v.as_str()),
                Some("A"),
                "Tenant A oplog should only contain tenant A documents"
            );
        }
    }

    // Verify tenant B ops only reference tenant B
    for op in &ops_b {
        let body = op.payload.get("body");
        if let Some(body_obj) = body {
            assert_eq!(
                body_obj.get("tenant").and_then(|v| v.as_str()),
                Some("B"),
                "Tenant B oplog should only contain tenant B documents"
            );
        }
    }
}

/// Test that sequence numbers are monotonically increasing (no gaps or duplicates)
/// Gaps could cause replication to miss operations
#[tokio::test]
async fn test_oplog_sequence_numbers_monotonic() {
    let temp_dir = TempDir::new().unwrap();
    let manager = IndexManager::new(temp_dir.path());

    manager.create_tenant("test").unwrap();

    // Add multiple documents
    for i in 1..=5 {
        let doc =
            Document::from_json(&serde_json::json!({"_id": i.to_string(), "value": i})).unwrap();
        manager.add_documents_sync("test", vec![doc]).await.unwrap();
    }

    // Read oplog and check sequences
    let oplog = manager.get_or_create_oplog("test").unwrap();
    let ops = oplog.read_since(0).unwrap();

    assert_eq!(ops.len(), 5, "Should have 5 operations");

    // Check sequences are strictly increasing
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

/// Test that oplog read_since returns correct subset
/// False positive check: ensure read_since(N) doesn't return ops with seq <= N
#[tokio::test]
async fn test_oplog_read_since_boundary() {
    let temp_dir = TempDir::new().unwrap();
    let manager = IndexManager::new(temp_dir.path());

    manager.create_tenant("test").unwrap();

    // Add 10 documents
    for i in 1..=10 {
        let doc =
            Document::from_json(&serde_json::json!({"_id": i.to_string(), "value": i})).unwrap();
        manager.add_documents_sync("test", vec![doc]).await.unwrap();
    }

    let oplog = manager.get_or_create_oplog("test").unwrap();

    // Read all ops to get the sequence numbers
    let all_ops = oplog.read_since(0).unwrap();
    assert_eq!(all_ops.len(), 10);

    // Get the 5th operation's sequence number
    let fifth_seq = all_ops[4].seq;

    // read_since(fifth_seq) should return ops AFTER the 5th, not including it
    let ops_after_fifth = oplog.read_since(fifth_seq).unwrap();

    // Should have 5 remaining ops (6th through 10th)
    assert_eq!(
        ops_after_fifth.len(),
        5,
        "read_since({}) should return 5 remaining ops, not {}",
        fifth_seq,
        ops_after_fifth.len()
    );

    // Verify none of the returned ops have seq <= fifth_seq
    for op in &ops_after_fifth {
        assert!(
            op.seq > fifth_seq,
            "read_since({}) returned op with seq {}, should only return seq > {}",
            fifth_seq,
            op.seq,
            fifth_seq
        );
    }
}

/// Test that replicate_ops handler correctly parses and applies operations
/// This is an integration test of the /internal/replicate endpoint logic
#[tokio::test]
async fn test_replicate_ops_handler_applies_correctly() {
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use axum::Router;
    use tower::ServiceExt;

    let temp_dir = TempDir::new().unwrap();
    let manager = IndexManager::new(temp_dir.path());

    let state = Arc::new(flapjack_http::handlers::AppState {
        manager: manager.clone(),
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

    // Create a replicate request with 2 upserts and 1 delete
    let req_body = serde_json::json!({
        "tenant_id": "test",
        "ops": [
            {
                "seq": 1,
                "timestamp_ms": 1000,
                "node_id": "node-a",
                "tenant_id": "test",
                "op_type": "upsert",
                "payload": {
                    "body": {"_id": "1", "title": "First"}
                }
            },
            {
                "seq": 2,
                "timestamp_ms": 2000,
                "node_id": "node-a",
                "tenant_id": "test",
                "op_type": "upsert",
                "payload": {
                    "body": {"_id": "2", "title": "Second"}
                }
            },
            {
                "seq": 3,
                "timestamp_ms": 3000,
                "node_id": "node-a",
                "tenant_id": "test",
                "op_type": "delete",
                "payload": {
                    "objectID": "1"
                }
            }
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

    // Verify the documents were applied correctly
    let search_result = manager.search("test", "First", None, None, 10).unwrap();
    assert_eq!(search_result.total, 0, "Document 1 should be deleted");

    let search_result2 = manager.search("test", "Second", None, None, 10).unwrap();
    assert_eq!(search_result2.total, 1, "Document 2 should exist");
}
