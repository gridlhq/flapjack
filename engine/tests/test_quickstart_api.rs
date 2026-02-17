//! Tests for the Quickstart API — no-auth convenience endpoints.
//!
//! These tests validate the simplified REST API layer that sits at root level
//! (no /1/ prefix, no auth headers, no Content-Type header required).
//! Every test uses the quickstart endpoints exclusively — no Algolia-style headers.

use serde_json::{json, Value};

mod common;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Seed an index via the quickstart documents endpoint and wait for indexing.
async fn seed_index(client: &reqwest::Client, base: &str, index: &str, docs: Value) {
    let resp = client
        .post(format!("{}/indexes/{}/documents", base, index))
        .json(&docs)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200, "Seed failed: {:?}", resp.text().await);
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
}

// ---------------------------------------------------------------------------
// Full lifecycle
// ---------------------------------------------------------------------------

#[tokio::test]
async fn quickstart_full_lifecycle() {
    let (addr, _temp) = common::spawn_server().await;
    let client = reqwest::Client::new();
    let base = format!("http://{}", addr);

    // 1. Add documents (array)
    let resp = client
        .post(format!("{}/indexes/movies/documents", base))
        .json(&json!([
            {"objectID": "1", "title": "The Matrix", "year": 1999},
            {"objectID": "2", "title": "Inception", "year": 2010},
            {"objectID": "3", "title": "Interstellar", "year": 2014}
        ]))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.unwrap();
    assert!(body.get("taskID").is_some(), "Missing taskID: {:?}", body);
    assert_eq!(
        body["objectIDs"].as_array().unwrap().len(),
        3,
        "Expected 3 objectIDs"
    );

    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    // 2. GET search
    let resp = client
        .get(format!("{}/indexes/movies/search?q=matrix", base))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.unwrap();
    assert!(
        body["nbHits"].as_u64().unwrap() >= 1,
        "Expected hits for 'matrix'"
    );
    assert!(body.get("hits").is_some(), "Missing hits");

    // 3. GET document
    let resp = client
        .get(format!("{}/indexes/movies/documents/1", base))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["objectID"], "1");
    assert_eq!(body["title"], "The Matrix");

    // 4. List indexes
    let resp = client
        .get(format!("{}/indexes", base))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.unwrap();
    let items = body["items"].as_array().unwrap();
    assert!(
        items.iter().any(|i| i["name"] == "movies"),
        "Index 'movies' not found in {:?}",
        items
    );

    // 5. Delete document
    let resp = client
        .delete(format!("{}/indexes/movies/documents/1", base))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.unwrap();
    assert!(body.get("taskID").is_some(), "Missing taskID on delete");
    assert!(body.get("deletedAt").is_some(), "Missing deletedAt");

    tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;

    // Verify deletion
    let resp = client
        .get(format!("{}/indexes/movies/documents/1", base))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 404, "Document should be deleted");

    // Brief pause to let async delete settle before index delete
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    // 6. Delete index (may fail under extreme parallel load due to resource contention;
    //    retry once to handle this)
    let resp = client
        .delete(format!("{}/indexes/movies", base))
        .send()
        .await
        .unwrap();
    let status = resp.status();
    if status == 200 {
        let body: Value = resp.json().await.unwrap();
        assert!(
            body.get("taskID").is_some(),
            "Missing taskID on index delete"
        );
    } else {
        // Retry after brief delay
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        let resp = client
            .delete(format!("{}/indexes/movies", base))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 200, "Delete index failed even after retry");
        let body: Value = resp.json().await.unwrap();
        assert!(
            body.get("taskID").is_some(),
            "Missing taskID on index delete"
        );
    }
}

// ---------------------------------------------------------------------------
// Single object POST (not array)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn quickstart_single_document() {
    let (addr, _temp) = common::spawn_server().await;
    let client = reqwest::Client::new();
    let base = format!("http://{}", addr);

    let resp = client
        .post(format!("{}/indexes/items/documents", base))
        .json(&json!({"objectID": "abc", "name": "Widget"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["objectIDs"].as_array().unwrap().len(), 1);
    assert_eq!(body["objectIDs"][0], "abc");
}

// ---------------------------------------------------------------------------
// Auto-generated objectID
// ---------------------------------------------------------------------------

#[tokio::test]
async fn quickstart_auto_id() {
    let (addr, _temp) = common::spawn_server().await;
    let client = reqwest::Client::new();
    let base = format!("http://{}", addr);

    let resp = client
        .post(format!("{}/indexes/items/documents", base))
        .json(&json!([{"name": "No ID provided"}]))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.unwrap();
    let ids = body["objectIDs"].as_array().unwrap();
    assert_eq!(ids.len(), 1);
    // Auto-generated UUID should be reasonably long
    let id_str = ids[0].as_str().unwrap();
    assert!(
        id_str.len() >= 32,
        "Expected UUID-length ID, got: {}",
        id_str
    );
}

// ---------------------------------------------------------------------------
// No auth headers required (even with admin key set)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn quickstart_no_auth_required_with_key() {
    // Spawn a server WITH an admin key set — quickstart should still work
    let (addr, _temp) = common::spawn_server_with_key(Some("test-admin-key")).await;
    let client = reqwest::Client::new();

    // No x-algolia-api-key or x-algolia-application-id headers
    let resp = client
        .get(format!("http://{}/indexes", addr))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        200,
        "Quickstart endpoints should bypass auth"
    );
}

// ---------------------------------------------------------------------------
// GET search with query params
// ---------------------------------------------------------------------------

#[tokio::test]
async fn quickstart_search_get_with_params() {
    let (addr, _temp) = common::spawn_server().await;
    let client = reqwest::Client::new();
    let base = format!("http://{}", addr);

    seed_index(
        &client,
        &base,
        "products",
        json!([
            {"objectID": "1", "title": "Laptop Pro", "price": 999},
            {"objectID": "2", "title": "Wireless Mouse", "price": 29},
            {"objectID": "3", "title": "USB Keyboard", "price": 49}
        ]),
    )
    .await;

    // Search with hitsPerPage=1
    let resp = client
        .get(format!("{}/indexes/products/search?q=&hitsPerPage=1", base))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.unwrap();
    assert_eq!(
        body["hits"].as_array().unwrap().len(),
        1,
        "hitsPerPage=1 should return 1 hit"
    );
    assert_eq!(body["hitsPerPage"].as_u64().unwrap(), 1);

    // Search with specific query
    let resp = client
        .get(format!("{}/indexes/products/search?q=laptop", base))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["nbHits"].as_u64().unwrap(), 1);
    assert_eq!(body["hits"][0]["title"], "Laptop Pro");
}

// ---------------------------------------------------------------------------
// POST search with JSON body
// ---------------------------------------------------------------------------

#[tokio::test]
async fn quickstart_search_post() {
    let (addr, _temp) = common::spawn_server().await;
    let client = reqwest::Client::new();
    let base = format!("http://{}", addr);

    seed_index(
        &client,
        &base,
        "books",
        json!([
            {"objectID": "1", "title": "The Hobbit", "author": "Tolkien"},
            {"objectID": "2", "title": "Dune", "author": "Herbert"}
        ]),
    )
    .await;

    let resp = client
        .post(format!("{}/indexes/books/search", base))
        .json(&json!({"query": "hobbit", "hitsPerPage": 10}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["nbHits"].as_u64().unwrap(), 1);
    assert_eq!(body["hits"][0]["title"], "The Hobbit");
}

// ---------------------------------------------------------------------------
// Typo tolerance via quickstart search
// ---------------------------------------------------------------------------

#[tokio::test]
async fn quickstart_typo_tolerance() {
    let (addr, _temp) = common::spawn_server().await;
    let client = reqwest::Client::new();
    let base = format!("http://{}", addr);

    seed_index(
        &client,
        &base,
        "movies",
        json!([
            {"objectID": "1", "title": "The Matrix", "year": 1999}
        ]),
    )
    .await;

    // Search with typo: "matrx" instead of "matrix"
    let resp = client
        .get(format!("{}/indexes/movies/search?q=matrx", base))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.unwrap();
    assert_eq!(
        body["nbHits"].as_u64().unwrap(),
        1,
        "Typo-tolerant search should find 'The Matrix' for query 'matrx'"
    );
    assert_eq!(body["hits"][0]["title"], "The Matrix");
}

// ---------------------------------------------------------------------------
// Empty search (browse all)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn quickstart_empty_search() {
    let (addr, _temp) = common::spawn_server().await;
    let client = reqwest::Client::new();
    let base = format!("http://{}", addr);

    seed_index(
        &client,
        &base,
        "items",
        json!([
            {"objectID": "1", "title": "Alpha"},
            {"objectID": "2", "title": "Beta"},
            {"objectID": "3", "title": "Gamma"}
        ]),
    )
    .await;

    // Empty query should return all documents
    let resp = client
        .get(format!("{}/indexes/items/search?q=", base))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.unwrap();
    assert_eq!(
        body["nbHits"].as_u64().unwrap(),
        3,
        "Empty query should return all 3 docs"
    );
}

// ---------------------------------------------------------------------------
// Task status endpoint
// ---------------------------------------------------------------------------

#[tokio::test]
async fn quickstart_task_status() {
    let (addr, _temp) = common::spawn_server().await;
    let client = reqwest::Client::new();
    let base = format!("http://{}", addr);

    // Add a doc to get a taskID
    let resp = client
        .post(format!("{}/indexes/test/documents", base))
        .json(&json!([{"objectID": "1", "title": "Test"}]))
        .send()
        .await
        .unwrap();
    let body: Value = resp.json().await.unwrap();
    let task_id = body["taskID"].as_i64().unwrap();

    // Wait for indexing
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    // Fetch task via quickstart endpoint using the numeric task ID
    // (tasks are stored under both the full string ID and the numeric ID)
    let resp = client
        .get(format!("{}/tasks/{}", base, task_id))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.unwrap();
    assert_eq!(
        body["status"], "published",
        "Task should be published after wait"
    );
}

// ---------------------------------------------------------------------------
// Migrate endpoint is wired (validates routing, not actual Algolia connection)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn quickstart_migrate_endpoint_wired() {
    let (addr, _temp) = common::spawn_server().await;
    let client = reqwest::Client::new();
    let base = format!("http://{}", addr);

    // Send a migrate request with fake credentials — should fail validation,
    // but the fact that we get a proper error (not 404) proves the route works.
    let resp = client
        .post(format!("{}/migrate", base))
        .json(&json!({
            "appId": "",
            "apiKey": "",
            "sourceIndex": ""
        }))
        .send()
        .await
        .unwrap();

    // Empty credentials should return 400, not 404 (proving route is wired)
    assert_ne!(
        resp.status().as_u16(),
        404,
        "Migrate endpoint should be routed (got 404)"
    );
}

// ---------------------------------------------------------------------------
// Invalid input handling
// ---------------------------------------------------------------------------

#[tokio::test]
async fn quickstart_invalid_document_body() {
    let (addr, _temp) = common::spawn_server().await;
    let client = reqwest::Client::new();
    let base = format!("http://{}", addr);

    // Send a string instead of object/array
    let resp = client
        .post(format!("{}/indexes/test/documents", base))
        .json(&json!("not an object"))
        .send()
        .await
        .unwrap();
    assert_ne!(resp.status(), 200, "String body should be rejected");
}

#[tokio::test]
async fn quickstart_search_nonexistent_index() {
    let (addr, _temp) = common::spawn_server().await;
    let client = reqwest::Client::new();
    let base = format!("http://{}", addr);

    let resp = client
        .get(format!("{}/indexes/nonexistent/search?q=hello", base))
        .send()
        .await
        .unwrap();
    // Should return an error (404 or similar), not crash
    assert!(
        resp.status().is_client_error() || resp.status().is_server_error(),
        "Searching nonexistent index should return error status, got {}",
        resp.status()
    );
}

// ---------------------------------------------------------------------------
// Bulk delete by ID array
// ---------------------------------------------------------------------------

#[tokio::test]
async fn quickstart_bulk_delete() {
    let (addr, _temp) = common::spawn_server().await;
    let client = reqwest::Client::new();
    let base = format!("http://{}", addr);

    // Seed 5 documents
    seed_index(
        &client,
        &base,
        "bulk_del",
        json!([
            {"objectID": "a", "title": "Alpha"},
            {"objectID": "b", "title": "Bravo"},
            {"objectID": "c", "title": "Charlie"},
            {"objectID": "d", "title": "Delta"},
            {"objectID": "e", "title": "Echo"}
        ]),
    )
    .await;

    // Bulk delete 3 of them
    let resp = client
        .post(format!("{}/indexes/bulk_del/documents/delete", base))
        .json(&json!(["a", "c", "e"]))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.unwrap();
    assert!(body.get("taskID").is_some(), "Missing taskID: {:?}", body);
    assert!(body.get("deletedAt").is_some(), "Missing deletedAt");

    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    // Verify deleted docs are gone
    for id in &["a", "c", "e"] {
        let resp = client
            .get(format!("{}/indexes/bulk_del/documents/{}", base, id))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 404, "Doc {} should be deleted", id);
    }

    // Verify remaining docs still exist
    for id in &["b", "d"] {
        let resp = client
            .get(format!("{}/indexes/bulk_del/documents/{}", base, id))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 200, "Doc {} should still exist", id);
    }
}

#[tokio::test]
async fn quickstart_bulk_delete_empty_array() {
    let (addr, _temp) = common::spawn_server().await;
    let client = reqwest::Client::new();
    let base = format!("http://{}", addr);

    // Seed a document first so the index exists
    seed_index(
        &client,
        &base,
        "empty_del",
        json!([{"objectID": "1", "title": "Test"}]),
    )
    .await;

    // Bulk delete with empty array — should succeed with noop
    let resp = client
        .post(format!("{}/indexes/empty_del/documents/delete", base))
        .json(&json!([]))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.unwrap();
    assert!(
        body.get("taskID").is_some(),
        "Empty delete should still return taskID"
    );

    // Original doc should still exist
    tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
    let resp = client
        .get(format!("{}/indexes/empty_del/documents/1", base))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        200,
        "Doc should still exist after empty delete"
    );
}

// ---------------------------------------------------------------------------
// Settings endpoints
// ---------------------------------------------------------------------------

#[tokio::test]
async fn quickstart_settings_get_defaults() {
    let (addr, _temp) = common::spawn_server().await;
    let client = reqwest::Client::new();
    let base = format!("http://{}", addr);

    // Seed an index so it exists
    seed_index(
        &client,
        &base,
        "settings_test",
        json!([{"objectID": "1", "title": "Test"}]),
    )
    .await;

    // GET settings — should return defaults
    let resp = client
        .get(format!("{}/indexes/settings_test/settings", base))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.unwrap();
    // Default settings should have attributesForFaceting as empty array
    assert!(
        body.get("attributesForFaceting").is_some()
            || body.get("attributes_for_faceting").is_some(),
        "Settings should include faceting config: {:?}",
        body
    );
}

#[tokio::test]
async fn quickstart_settings_put_and_get() {
    let (addr, _temp) = common::spawn_server().await;
    let client = reqwest::Client::new();
    let base = format!("http://{}", addr);

    // Seed an index
    seed_index(
        &client,
        &base,
        "settings_rw",
        json!([
            {"objectID": "1", "title": "Widget", "category": "tools", "price": 10},
            {"objectID": "2", "title": "Gadget", "category": "electronics", "price": 50}
        ]),
    )
    .await;

    // PUT settings — configure facets and searchable attributes
    let resp = client
        .put(format!("{}/indexes/settings_rw/settings", base))
        .json(&json!({
            "attributesForFaceting": ["category"],
            "searchableAttributes": ["title", "category"]
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        200,
        "PUT settings failed: {:?}",
        resp.text().await
    );

    // GET settings — verify they persisted
    let resp = client
        .get(format!("{}/indexes/settings_rw/settings", base))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.unwrap();

    let facets = body
        .get("attributesForFaceting")
        .or_else(|| body.get("attributes_for_faceting"));
    assert!(facets.is_some(), "Settings should have faceting config");
    let facets_arr = facets.unwrap().as_array().unwrap();
    assert_eq!(facets_arr.len(), 1);
    assert_eq!(facets_arr[0], "category");

    let searchable = body
        .get("searchableAttributes")
        .or_else(|| body.get("searchable_attributes"));
    assert!(
        searchable.is_some(),
        "Settings should have searchable attributes"
    );
    let searchable_arr = searchable.unwrap().as_array().unwrap();
    assert!(searchable_arr.contains(&json!("title")));
    assert!(searchable_arr.contains(&json!("category")));
}

#[tokio::test]
async fn quickstart_settings_no_auth_required() {
    // Spawn a server WITH an admin key — quickstart settings should still work
    let (addr, _temp) = common::spawn_server_with_key(Some("test-admin-key")).await;
    let client = reqwest::Client::new();

    // No auth headers — GET settings should bypass auth
    let resp = client
        .get(format!("http://{}/indexes/any/settings", addr))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200, "Quickstart settings should bypass auth");

    // PUT settings should also bypass auth
    let resp = client
        .put(format!("http://{}/indexes/any/settings", addr))
        .json(&json!({"attributesForFaceting": ["color"]}))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        200,
        "Quickstart settings PUT should bypass auth"
    );
}
