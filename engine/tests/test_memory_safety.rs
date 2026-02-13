/// Tests for memory safety features:
/// Pass 1: Document size limits, batch size limits, health endpoint
/// Pass 3: Memory pressure middleware, load shedding
mod common;

use axum::{
    middleware,
    routing::{get, post},
    Router,
};
use flapjack::types::{Document, FieldValue, TaskStatus};
use flapjack::IndexManager;
use std::collections::HashMap;
use std::sync::Arc;
use tempfile::TempDir;
use tokio::net::TcpListener;

// ============================================================
// Test 1: Oversized document appears in rejected array
// ============================================================

#[tokio::test]
async fn test_oversized_document_rejected() {
    // Set a very low doc size limit for this test (env is process-wide,
    // but the global budget is initialized once via OnceLock — so we
    // use a fresh MemoryBudget with a custom config instead).
    // The default is 3 MB. We create a ~4 MB document to exceed it.
    let temp_dir = TempDir::new().unwrap();
    let manager = IndexManager::new(temp_dir.path());
    manager.create_tenant("test_idx").unwrap();

    // Build a document larger than 3 MB (default FLAPJACK_MAX_DOC_MB)
    let big_value = "x".repeat(4 * 1024 * 1024); // 4 MB string
    let doc = Document {
        id: "big-doc-1".to_string(),
        fields: HashMap::from([("payload".to_string(), FieldValue::Text(big_value))]),
    };

    // Submit via async write queue (not sync, so we can inspect the task)
    let task = manager.add_documents("test_idx", vec![doc]).unwrap();

    // Poll until task completes
    loop {
        let status = manager.get_task(&task.id).unwrap();
        match status.status {
            TaskStatus::Enqueued | TaskStatus::Processing => {
                tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
            }
            TaskStatus::Succeeded => {
                // Task succeeded but document should be in rejected list
                assert_eq!(status.rejected_count, 1, "Expected 1 rejected document");
                assert_eq!(status.rejected_documents.len(), 1);
                assert_eq!(status.rejected_documents[0].doc_id, "big-doc-1");
                assert_eq!(status.rejected_documents[0].error, "document_too_large");
                assert_eq!(
                    status.indexed_documents, 0,
                    "No docs should have been indexed"
                );
                break;
            }
            TaskStatus::Failed(e) => {
                panic!("Task failed unexpectedly: {}", e);
            }
        }
    }
}

#[tokio::test]
async fn test_normal_document_accepted() {
    let temp_dir = TempDir::new().unwrap();
    let manager = IndexManager::new(temp_dir.path());
    manager.create_tenant("test_idx").unwrap();

    let doc = Document {
        id: "small-doc-1".to_string(),
        fields: HashMap::from([(
            "title".to_string(),
            FieldValue::Text("A perfectly normal document".to_string()),
        )]),
    };

    let task = manager.add_documents("test_idx", vec![doc]).unwrap();

    loop {
        let status = manager.get_task(&task.id).unwrap();
        match status.status {
            TaskStatus::Enqueued | TaskStatus::Processing => {
                tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
            }
            TaskStatus::Succeeded => {
                assert_eq!(status.rejected_count, 0, "No docs should be rejected");
                assert!(status.indexed_documents > 0, "Document should be indexed");
                break;
            }
            TaskStatus::Failed(e) => {
                panic!("Task failed unexpectedly: {}", e);
            }
        }
    }
}

// ============================================================
// Test: Mixed batch — oversized docs rejected, normal docs indexed
// ============================================================

#[tokio::test]
async fn test_mixed_batch_partial_rejection() {
    let temp_dir = TempDir::new().unwrap();
    let manager = IndexManager::new(temp_dir.path());
    manager.create_tenant("test_idx").unwrap();

    let big_value = "x".repeat(4 * 1024 * 1024); // 4 MB — over default 3 MB limit
    let docs = vec![
        Document {
            id: "small-1".to_string(),
            fields: HashMap::from([(
                "title".to_string(),
                FieldValue::Text("Good doc".to_string()),
            )]),
        },
        Document {
            id: "big-1".to_string(),
            fields: HashMap::from([("payload".to_string(), FieldValue::Text(big_value))]),
        },
        Document {
            id: "small-2".to_string(),
            fields: HashMap::from([(
                "title".to_string(),
                FieldValue::Text("Another good doc".to_string()),
            )]),
        },
    ];

    let task = manager.add_documents("test_idx", docs).unwrap();

    loop {
        let status = manager.get_task(&task.id).unwrap();
        match status.status {
            TaskStatus::Enqueued | TaskStatus::Processing => {
                tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
            }
            TaskStatus::Succeeded => {
                assert_eq!(
                    status.rejected_count, 1,
                    "Only the oversized doc should be rejected"
                );
                assert_eq!(status.rejected_documents[0].doc_id, "big-1");
                assert_eq!(status.rejected_documents[0].error, "document_too_large");
                assert_eq!(
                    status.indexed_documents, 2,
                    "Two normal docs should be indexed"
                );
                break;
            }
            TaskStatus::Failed(e) => {
                panic!("Task failed unexpectedly: {}", e);
            }
        }
    }
}

// ============================================================
// Test: Oversized document rejected via upsert path
// ============================================================

#[tokio::test]
async fn test_oversized_upsert_rejected() {
    let temp_dir = TempDir::new().unwrap();
    let manager = IndexManager::new(temp_dir.path());
    manager.create_tenant("test_idx").unwrap();

    // First, insert a small document (add_documents_insert = non-upsert Add path)
    let small_doc = Document {
        id: "doc-1".to_string(),
        fields: HashMap::from([(
            "title".to_string(),
            FieldValue::Text("OriginalUpsertTest".to_string()),
        )]),
    };
    let task = manager
        .add_documents_insert("test_idx", vec![small_doc])
        .unwrap();
    loop {
        let status = manager.get_task(&task.id).unwrap();
        match status.status {
            TaskStatus::Enqueued | TaskStatus::Processing => {
                tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
            }
            TaskStatus::Succeeded => {
                break;
            }
            TaskStatus::Failed(e) => {
                panic!("Initial add failed: {}", e);
            }
        }
    }

    // Now upsert (add_documents = upsert path) with an oversized replacement.
    // Should be rejected AND the original document should survive (the size
    // check runs BEFORE delete_term in the Upsert arm of write_queue.rs).
    let big_value = "x".repeat(4 * 1024 * 1024);
    let big_doc = Document {
        id: "doc-1".to_string(),
        fields: HashMap::from([("payload".to_string(), FieldValue::Text(big_value))]),
    };
    let task = manager.add_documents("test_idx", vec![big_doc]).unwrap();
    loop {
        let status = manager.get_task(&task.id).unwrap();
        match status.status {
            TaskStatus::Enqueued | TaskStatus::Processing => {
                tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
            }
            TaskStatus::Succeeded => {
                assert_eq!(
                    status.rejected_count, 1,
                    "Oversized upsert should be rejected"
                );
                assert_eq!(status.rejected_documents[0].doc_id, "doc-1");
                assert_eq!(status.rejected_documents[0].error, "document_too_large");
                assert_eq!(status.indexed_documents, 0, "No docs should be re-indexed");
                break;
            }
            TaskStatus::Failed(e) => {
                panic!("Upsert task failed: {}", e);
            }
        }
    }

    // Verify the original document is still searchable (proves size check
    // runs before delete_term, so a failed upsert doesn't destroy the old doc)
    let results = manager
        .search("test_idx", "OriginalUpsertTest", None, None, 10)
        .unwrap();
    assert_eq!(
        results.documents.len(),
        1,
        "Original document should still exist after failed upsert"
    );
}

// ============================================================
// Test 2: Batch size limit (sequential to avoid env var races)
// ============================================================

#[tokio::test]
async fn test_batch_size_limit() {
    std::env::set_var("FLAPJACK_MAX_BATCH_SIZE", "5");

    let (addr, _tmp) = common::spawn_server().await;
    let client = reqwest::Client::new();

    // Batch within limit should succeed
    let small_ops: Vec<serde_json::Value> = (0..5)
        .map(|i| {
            serde_json::json!({
                "action": "addObject",
                "body": {"objectID": format!("doc-{}", i), "title": format!("Doc {}", i)}
            })
        })
        .collect();

    let resp = client
        .post(format!("http://{}/1/indexes/test_idx/batch", addr))
        .json(&serde_json::json!({"requests": small_ops}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200, "Expected 200 for batch within limit");

    // Batch exceeding limit should fail
    let big_ops: Vec<serde_json::Value> = (0..6)
        .map(|i| {
            serde_json::json!({
                "action": "addObject",
                "body": {"objectID": format!("big-{}", i), "title": format!("Doc {}", i)}
            })
        })
        .collect();

    let resp = client
        .post(format!("http://{}/1/indexes/test_idx/batch", addr))
        .json(&serde_json::json!({"requests": big_ops}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 400, "Expected 400 for batch too large");
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["error"], "batch_too_large");

    std::env::remove_var("FLAPJACK_MAX_BATCH_SIZE");
}

// ============================================================
// Test 3: Health endpoint returns JSON with memory fields
// ============================================================

#[tokio::test]
async fn test_health_returns_json_with_memory_fields() {
    let (addr, _tmp) = common::spawn_server().await;
    let client = reqwest::Client::new();

    let resp = client
        .get(format!("http://{}/health", addr))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();

    assert_eq!(body["status"], "ok");
    assert!(
        body["active_writers"].is_number(),
        "active_writers should be a number"
    );
    assert!(
        body["max_concurrent_writers"].is_number(),
        "max_concurrent_writers should be a number"
    );
    assert!(
        body["facet_cache_entries"].is_number(),
        "facet_cache_entries should be a number"
    );
    assert!(
        body["facet_cache_cap"].is_number(),
        "facet_cache_cap should be a number"
    );
    // Pass 2 fields
    assert!(
        body["heap_allocated_mb"].is_number(),
        "heap_allocated_mb should be a number"
    );
    assert!(
        body["system_limit_mb"].is_number(),
        "system_limit_mb should be a number"
    );
    assert!(
        body["pressure_level"].is_string(),
        "pressure_level should be a string"
    );
    assert!(
        body["allocator"].is_string(),
        "allocator should be a string"
    );
}

// ============================================================
// Pass 3: Memory pressure middleware tests
// ============================================================

/// Spawn a server with the memory pressure middleware included.
async fn spawn_server_with_pressure_middleware() -> (String, TempDir) {
    let temp_dir = TempDir::new().unwrap();
    let manager = flapjack::IndexManager::new(temp_dir.path());

    let state = Arc::new(flapjack_http::handlers::AppState {
        manager,
        key_store: None,
        replication_manager: None,
        ssl_manager: None,
    });

    let health_route = Router::new()
        .route("/health", get(flapjack_http::handlers::health))
        .with_state(state.clone());

    let protected = Router::new()
        .route("/1/indexes", post(flapjack_http::handlers::create_index))
        .route(
            "/1/indexes/:indexName/batch",
            post(flapjack_http::handlers::add_documents),
        )
        .route(
            "/1/indexes/:indexName/query",
            post(flapjack_http::handlers::search),
        )
        .route("/1/tasks/:task_id", get(flapjack_http::handlers::get_task))
        .with_state(state.clone());

    let auth_middleware = middleware::from_fn(
        move |request: axum::extract::Request, next: middleware::Next| async move {
            flapjack_http::auth::authenticate_and_authorize(request, next).await
        },
    );

    let mgr_for_pressure = Arc::clone(&state.manager);
    let default_cap = state
        .manager
        .facet_cache_cap
        .load(std::sync::atomic::Ordering::Relaxed);
    let memory_middleware = middleware::from_fn(
        move |request: axum::extract::Request, next: middleware::Next| {
            let mgr = mgr_for_pressure.clone();
            async move {
                flapjack_http::memory_middleware::memory_pressure_guard(
                    request,
                    next,
                    &mgr,
                    default_cap,
                )
                .await
            }
        },
    );

    let app = Router::new()
        .merge(health_route)
        .merge(protected)
        .layer(memory_middleware)
        .layer(auth_middleware);

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap().to_string();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    (addr, temp_dir)
}

/// Combined test for Critical, Elevated, and Normal pressure levels.
/// These must run sequentially because they share the global MemoryObserver
/// pressure override (AtomicU8). Separate #[tokio::test] functions would
/// race on the global state and produce flaky failures.
#[tokio::test]
async fn test_memory_pressure_levels() {
    let (addr, _tmp) = spawn_server_with_pressure_middleware().await;
    let client = reqwest::Client::new();
    let observer = flapjack::MemoryObserver::global();

    // --- Normal: everything should pass through the middleware ---
    observer.set_pressure_override(Some(flapjack::PressureLevel::Normal));

    // Write under Normal: should NOT get 503 (may get 200 or other non-503 status)
    let resp = client
        .post(format!("http://{}/1/indexes/test_idx/batch", addr))
        .json(&serde_json::json!({"requests": [{
            "action": "addObject",
            "body": {"objectID": "doc-normal", "title": "Test"}
        }]}))
        .send()
        .await
        .unwrap();
    assert_ne!(
        resp.status(),
        503,
        "Writes should NOT be rejected under normal pressure"
    );

    // --- Critical: reject everything except /health ---
    observer.set_pressure_override(Some(flapjack::PressureLevel::Critical));

    // Write request should be rejected with 503 + Retry-After header
    let resp = client
        .post(format!("http://{}/1/indexes/test_idx/batch", addr))
        .json(&serde_json::json!({"requests": [{
            "action": "addObject",
            "body": {"objectID": "doc-1", "title": "Test"}
        }]}))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        503,
        "Expected 503 for writes under critical pressure"
    );
    assert_eq!(
        resp.headers()
            .get("retry-after")
            .and_then(|v| v.to_str().ok()),
        Some("5"),
        "503 should include Retry-After: 5 header"
    );
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["error"], "memory_pressure");

    // Search (POST) should also be rejected under Critical
    let resp = client
        .post(format!("http://{}/1/indexes/test_idx/query", addr))
        .json(&serde_json::json!({"query": "test"}))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        503,
        "Search should be rejected under critical pressure"
    );

    // Health endpoint should still return 200
    let resp = client
        .get(format!("http://{}/health", addr))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        200,
        "Health should return 200 even under critical pressure"
    );
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["status"], "ok");

    // --- Elevated: reject all POSTs (writes + search), allow GET + health ---
    observer.set_pressure_override(Some(flapjack::PressureLevel::Elevated));

    // Write POST should be rejected with 503
    let resp = client
        .post(format!("http://{}/1/indexes/test_idx/batch", addr))
        .json(&serde_json::json!({"requests": [{
            "action": "addObject",
            "body": {"objectID": "doc-1", "title": "Test"}
        }]}))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        503,
        "Writes should be rejected under elevated pressure"
    );

    // POST search is also rejected under Elevated (middleware rejects all non-GET)
    let resp = client
        .post(format!("http://{}/1/indexes/test_idx/query", addr))
        .json(&serde_json::json!({"query": "test"}))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        503,
        "POST search should be rejected under elevated pressure"
    );

    // GET /health should still work
    let resp = client
        .get(format!("http://{}/health", addr))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        200,
        "Health should return 200 under elevated pressure"
    );

    // Clear override
    observer.set_pressure_override(None);
}
