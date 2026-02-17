use serde_json::json;

mod common;
use common::spawn_server;

/// Helper: create a reqwest client with Algolia-style headers pre-set
fn algolia_client() -> reqwest::Client {
    reqwest::Client::new()
}

/// Helper: add standard Algolia headers to a request
fn h(rb: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
    rb.header("x-algolia-application-id", "test-app")
        .header("x-algolia-api-key", "test-key")
}

/// Helper: seed an index with test data and wait for indexing
async fn seed_index(base: &str, index: &str, records: Vec<serde_json::Value>) {
    let client = algolia_client();
    let requests: Vec<serde_json::Value> = records
        .into_iter()
        .map(|body| json!({"action": "addObject", "body": body}))
        .collect();
    h(client.post(format!("{}/1/indexes/{}/batch", base, index)))
        .json(&json!({"requests": requests}))
        .send()
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
}

#[tokio::test]
async fn test_sdk_endpoints_exist() {
    let (addr, _temp) = spawn_server().await;
    let client = reqwest::Client::new();
    let base = format!("http://{}", addr);

    // SDK v5 uses PUT for settings (not POST)
    let res = client
        .put(format!("{}/1/indexes/products/settings", base))
        .header("x-algolia-application-id", "test-app")
        .header("x-algolia-api-key", "test-key")
        .json(&json!({"attributesForFaceting": ["category"]}))
        .send()
        .await
        .unwrap();

    println!("PUT settings status: {}", res.status());
    assert!(
        res.status().is_success(),
        "PUT /settings returned {} — SDK v5 requires PUT support",
        res.status()
    );

    // SDK v5 batch format
    let res = client
        .post(format!("{}/1/indexes/products/batch", base))
        .header("x-algolia-application-id", "test-app")
        .header("x-algolia-api-key", "test-key")
        .json(&json!({
            "requests": [{
                "action": "addObject",
                "body": {"objectID": "1", "name": "Laptop"}
            }]
        }))
        .send()
        .await
        .unwrap();

    println!("POST batch status: {}", res.status());
    assert!(res.status().is_success());
}

/// POST /1/indexes/{indexName} — add record with auto-generated objectID
#[tokio::test]
async fn test_add_record_auto_id() {
    let (addr, _temp) = spawn_server().await;
    let client = reqwest::Client::new();
    let base = format!("http://{}", addr);

    let res = client
        .post(format!("{}/1/indexes/products", base))
        .header("x-algolia-application-id", "test-app")
        .header("x-algolia-api-key", "test-key")
        .json(&json!({"name": "Widget", "price": 9.99}))
        .send()
        .await
        .unwrap();

    assert!(
        res.status().is_success(),
        "POST /indexes/products returned {}",
        res.status()
    );
    let body: serde_json::Value = res.json().await.unwrap();

    // Must return objectID, taskID, createdAt
    assert!(
        body.get("objectID").is_some(),
        "missing objectID in response"
    );
    assert!(body.get("taskID").is_some(), "missing taskID in response");
    assert!(
        body.get("createdAt").is_some(),
        "missing createdAt in response"
    );

    // objectID should be a valid UUID
    let oid = body["objectID"].as_str().unwrap();
    assert!(
        uuid::Uuid::parse_str(oid).is_ok(),
        "objectID is not a UUID: {}",
        oid
    );

    // Verify the record was actually stored by fetching it
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    let res = client
        .get(format!("{}/1/indexes/products/{}", base, oid))
        .header("x-algolia-application-id", "test-app")
        .header("x-algolia-api-key", "test-key")
        .send()
        .await
        .unwrap();

    assert!(
        res.status().is_success(),
        "GET object returned {}",
        res.status()
    );
    let obj: serde_json::Value = res.json().await.unwrap();
    assert_eq!(obj["objectID"], oid);
    assert_eq!(obj["name"], "Widget");
}

/// POST /1/indexes/{indexName}/{objectID}/partial — partial update existing doc
#[tokio::test]
async fn test_partial_update_existing() {
    let (addr, _temp) = spawn_server().await;
    let client = reqwest::Client::new();
    let base = format!("http://{}", addr);

    // First, add a record via batch
    client
        .post(format!("{}/1/indexes/products/batch", base))
        .header("x-algolia-application-id", "test-app")
        .header("x-algolia-api-key", "test-key")
        .json(&json!({
            "requests": [{
                "action": "addObject",
                "body": {"objectID": "p1", "name": "Laptop", "price": 999}
            }]
        }))
        .send()
        .await
        .unwrap();

    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    // Partial update: change price, keep name
    let res = client
        .post(format!("{}/1/indexes/products/p1/partial", base))
        .header("x-algolia-application-id", "test-app")
        .header("x-algolia-api-key", "test-key")
        .json(&json!({"price": 799}))
        .send()
        .await
        .unwrap();

    assert!(
        res.status().is_success(),
        "partial update returned {}",
        res.status()
    );
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["objectID"], "p1");
    assert!(body.get("updatedAt").is_some());

    // Fetch and verify merge: name should still be "Laptop", price should be 799
    let res = client
        .get(format!("{}/1/indexes/products/p1", base))
        .header("x-algolia-application-id", "test-app")
        .header("x-algolia-api-key", "test-key")
        .send()
        .await
        .unwrap();

    assert!(res.status().is_success());
    let obj: serde_json::Value = res.json().await.unwrap();
    assert_eq!(
        obj["name"], "Laptop",
        "name should be preserved after partial update"
    );
    assert_eq!(obj["price"], 799, "price should be updated");
}

/// POST /1/indexes/{indexName}/{objectID}/partial with createIfNotExists=true (default)
#[tokio::test]
async fn test_partial_update_creates_when_missing() {
    let (addr, _temp) = spawn_server().await;
    let client = reqwest::Client::new();
    let base = format!("http://{}", addr);

    // Partial update a non-existent record (default createIfNotExists=true)
    let res = client
        .post(format!("{}/1/indexes/products/new-item/partial", base))
        .header("x-algolia-application-id", "test-app")
        .header("x-algolia-api-key", "test-key")
        .json(&json!({"name": "New Widget", "stock": 50}))
        .send()
        .await
        .unwrap();

    assert!(res.status().is_success());

    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    // Verify it was created
    let res = client
        .get(format!("{}/1/indexes/products/new-item", base))
        .header("x-algolia-application-id", "test-app")
        .header("x-algolia-api-key", "test-key")
        .send()
        .await
        .unwrap();

    assert!(
        res.status().is_success(),
        "object should exist after partial update create"
    );
    let obj: serde_json::Value = res.json().await.unwrap();
    assert_eq!(obj["objectID"], "new-item");
    assert_eq!(obj["name"], "New Widget");
}

/// POST /1/indexes/{indexName}/{objectID}/partial?createIfNotExists=false — no-op when missing
#[tokio::test]
async fn test_partial_update_noop_when_missing() {
    let (addr, _temp) = spawn_server().await;
    let client = reqwest::Client::new();
    let base = format!("http://{}", addr);

    // Partial update with createIfNotExists=false on non-existent record
    let res = client
        .post(format!(
            "{}/1/indexes/products/ghost/partial?createIfNotExists=false",
            base
        ))
        .header("x-algolia-application-id", "test-app")
        .header("x-algolia-api-key", "test-key")
        .json(&json!({"name": "Ghost"}))
        .send()
        .await
        .unwrap();

    // Algolia returns 200 even for no-op
    assert!(res.status().is_success());

    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    // Verify it was NOT created
    let res = client
        .get(format!("{}/1/indexes/products/ghost", base))
        .header("x-algolia-application-id", "test-app")
        .header("x-algolia-api-key", "test-key")
        .send()
        .await
        .unwrap();

    assert_eq!(
        res.status(),
        404,
        "object should not exist when createIfNotExists=false"
    );
}

/// Batch addObject without objectID should auto-generate a UUID
#[tokio::test]
async fn test_batch_add_object_auto_id() {
    let (addr, _temp) = spawn_server().await;
    let client = reqwest::Client::new();
    let base = format!("http://{}", addr);

    let res = client
        .post(format!("{}/1/indexes/products/batch", base))
        .header("x-algolia-application-id", "test-app")
        .header("x-algolia-api-key", "test-key")
        .json(&json!({
            "requests": [{
                "action": "addObject",
                "body": {"name": "Auto-ID Widget"}
            }]
        }))
        .send()
        .await
        .unwrap();

    assert!(res.status().is_success());
    let body: serde_json::Value = res.json().await.unwrap();

    let ids = body["objectIDs"].as_array().unwrap();
    assert_eq!(ids.len(), 1);

    let auto_id = ids[0].as_str().unwrap();
    assert!(
        uuid::Uuid::parse_str(auto_id).is_ok(),
        "batch addObject without objectID should generate a UUID, got: {}",
        auto_id
    );
}

/// Unknown search params should not cause 400 errors (serde ignores them)
#[tokio::test]
async fn test_unknown_search_params_accepted() {
    let (addr, _temp) = spawn_server().await;
    let client = reqwest::Client::new();
    let base = format!("http://{}", addr);

    // Create the index first so search doesn't 404
    client
        .post(format!("{}/1/indexes/products/batch", base))
        .header("x-algolia-application-id", "test-app")
        .header("x-algolia-api-key", "test-key")
        .json(&json!({
            "requests": [{"action": "addObject", "body": {"objectID": "1", "name": "Test"}}]
        }))
        .send()
        .await
        .unwrap();

    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    let res = client
        .post(format!("{}/1/indexes/products/query", base))
        .header("x-algolia-application-id", "test-app")
        .header("x-algolia-api-key", "test-key")
        .json(&json!({
            "query": "test",
            "typoTolerance": true,
            "optionalFilters": ["brand:Apple"],
            "queryType": "prefixLast",
            "removeWordsIfNoResults": "lastWords",
            "advancedSyntax": true,
            "enablePersonalization": false,
            "relevancyStrictness": 90,
            "decompoundQuery": false,
            "enableReRanking": false,
            "mode": "keywordSearch"
        }))
        .send()
        .await
        .unwrap();

    assert!(
        res.status().is_success(),
        "search with unknown params should not 400, got {}",
        res.status()
    );
}

// ──────────────────────────────────────────────────────────────────
// Settings roundtrip (PUT + GET)
// ──────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_settings_roundtrip() {
    let (addr, _temp) = spawn_server().await;
    let client = algolia_client();
    let base = format!("http://{}", addr);

    // Create index
    seed_index(
        &base,
        "products",
        vec![json!({"objectID": "1", "name": "A"})],
    )
    .await;

    // PUT settings
    let res = h(client.put(format!("{}/1/indexes/products/settings", base)))
        .json(&json!({
            "searchableAttributes": ["name", "description"],
            "attributesForFaceting": ["category", "brand"]
        }))
        .send()
        .await
        .unwrap();
    assert!(res.status().is_success(), "PUT settings: {}", res.status());

    // GET settings — verify they persisted
    let res = h(client.get(format!("{}/1/indexes/products/settings", base)))
        .send()
        .await
        .unwrap();
    assert!(res.status().is_success(), "GET settings: {}", res.status());
    let settings: serde_json::Value = res.json().await.unwrap();
    assert!(
        settings.get("searchableAttributes").is_some(),
        "searchableAttributes missing"
    );
    assert!(
        settings.get("attributesForFaceting").is_some(),
        "attributesForFaceting missing"
    );
}

// ──────────────────────────────────────────────────────────────────
// PUT object (full replace)
// ──────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_put_object_replaces_fully() {
    let (addr, _temp) = spawn_server().await;
    let client = algolia_client();
    let base = format!("http://{}", addr);

    seed_index(
        &base,
        "products",
        vec![json!({"objectID": "p1", "name": "Laptop", "price": 999, "brand": "Acme"})],
    )
    .await;

    // PUT replaces the entire object
    let res = h(client.put(format!("{}/1/indexes/products/p1", base)))
        .json(&json!({"name": "Laptop Pro", "price": 1299}))
        .send()
        .await
        .unwrap();
    assert!(res.status().is_success(), "PUT object: {}", res.status());
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["objectID"], "p1");
    assert!(body.get("updatedAt").is_some());

    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    // Verify: brand should be gone (full replace, not merge)
    let res = h(client.get(format!("{}/1/indexes/products/p1", base)))
        .send()
        .await
        .unwrap();
    let obj: serde_json::Value = res.json().await.unwrap();
    assert_eq!(obj["name"], "Laptop Pro");
    assert_eq!(obj["price"], 1299);
    assert!(
        obj.get("brand").is_none(),
        "brand should be gone after full PUT replace"
    );
}

// ──────────────────────────────────────────────────────────────────
// DELETE object
// ──────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_delete_object() {
    let (addr, _temp) = spawn_server().await;
    let client = algolia_client();
    let base = format!("http://{}", addr);

    seed_index(
        &base,
        "products",
        vec![json!({"objectID": "d1", "name": "Delete Me"})],
    )
    .await;

    let res = h(client.delete(format!("{}/1/indexes/products/d1", base)))
        .send()
        .await
        .unwrap();
    assert!(res.status().is_success(), "DELETE object: {}", res.status());
    let body: serde_json::Value = res.json().await.unwrap();
    assert!(body.get("taskID").is_some());
    assert!(body.get("deletedAt").is_some());

    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    // Verify gone
    let res = h(client.get(format!("{}/1/indexes/products/d1", base)))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 404, "deleted object should return 404");
}

// ──────────────────────────────────────────────────────────────────
// Batch deleteObject
// ──────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_batch_delete_object() {
    let (addr, _temp) = spawn_server().await;
    let client = algolia_client();
    let base = format!("http://{}", addr);

    seed_index(
        &base,
        "products",
        vec![
            json!({"objectID": "bd1", "name": "A"}),
            json!({"objectID": "bd2", "name": "B"}),
        ],
    )
    .await;

    let res = h(client.post(format!("{}/1/indexes/products/batch", base)))
        .json(&json!({
            "requests": [
                {"action": "deleteObject", "body": {"objectID": "bd1"}},
                {"action": "deleteObject", "body": {"objectID": "bd2"}}
            ]
        }))
        .send()
        .await
        .unwrap();
    assert!(res.status().is_success(), "batch delete: {}", res.status());

    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    let res = h(client.get(format!("{}/1/indexes/products/bd1", base)))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 404);
}

// ──────────────────────────────────────────────────────────────────
// Batch updateObject (full replace via batch)
// ──────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_batch_update_object() {
    let (addr, _temp) = spawn_server().await;
    let client = algolia_client();
    let base = format!("http://{}", addr);

    seed_index(
        &base,
        "products",
        vec![json!({"objectID": "u1", "name": "Old", "color": "red"})],
    )
    .await;

    let res = h(client.post(format!("{}/1/indexes/products/batch", base)))
        .json(&json!({
            "requests": [{
                "action": "updateObject",
                "body": {"objectID": "u1", "name": "New"}
            }]
        }))
        .send()
        .await
        .unwrap();
    assert!(res.status().is_success());

    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    let res = h(client.get(format!("{}/1/indexes/products/u1", base)))
        .send()
        .await
        .unwrap();
    let obj: serde_json::Value = res.json().await.unwrap();
    assert_eq!(obj["name"], "New");
}

// ──────────────────────────────────────────────────────────────────
// List indices
// ──────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_list_indices() {
    let (addr, _temp) = spawn_server().await;
    let client = algolia_client();
    let base = format!("http://{}", addr);

    seed_index(&base, "alpha", vec![json!({"objectID": "1", "x": 1})]).await;
    seed_index(&base, "beta", vec![json!({"objectID": "1", "x": 2})]).await;

    let res = h(client.get(format!("{}/1/indexes", base)))
        .send()
        .await
        .unwrap();
    assert!(res.status().is_success(), "list indices: {}", res.status());
    let body: serde_json::Value = res.json().await.unwrap();
    let items = body["items"].as_array().unwrap();
    let names: Vec<&str> = items.iter().filter_map(|i| i["name"].as_str()).collect();
    assert!(
        names.contains(&"alpha"),
        "should list alpha, got {:?}",
        names
    );
    assert!(names.contains(&"beta"), "should list beta, got {:?}", names);
}

// ──────────────────────────────────────────────────────────────────
// Delete index
// ──────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_delete_index() {
    let (addr, _temp) = spawn_server().await;
    let client = algolia_client();
    let base = format!("http://{}", addr);

    seed_index(&base, "temp", vec![json!({"objectID": "1", "x": 1})]).await;

    let res = h(client.delete(format!("{}/1/indexes/temp", base)))
        .send()
        .await
        .unwrap();
    assert!(res.status().is_success(), "delete index: {}", res.status());

    // Search should fail or return 0
    let res = h(client.post(format!("{}/1/indexes/temp/query", base)))
        .json(&json!({"query": ""}))
        .send()
        .await
        .unwrap();
    // After deletion, searching the index must either 404 or return 0 hits
    let status = res.status();
    if status.is_success() {
        let body: serde_json::Value = res.json().await.unwrap();
        assert_eq!(body["nbHits"], 0, "deleted index should have 0 hits");
    } else {
        assert!(
            status == 404 || status == 400,
            "expected 404 or 400 for deleted index, got {}",
            status
        );
    }
}

// ──────────────────────────────────────────────────────────────────
// Clear index (remove all records, keep settings)
// ──────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_clear_index() {
    let (addr, _temp) = spawn_server().await;
    let client = algolia_client();
    let base = format!("http://{}", addr);

    seed_index(
        &base,
        "products",
        vec![
            json!({"objectID": "1", "name": "A"}),
            json!({"objectID": "2", "name": "B"}),
        ],
    )
    .await;

    let res = h(client.post(format!("{}/1/indexes/products/clear", base)))
        .send()
        .await
        .unwrap();
    assert!(res.status().is_success(), "clear index: {}", res.status());

    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    let res = h(client.post(format!("{}/1/indexes/products/query", base)))
        .json(&json!({"query": ""}))
        .send()
        .await
        .unwrap();
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["nbHits"], 0, "cleared index should have 0 hits");
}

// ──────────────────────────────────────────────────────────────────
// Multi-index search (POST /1/indexes/*/queries)
// ──────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_multi_index_search() {
    let (addr, _temp) = spawn_server().await;
    let client = algolia_client();
    let base = format!("http://{}", addr);

    seed_index(
        &base,
        "products",
        vec![json!({"objectID": "1", "name": "Laptop"})],
    )
    .await;
    seed_index(
        &base,
        "articles",
        vec![json!({"objectID": "1", "name": "Review"})],
    )
    .await;

    let res = h(client.post(format!("{}/1/indexes/*/queries", base)))
        .json(&json!({
            "requests": [
                {"indexName": "products", "query": "laptop"},
                {"indexName": "articles", "query": "review"}
            ]
        }))
        .send()
        .await
        .unwrap();
    assert!(
        res.status().is_success(),
        "multi-index search: {}",
        res.status()
    );
    let body: serde_json::Value = res.json().await.unwrap();
    let results = body["results"].as_array().unwrap();
    assert_eq!(results.len(), 2, "should return 2 result sets");
}

// ──────────────────────────────────────────────────────────────────
// Multi-index getObjects (POST /1/indexes/*/objects)
// ──────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_multi_index_get_objects() {
    let (addr, _temp) = spawn_server().await;
    let client = algolia_client();
    let base = format!("http://{}", addr);

    seed_index(
        &base,
        "products",
        vec![json!({"objectID": "p1", "name": "Laptop"})],
    )
    .await;
    seed_index(
        &base,
        "articles",
        vec![json!({"objectID": "a1", "name": "Review"})],
    )
    .await;

    let res = h(client.post(format!("{}/1/indexes/products/objects", base)))
        .json(&json!({
            "requests": [
                {"indexName": "products", "objectID": "p1"},
                {"indexName": "articles", "objectID": "a1"}
            ]
        }))
        .send()
        .await
        .unwrap();
    assert!(
        res.status().is_success(),
        "multi-index getObjects: {}",
        res.status()
    );
    let body: serde_json::Value = res.json().await.unwrap();
    let results = body["results"].as_array().unwrap();
    assert_eq!(results.len(), 2);
    assert_eq!(results[0]["objectID"], "p1");
    assert_eq!(results[1]["objectID"], "a1");
}

// ──────────────────────────────────────────────────────────────────
// Synonyms CRUD
// ──────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_synonyms_crud() {
    let (addr, _temp) = spawn_server().await;
    let client = algolia_client();
    let base = format!("http://{}", addr);

    seed_index(
        &base,
        "products",
        vec![json!({"objectID": "1", "name": "TV"})],
    )
    .await;

    // Save synonym via PUT
    let res = h(client.put(format!("{}/1/indexes/products/synonyms/syn1", base)))
        .json(&json!({
            "objectID": "syn1",
            "type": "synonym",
            "synonyms": ["tv", "television", "telly"]
        }))
        .send()
        .await
        .unwrap();
    assert!(res.status().is_success(), "PUT synonym: {}", res.status());

    // GET synonym
    let res = h(client.get(format!("{}/1/indexes/products/synonyms/syn1", base)))
        .send()
        .await
        .unwrap();
    assert!(res.status().is_success(), "GET synonym: {}", res.status());
    let syn: serde_json::Value = res.json().await.unwrap();
    assert_eq!(syn["objectID"], "syn1");

    // Search synonyms
    let res = h(client.post(format!("{}/1/indexes/products/synonyms/search", base)))
        .json(&json!({"query": ""}))
        .send()
        .await
        .unwrap();
    assert!(
        res.status().is_success(),
        "search synonyms: {}",
        res.status()
    );
    let body: serde_json::Value = res.json().await.unwrap();
    assert!(body.get("hits").is_some() || body.get("nbHits").is_some());

    // Delete synonym
    let res = h(client.delete(format!("{}/1/indexes/products/synonyms/syn1", base)))
        .send()
        .await
        .unwrap();
    assert!(
        res.status().is_success(),
        "DELETE synonym: {}",
        res.status()
    );

    // Verify gone
    let res = h(client.get(format!("{}/1/indexes/products/synonyms/syn1", base)))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 404, "deleted synonym should return 404");
}

// ──────────────────────────────────────────────────────────────────
// Synonyms batch save + clear
// ──────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_synonyms_batch_and_clear() {
    let (addr, _temp) = spawn_server().await;
    let client = algolia_client();
    let base = format!("http://{}", addr);

    seed_index(
        &base,
        "products",
        vec![json!({"objectID": "1", "name": "TV"})],
    )
    .await;

    // Batch save
    let res = h(client.post(format!("{}/1/indexes/products/synonyms/batch", base)))
        .json(&json!([
            {"objectID": "s1", "type": "synonym", "synonyms": ["phone", "mobile"]},
            {"objectID": "s2", "type": "synonym", "synonyms": ["laptop", "notebook"]}
        ]))
        .send()
        .await
        .unwrap();
    assert!(
        res.status().is_success(),
        "batch synonyms: {}",
        res.status()
    );

    // Verify they exist
    let res = h(client.get(format!("{}/1/indexes/products/synonyms/s1", base)))
        .send()
        .await
        .unwrap();
    assert!(res.status().is_success());

    // Clear all synonyms
    let res = h(client.post(format!("{}/1/indexes/products/synonyms/clear", base)))
        .send()
        .await
        .unwrap();
    assert!(
        res.status().is_success(),
        "clear synonyms: {}",
        res.status()
    );

    // Verify cleared
    let res = h(client.get(format!("{}/1/indexes/products/synonyms/s1", base)))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 404, "synonym should be cleared");
}

// ──────────────────────────────────────────────────────────────────
// Rules CRUD
// ──────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_rules_crud() {
    let (addr, _temp) = spawn_server().await;
    let client = algolia_client();
    let base = format!("http://{}", addr);

    seed_index(
        &base,
        "products",
        vec![json!({"objectID": "1", "name": "Laptop"})],
    )
    .await;

    // Save rule via PUT
    let res = h(client.put(format!("{}/1/indexes/products/rules/rule1", base)))
        .json(&json!({
            "objectID": "rule1",
            "conditions": [{"anchoring": "contains", "pattern": "laptop"}],
            "consequence": {
                "params": {"query": "laptop computer"}
            }
        }))
        .send()
        .await
        .unwrap();
    assert!(res.status().is_success(), "PUT rule: {}", res.status());

    // GET rule
    let res = h(client.get(format!("{}/1/indexes/products/rules/rule1", base)))
        .send()
        .await
        .unwrap();
    assert!(res.status().is_success(), "GET rule: {}", res.status());
    let rule: serde_json::Value = res.json().await.unwrap();
    assert_eq!(rule["objectID"], "rule1");

    // Search rules
    let res = h(client.post(format!("{}/1/indexes/products/rules/search", base)))
        .json(&json!({"query": ""}))
        .send()
        .await
        .unwrap();
    assert!(res.status().is_success(), "search rules: {}", res.status());

    // Delete rule
    let res = h(client.delete(format!("{}/1/indexes/products/rules/rule1", base)))
        .send()
        .await
        .unwrap();
    assert!(res.status().is_success(), "DELETE rule: {}", res.status());

    // Verify gone
    let res = h(client.get(format!("{}/1/indexes/products/rules/rule1", base)))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 404, "deleted rule should return 404");
}

// ──────────────────────────────────────────────────────────────────
// Rules batch + clear
// ──────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_rules_batch_and_clear() {
    let (addr, _temp) = spawn_server().await;
    let client = algolia_client();
    let base = format!("http://{}", addr);

    seed_index(
        &base,
        "products",
        vec![json!({"objectID": "1", "name": "X"})],
    )
    .await;

    let res = h(client.post(format!("{}/1/indexes/products/rules/batch", base)))
        .json(&json!([
            {
                "objectID": "r1",
                "conditions": [{"anchoring": "contains", "pattern": "sale"}],
                "consequence": {"params": {"filters": "onSale:true"}}
            },
            {
                "objectID": "r2",
                "conditions": [{"anchoring": "contains", "pattern": "new"}],
                "consequence": {"params": {"filters": "isNew:true"}}
            }
        ]))
        .send()
        .await
        .unwrap();
    assert!(res.status().is_success(), "batch rules: {}", res.status());

    let res = h(client.get(format!("{}/1/indexes/products/rules/r1", base)))
        .send()
        .await
        .unwrap();
    assert!(res.status().is_success());

    // Clear
    let res = h(client.post(format!("{}/1/indexes/products/rules/clear", base)))
        .send()
        .await
        .unwrap();
    assert!(res.status().is_success(), "clear rules: {}", res.status());

    let res = h(client.get(format!("{}/1/indexes/products/rules/r1", base)))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 404, "rule should be cleared");
}

// ──────────────────────────────────────────────────────────────────
// Browse endpoint
// ──────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_browse_index() {
    let (addr, _temp) = spawn_server().await;
    let client = algolia_client();
    let base = format!("http://{}", addr);

    seed_index(
        &base,
        "products",
        vec![
            json!({"objectID": "1", "name": "Alpha"}),
            json!({"objectID": "2", "name": "Beta"}),
            json!({"objectID": "3", "name": "Gamma"}),
        ],
    )
    .await;

    let res = h(client.post(format!("{}/1/indexes/products/browse", base)))
        .json(&json!({"query": "", "hitsPerPage": 2}))
        .send()
        .await
        .unwrap();
    assert!(res.status().is_success(), "browse: {}", res.status());
    let body: serde_json::Value = res.json().await.unwrap();
    let hits = body["hits"]
        .as_array()
        .expect("browse should return hits array");
    assert_eq!(hits.len(), 2, "hitsPerPage=2 should return exactly 2 hits");

    // Cursor must be a non-empty string for pagination when more results exist
    let cursor = body["cursor"]
        .as_str()
        .expect("cursor must be present when there are more results");
    assert!(!cursor.is_empty(), "cursor must be a non-empty string");

    // Verify cursor works for fetching the next page
    let res2 = h(client.post(format!("{}/1/indexes/products/browse", base)))
        .json(&json!({"cursor": cursor}))
        .send()
        .await
        .unwrap();
    assert!(
        res2.status().is_success(),
        "browse with cursor should succeed"
    );
    let body2: serde_json::Value = res2.json().await.unwrap();
    let hits2 = body2["hits"]
        .as_array()
        .expect("cursor page should return hits");
    assert!(
        !hits2.is_empty(),
        "cursor page should have remaining results"
    );
}

// ──────────────────────────────────────────────────────────────────
// Search response format compliance
// ──────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_search_response_format() {
    let (addr, _temp) = spawn_server().await;
    let client = algolia_client();
    let base = format!("http://{}", addr);

    seed_index(
        &base,
        "products",
        vec![json!({"objectID": "1", "name": "Laptop", "price": 999})],
    )
    .await;

    let res = h(client.post(format!("{}/1/indexes/products/query", base)))
        .json(&json!({"query": "laptop"}))
        .send()
        .await
        .unwrap();
    assert!(res.status().is_success());
    let body: serde_json::Value = res.json().await.unwrap();

    // Algolia SDK expects these fields in every search response
    assert!(body.get("hits").is_some(), "missing 'hits'");
    assert!(body.get("nbHits").is_some(), "missing 'nbHits'");
    assert!(body.get("page").is_some(), "missing 'page'");
    assert!(body.get("nbPages").is_some(), "missing 'nbPages'");
    assert!(body.get("hitsPerPage").is_some(), "missing 'hitsPerPage'");
    assert!(
        body.get("processingTimeMS").is_some(),
        "missing 'processingTimeMS'"
    );
    assert!(body.get("query").is_some(), "missing 'query'");
    assert!(body.get("params").is_some(), "missing 'params'");
    assert!(
        body.get("exhaustiveNbHits").is_some(),
        "missing 'exhaustiveNbHits'"
    );

    // Verify hits contain objectID and _highlightResult
    let hits = body["hits"].as_array().unwrap();
    assert!(!hits.is_empty(), "should have at least 1 hit");
    assert!(hits[0].get("objectID").is_some(), "hit missing objectID");
    assert!(
        hits[0].get("_highlightResult").is_some(),
        "hit missing _highlightResult"
    );
}

// ──────────────────────────────────────────────────────────────────
// Faceted search
// ──────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_faceted_search() {
    let (addr, _temp) = spawn_server().await;
    let client = algolia_client();
    let base = format!("http://{}", addr);

    // Configure facets
    h(client.put(format!("{}/1/indexes/products/settings", base)))
        .json(&json!({"attributesForFaceting": ["category"]}))
        .send()
        .await
        .unwrap();

    seed_index(
        &base,
        "products",
        vec![
            json!({"objectID": "1", "name": "Laptop", "category": "Electronics"}),
            json!({"objectID": "2", "name": "Phone", "category": "Electronics"}),
            json!({"objectID": "3", "name": "Shirt", "category": "Clothing"}),
        ],
    )
    .await;

    let res = h(client.post(format!("{}/1/indexes/products/query", base)))
        .json(&json!({"query": "", "facets": ["category"]}))
        .send()
        .await
        .unwrap();
    assert!(res.status().is_success());
    let body: serde_json::Value = res.json().await.unwrap();
    assert!(body.get("facets").is_some(), "missing facets in response");
    let facets = &body["facets"];
    assert!(facets.get("category").is_some(), "missing category facet");
}

// ──────────────────────────────────────────────────────────────────
// deleteByQuery
// ──────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_delete_by_query() {
    let (addr, _temp) = spawn_server().await;
    let client = algolia_client();
    let base = format!("http://{}", addr);

    h(client.put(format!("{}/1/indexes/products/settings", base)))
        .json(&json!({"attributesForFaceting": ["filterOnly(category)"]}))
        .send()
        .await
        .unwrap();

    seed_index(
        &base,
        "products",
        vec![
            json!({"objectID": "1", "name": "A", "category": "x"}),
            json!({"objectID": "2", "name": "B", "category": "y"}),
            json!({"objectID": "3", "name": "C", "category": "x"}),
        ],
    )
    .await;

    let res = h(client.post(format!("{}/1/indexes/products/deleteByQuery", base)))
        .json(&json!({"filters": "category:x"}))
        .send()
        .await
        .unwrap();
    assert!(res.status().is_success(), "deleteByQuery: {}", res.status());

    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    // Only "B" should remain
    let res = h(client.get(format!("{}/1/indexes/products/1", base)))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 404, "object 1 should be deleted");

    let res = h(client.get(format!("{}/1/indexes/products/2", base)))
        .send()
        .await
        .unwrap();
    assert!(res.status().is_success(), "object 2 should still exist");
}

// ── Partial Update Built-in Operations ──────────────────────────────────

#[tokio::test]
async fn test_partial_update_increment() {
    let (addr, _temp) = spawn_server().await;
    let client = algolia_client();
    let base = format!("http://{}", addr);

    seed_index(
        &base,
        "products",
        vec![json!({"objectID": "p1", "name": "Widget", "stock": 10, "price": 19.99})],
    )
    .await;

    // Increment integer field
    let res = h(client.post(format!("{}/1/indexes/products/p1/partial", base)))
        .json(&json!({"stock": {"_operation": "Increment", "value": 5}}))
        .send()
        .await
        .unwrap();
    assert!(res.status().is_success());

    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    let res = h(client.get(format!("{}/1/indexes/products/p1", base)))
        .send()
        .await
        .unwrap();
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["stock"], 15, "stock should be 10 + 5 = 15");
    assert_eq!(body["name"], "Widget", "other fields preserved");
}

#[tokio::test]
async fn test_partial_update_decrement() {
    let (addr, _temp) = spawn_server().await;
    let client = algolia_client();
    let base = format!("http://{}", addr);

    seed_index(
        &base,
        "products",
        vec![json!({"objectID": "p1", "stock": 10})],
    )
    .await;

    let res = h(client.post(format!("{}/1/indexes/products/p1/partial", base)))
        .json(&json!({"stock": {"_operation": "Decrement", "value": 3}}))
        .send()
        .await
        .unwrap();
    assert!(res.status().is_success());

    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    let res = h(client.get(format!("{}/1/indexes/products/p1", base)))
        .send()
        .await
        .unwrap();
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["stock"], 7, "stock should be 10 - 3 = 7");
}

#[tokio::test]
async fn test_partial_update_add_to_array() {
    let (addr, _temp) = spawn_server().await;
    let client = algolia_client();
    let base = format!("http://{}", addr);

    seed_index(
        &base,
        "products",
        vec![json!({"objectID": "p1", "tags": ["red", "sale"]})],
    )
    .await;

    let res = h(client.post(format!("{}/1/indexes/products/p1/partial", base)))
        .json(&json!({"tags": {"_operation": "Add", "value": "new-arrival"}}))
        .send()
        .await
        .unwrap();
    assert!(res.status().is_success());

    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    let res = h(client.get(format!("{}/1/indexes/products/p1", base)))
        .send()
        .await
        .unwrap();
    let body: serde_json::Value = res.json().await.unwrap();
    let tags = body["tags"].as_array().expect("tags should be array");
    assert_eq!(tags.len(), 3, "should have 3 tags after Add");
    assert!(
        tags.iter().any(|t| t == "new-arrival"),
        "new-arrival should be in tags"
    );
}

#[tokio::test]
async fn test_partial_update_remove_from_array() {
    let (addr, _temp) = spawn_server().await;
    let client = algolia_client();
    let base = format!("http://{}", addr);

    seed_index(
        &base,
        "products",
        vec![json!({"objectID": "p1", "tags": ["red", "sale", "clearance"]})],
    )
    .await;

    let res = h(client.post(format!("{}/1/indexes/products/p1/partial", base)))
        .json(&json!({"tags": {"_operation": "Remove", "value": "sale"}}))
        .send()
        .await
        .unwrap();
    assert!(res.status().is_success());

    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    let res = h(client.get(format!("{}/1/indexes/products/p1", base)))
        .send()
        .await
        .unwrap();
    let body: serde_json::Value = res.json().await.unwrap();
    let tags = body["tags"].as_array().expect("tags should be array");
    assert_eq!(tags.len(), 2, "should have 2 tags after Remove");
    assert!(
        !tags.iter().any(|t| t == "sale"),
        "sale should be removed from tags"
    );
}

#[tokio::test]
async fn test_partial_update_add_unique() {
    let (addr, _temp) = spawn_server().await;
    let client = algolia_client();
    let base = format!("http://{}", addr);

    seed_index(
        &base,
        "products",
        vec![json!({"objectID": "p1", "tags": ["red", "sale"]})],
    )
    .await;

    // AddUnique with a value that already exists — should not duplicate
    let res = h(client.post(format!("{}/1/indexes/products/p1/partial", base)))
        .json(&json!({"tags": {"_operation": "AddUnique", "value": "red"}}))
        .send()
        .await
        .unwrap();
    assert!(res.status().is_success());

    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    let res = h(client.get(format!("{}/1/indexes/products/p1", base)))
        .send()
        .await
        .unwrap();
    let body: serde_json::Value = res.json().await.unwrap();
    let tags = body["tags"].as_array().expect("tags should be array");
    assert_eq!(tags.len(), 2, "should still have 2 tags (no duplicate)");

    // AddUnique with a new value — should add
    let res = h(client.post(format!("{}/1/indexes/products/p1/partial", base)))
        .json(&json!({"tags": {"_operation": "AddUnique", "value": "exclusive"}}))
        .send()
        .await
        .unwrap();
    assert!(res.status().is_success());

    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    let res = h(client.get(format!("{}/1/indexes/products/p1", base)))
        .send()
        .await
        .unwrap();
    let body: serde_json::Value = res.json().await.unwrap();
    let tags = body["tags"].as_array().expect("tags should be array");
    assert_eq!(tags.len(), 3, "should have 3 tags after AddUnique");
    assert!(
        tags.iter().any(|t| t == "exclusive"),
        "exclusive should be in tags"
    );
}

#[tokio::test]
async fn test_partial_update_increment_on_missing_field() {
    let (addr, _temp) = spawn_server().await;
    let client = algolia_client();
    let base = format!("http://{}", addr);

    seed_index(
        &base,
        "products",
        vec![json!({"objectID": "p1", "name": "Widget"})],
    )
    .await;

    // Increment a field that doesn't exist yet
    let res = h(client.post(format!("{}/1/indexes/products/p1/partial", base)))
        .json(&json!({"views": {"_operation": "Increment", "value": 1}}))
        .send()
        .await
        .unwrap();
    assert!(res.status().is_success());

    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    let res = h(client.get(format!("{}/1/indexes/products/p1", base)))
        .send()
        .await
        .unwrap();
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["views"], 1, "missing field should start at value");
    assert_eq!(body["name"], "Widget", "other fields preserved");
}

#[tokio::test]
async fn test_partial_update_batch_operations() {
    let (addr, _temp) = spawn_server().await;
    let client = algolia_client();
    let base = format!("http://{}", addr);

    seed_index(
        &base,
        "products",
        vec![json!({"objectID": "p1", "stock": 100, "tags": ["featured"]})],
    )
    .await;

    // Batch: increment stock and add a tag in one partialUpdateObject
    let res = h(client.post(format!("{}/1/indexes/products/batch", base)))
        .json(&json!({
            "requests": [{
                "action": "partialUpdateObject",
                "body": {
                    "objectID": "p1",
                    "stock": {"_operation": "Decrement", "value": 10},
                    "tags": {"_operation": "Add", "value": "on-sale"}
                }
            }]
        }))
        .send()
        .await
        .unwrap();
    assert!(res.status().is_success());

    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    let res = h(client.get(format!("{}/1/indexes/products/p1", base)))
        .send()
        .await
        .unwrap();
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["stock"], 90, "stock should be 100 - 10 = 90");
    let tags = body["tags"].as_array().expect("tags should be array");
    assert_eq!(tags.len(), 2);
    assert!(tags.iter().any(|t| t == "on-sale"));
}

// ── Snippet Tests ───────────────────────────────────────────────────────

#[tokio::test]
async fn test_snippet_result_basic() {
    let (addr, _temp) = spawn_server().await;
    let client = algolia_client();
    let base = format!("http://{}", addr);

    seed_index(
        &base,
        "products",
        vec![json!({
            "objectID": "p1",
            "name": "Blue Wireless Earbuds",
            "description": "These amazing blue wireless earbuds deliver crystal clear sound quality with deep bass and active noise cancellation for an immersive listening experience on the go"
        })],
    )
    .await;

    let res = h(client.post(format!("{}/1/indexes/products/query", base)))
        .json(&json!({
            "query": "blue earbuds",
            "attributesToSnippet": ["description:5"]
        }))
        .send()
        .await
        .unwrap();
    assert!(res.status().is_success());

    let body: serde_json::Value = res.json().await.unwrap();
    let hits = body["hits"].as_array().expect("hits should be array");
    assert!(!hits.is_empty(), "should find at least one hit");

    let hit = &hits[0];
    let snippet = &hit["_snippetResult"];
    assert!(snippet.is_object(), "_snippetResult should be present");

    let desc_snippet = &snippet["description"];
    assert!(desc_snippet.is_object(), "description snippet should exist");
    assert!(
        desc_snippet["value"].is_string(),
        "snippet value should be string"
    );

    let snippet_value = desc_snippet["value"].as_str().unwrap();
    // Snippet should be truncated (shorter than full description) and have highlight tags
    assert!(
        snippet_value.contains("<em>") || snippet_value.contains('\u{2026}'),
        "snippet should have highlight tags or ellipsis: {}",
        snippet_value
    );
}

#[tokio::test]
async fn test_snippet_with_highlight() {
    let (addr, _temp) = spawn_server().await;
    let client = algolia_client();
    let base = format!("http://{}", addr);

    seed_index(
        &base,
        "products",
        vec![json!({
            "objectID": "p1",
            "name": "Widget",
            "description": "A simple widget"
        })],
    )
    .await;

    // Request both highlight and snippet
    let res = h(client.post(format!("{}/1/indexes/products/query", base)))
        .json(&json!({
            "query": "widget",
            "attributesToSnippet": ["description:10"],
            "attributesToHighlight": ["name"]
        }))
        .send()
        .await
        .unwrap();
    assert!(res.status().is_success());

    let body: serde_json::Value = res.json().await.unwrap();
    let hit = &body["hits"][0];

    // Both should be present
    assert!(
        hit["_highlightResult"].is_object(),
        "_highlightResult should exist"
    );
    assert!(
        hit["_snippetResult"].is_object(),
        "_snippetResult should exist"
    );

    let snippet = &hit["_snippetResult"]["description"];
    assert_eq!(
        snippet["matchLevel"].as_str().unwrap(),
        "full",
        "snippet matchLevel should be full"
    );
}

#[tokio::test]
async fn test_snippet_no_match() {
    let (addr, _temp) = spawn_server().await;
    let client = algolia_client();
    let base = format!("http://{}", addr);

    seed_index(
        &base,
        "products",
        vec![json!({
            "objectID": "p1",
            "name": "Widget",
            "description": "The quick brown fox jumps over the lazy dog repeatedly and with great enthusiasm for exercise"
        })],
    )
    .await;

    // Search for something in name, snippet description (which won't match)
    let res = h(client.post(format!("{}/1/indexes/products/query", base)))
        .json(&json!({
            "query": "widget",
            "attributesToSnippet": ["description:5"]
        }))
        .send()
        .await
        .unwrap();
    assert!(res.status().is_success());

    let body: serde_json::Value = res.json().await.unwrap();
    let hits = body["hits"]
        .as_array()
        .expect("response must have hits array");
    assert!(
        !hits.is_empty(),
        "snippet no-match query should still return hits"
    );
    let snippet = &hits[0]["_snippetResult"]["description"];
    assert!(
        snippet.is_object(),
        "_snippetResult.description must be present as object, got: {}",
        snippet
    );
    assert_eq!(
        snippet["matchLevel"].as_str().unwrap(),
        "none",
        "no match in description"
    );
}

// ── queryType Tests ─────────────────────────────────────────────────────

#[tokio::test]
async fn test_query_type_prefix_last() {
    let (addr, _temp) = spawn_server().await;
    let client = algolia_client();
    let base = format!("http://{}", addr);

    seed_index(
        &base,
        "products",
        vec![
            json!({"objectID": "1", "name": "Laptop Pro"}),
            json!({"objectID": "2", "name": "Desktop Ultra"}),
        ],
    )
    .await;

    // Default is prefixLast: "lap" should match "Laptop" (prefix on last word)
    let res = h(client.post(format!("{}/1/indexes/products/query", base)))
        .json(&json!({"query": "lap"}))
        .send()
        .await
        .unwrap();
    let body: serde_json::Value = res.json().await.unwrap();
    assert!(
        body["nbHits"].as_u64().unwrap() > 0,
        "prefixLast: 'lap' should match Laptop"
    );
}

#[tokio::test]
async fn test_query_type_prefix_none() {
    let (addr, _temp) = spawn_server().await;
    let client = algolia_client();
    let base = format!("http://{}", addr);

    seed_index(
        &base,
        "products",
        vec![
            json!({"objectID": "1", "name": "Laptop Pro"}),
            json!({"objectID": "2", "name": "Desktop Ultra"}),
        ],
    )
    .await;

    // First verify default (prefix) behavior: "lap" SHOULD match "Laptop"
    let res_default = h(client.post(format!("{}/1/indexes/products/query", base)))
        .json(&json!({"query": "lap"}))
        .send()
        .await
        .unwrap();
    let default_body: serde_json::Value = res_default.json().await.unwrap();
    let default_hits = default_body["nbHits"].as_u64().unwrap();
    assert!(
        default_hits > 0,
        "default prefix: 'lap' should match 'Laptop'"
    );

    // prefixNone with typoTolerance disabled: "lap" should NOT match "Laptop"
    let res = h(client.post(format!("{}/1/indexes/products/query", base)))
        .json(&json!({"query": "lap", "queryType": "prefixNone", "typoTolerance": false}))
        .send()
        .await
        .unwrap();
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(
        body["nbHits"].as_u64().unwrap(),
        0,
        "prefixNone with typoTolerance=false: 'lap' should NOT match 'Laptop'"
    );
}

#[tokio::test]
async fn test_query_type_prefix_all() {
    let (addr, _temp) = spawn_server().await;
    let client = algolia_client();
    let base = format!("http://{}", addr);

    seed_index(
        &base,
        "products",
        vec![json!({"objectID": "1", "name": "Laptop Pro Max"})],
    )
    .await;

    // prefixAll: both "lap" and "pro" should prefix-match
    let res = h(client.post(format!("{}/1/indexes/products/query", base)))
        .json(&json!({"query": "lap pro", "queryType": "prefixAll"}))
        .send()
        .await
        .unwrap();
    let body: serde_json::Value = res.json().await.unwrap();
    assert!(
        body["nbHits"].as_u64().unwrap() > 0,
        "prefixAll: 'lap pro' should match 'Laptop Pro Max'"
    );
}

// ── typoTolerance Tests ─────────────────────────────────────────────────

#[tokio::test]
async fn test_typo_tolerance_default() {
    let (addr, _temp) = spawn_server().await;
    let client = algolia_client();
    let base = format!("http://{}", addr);

    seed_index(
        &base,
        "products",
        vec![json!({"objectID": "1", "name": "Laptop Computer"})],
    )
    .await;

    // Default: fuzzy matching enabled. "latop" (typo) should find "laptop"
    let res = h(client.post(format!("{}/1/indexes/products/query", base)))
        .json(&json!({"query": "latop"}))
        .send()
        .await
        .unwrap();
    let body: serde_json::Value = res.json().await.unwrap();
    assert!(
        body["nbHits"].as_u64().unwrap() > 0,
        "fuzzy default: 'latop' should match 'laptop'"
    );
}

#[tokio::test]
async fn test_typo_tolerance_false() {
    let (addr, _temp) = spawn_server().await;
    let client = algolia_client();
    let base = format!("http://{}", addr);

    seed_index(
        &base,
        "products",
        vec![json!({"objectID": "1", "name": "Laptop Computer"})],
    )
    .await;

    // typoTolerance=false: "latop" should NOT match
    let res = h(client.post(format!("{}/1/indexes/products/query", base)))
        .json(&json!({"query": "latop", "typoTolerance": false}))
        .send()
        .await
        .unwrap();
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(
        body["nbHits"].as_u64().unwrap(),
        0,
        "typoTolerance=false: 'latop' should NOT match"
    );
}

#[tokio::test]
async fn test_typo_tolerance_true_explicit() {
    let (addr, _temp) = spawn_server().await;
    let client = algolia_client();
    let base = format!("http://{}", addr);

    seed_index(
        &base,
        "products",
        vec![json!({"objectID": "1", "name": "Laptop Computer"})],
    )
    .await;

    // typoTolerance=true: same as default
    let res = h(client.post(format!("{}/1/indexes/products/query", base)))
        .json(&json!({"query": "latop", "typoTolerance": true}))
        .send()
        .await
        .unwrap();
    let body: serde_json::Value = res.json().await.unwrap();
    assert!(
        body["nbHits"].as_u64().unwrap() > 0,
        "typoTolerance=true: 'latop' should match"
    );
}

// ── advancedSyntax Tests ────────────────────────────────────────────────

#[tokio::test]
async fn test_advanced_syntax_exclusion() {
    let (addr, _temp) = spawn_server().await;
    let client = algolia_client();
    let base = format!("http://{}", addr);

    seed_index(
        &base,
        "products",
        vec![
            json!({"objectID": "1", "name": "Laptop Pro"}),
            json!({"objectID": "2", "name": "Desktop Ultra"}),
            json!({"objectID": "3", "name": "Laptop Basic"}),
        ],
    )
    .await;

    // advancedSyntax=true, "-desktop" excludes desktop
    let res = h(client.post(format!("{}/1/indexes/products/query", base)))
        .json(&json!({"query": "laptop -desktop", "advancedSyntax": true}))
        .send()
        .await
        .unwrap();
    assert!(res.status().is_success());
    let body: serde_json::Value = res.json().await.unwrap();
    let hits = body["hits"].as_array().unwrap();
    // Should find laptops but NOT desktop
    for hit in hits {
        let name = hit["name"].as_str().unwrap_or("");
        assert!(
            !name.to_lowercase().contains("desktop"),
            "desktop should be excluded: {}",
            name
        );
    }
}

#[tokio::test]
async fn test_advanced_syntax_exact_phrase() {
    let (addr, _temp) = spawn_server().await;
    let client = algolia_client();
    let base = format!("http://{}", addr);

    seed_index(
        &base,
        "products",
        vec![
            json!({"objectID": "1", "name": "Blue Wireless Earbuds"}),
            json!({"objectID": "2", "name": "Wireless Blue Speaker"}),
        ],
    )
    .await;

    // advancedSyntax=true, "blue wireless" as exact phrase
    let res = h(client.post(format!("{}/1/indexes/products/query", base)))
        .json(&json!({"query": "\"blue wireless\"", "advancedSyntax": true}))
        .send()
        .await
        .unwrap();
    assert!(res.status().is_success());
    let body: serde_json::Value = res.json().await.unwrap();
    // Both items contain "blue" and "wireless", but only item 1 has the exact phrase "blue wireless"
    let hits = body["hits"].as_array().unwrap();
    assert!(
        !hits.is_empty(),
        "exact phrase search for 'blue wireless' must return results"
    );
    let ids: Vec<&str> = hits.iter().filter_map(|h| h["objectID"].as_str()).collect();
    assert!(
        ids.contains(&"1"),
        "item with 'Blue Wireless Earbuds' should match exact phrase"
    );
}

#[tokio::test]
async fn test_advanced_syntax_disabled_default() {
    let (addr, _temp) = spawn_server().await;
    let client = algolia_client();
    let base = format!("http://{}", addr);

    seed_index(
        &base,
        "products",
        vec![
            json!({"objectID": "1", "name": "Laptop Pro"}),
            json!({"objectID": "2", "name": "Desktop Ultra"}),
        ],
    )
    .await;

    // Without advancedSyntax, the dash is not treated as exclusion
    let res = h(client.post(format!("{}/1/indexes/products/query", base)))
        .json(&json!({"query": "laptop -desktop"}))
        .send()
        .await
        .unwrap();
    assert!(
        res.status().is_success(),
        "search with dash in query (no advancedSyntax) should not error, got {}",
        res.status()
    );

    let body: serde_json::Value = res.json().await.unwrap();
    // Verify the response has valid structure
    assert!(
        body.get("hits").is_some(),
        "response must include hits field"
    );
    assert!(
        body.get("nbHits").is_some() || body.get("totalHits").is_some(),
        "response must include a hit count field"
    );
    // Note: whether the dash excludes "Desktop" depends on the search engine's
    // query parser. The key requirement is that the query doesn't error out and
    // returns a valid response with the expected structure.
}

// ── removeWordsIfNoResults Tests ────────────────────────────────────────

#[tokio::test]
async fn test_remove_words_last_words() {
    let (addr, _temp) = spawn_server().await;
    let client = algolia_client();
    let base = format!("http://{}", addr);

    seed_index(
        &base,
        "products",
        vec![
            json!({"objectID": "1", "name": "Laptop"}),
            json!({"objectID": "2", "name": "Phone"}),
        ],
    )
    .await;

    // "laptop xyznonexistent" returns 0 results normally.
    // With lastWords, drops "xyznonexistent", retries "laptop" -> finds it
    let res = h(client.post(format!("{}/1/indexes/products/query", base)))
        .json(&json!({
            "query": "laptop xyznonexistent",
            "removeWordsIfNoResults": "lastWords"
        }))
        .send()
        .await
        .unwrap();
    let body: serde_json::Value = res.json().await.unwrap();
    assert!(
        body["nbHits"].as_u64().unwrap() > 0,
        "lastWords should find results after dropping last word"
    );
}

#[tokio::test]
async fn test_remove_words_first_words() {
    let (addr, _temp) = spawn_server().await;
    let client = algolia_client();
    let base = format!("http://{}", addr);

    seed_index(
        &base,
        "products",
        vec![json!({"objectID": "1", "name": "Laptop"})],
    )
    .await;

    // "xyznonexistent laptop" returns 0. firstWords drops "xyznonexistent", retries "laptop"
    let res = h(client.post(format!("{}/1/indexes/products/query", base)))
        .json(&json!({
            "query": "xyznonexistent laptop",
            "removeWordsIfNoResults": "firstWords"
        }))
        .send()
        .await
        .unwrap();
    let body: serde_json::Value = res.json().await.unwrap();
    assert!(
        body["nbHits"].as_u64().unwrap() > 0,
        "firstWords should find results after dropping first word"
    );
}

#[tokio::test]
async fn test_remove_words_none_default() {
    let (addr, _temp) = spawn_server().await;
    let client = algolia_client();
    let base = format!("http://{}", addr);

    seed_index(
        &base,
        "products",
        vec![json!({"objectID": "1", "name": "Laptop"})],
    )
    .await;

    // Default (none): "laptop xyznonexistent" returns 0 results
    let res = h(client.post(format!("{}/1/indexes/products/query", base)))
        .json(&json!({"query": "laptop xyznonexistent"}))
        .send()
        .await
        .unwrap();
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(
        body["nbHits"].as_u64().unwrap(),
        0,
        "default: no fallback, 0 results"
    );
}

// ── Highlight Custom Tags Test ──────────────────────────────────────────

#[tokio::test]
async fn test_highlight_custom_tags() {
    let (addr, _temp) = spawn_server().await;
    let client = algolia_client();
    let base = format!("http://{}", addr);

    seed_index(
        &base,
        "products",
        vec![json!({"objectID": "1", "name": "Blue Widget"})],
    )
    .await;

    let res = h(client.post(format!("{}/1/indexes/products/query", base)))
        .json(&json!({
            "query": "blue",
            "highlightPreTag": "<b>",
            "highlightPostTag": "</b>"
        }))
        .send()
        .await
        .unwrap();
    let body: serde_json::Value = res.json().await.unwrap();
    let hits = body["hits"].as_array().unwrap();
    assert!(!hits.is_empty());

    let highlight = &hits[0]["_highlightResult"]["name"]["value"];
    let hl_str = highlight.as_str().unwrap();
    assert!(
        hl_str.contains("<b>"),
        "should use custom pre tag: {}",
        hl_str
    );
    assert!(
        hl_str.contains("</b>"),
        "should use custom post tag: {}",
        hl_str
    );
    assert!(
        !hl_str.contains("<em>"),
        "should NOT use default em tag: {}",
        hl_str
    );
}

// ── Browse Cursor Pagination Test ───────────────────────────────────────

#[tokio::test]
async fn test_browse_pagination_cursor() {
    let (addr, _temp) = spawn_server().await;
    let client = algolia_client();
    let base = format!("http://{}", addr);

    seed_index(
        &base,
        "products",
        vec![
            json!({"objectID": "a1", "name": "Alpha"}),
            json!({"objectID": "b2", "name": "Bravo"}),
            json!({"objectID": "c3", "name": "Charlie"}),
        ],
    )
    .await;

    // Browse page 1 with hitsPerPage=2
    let res = h(client.post(format!("{}/1/indexes/products/browse", base)))
        .json(&json!({"hitsPerPage": 2}))
        .send()
        .await
        .unwrap();
    assert!(res.status().is_success());
    let body: serde_json::Value = res.json().await.unwrap();
    let hits = body["hits"].as_array().unwrap();
    assert_eq!(hits.len(), 2, "first page should have 2 hits");

    // Should have a cursor for next page
    let cursor = body["cursor"].as_str();
    assert!(cursor.is_some(), "should have cursor for next page");

    // Browse page 2 with cursor
    let res = h(client.post(format!("{}/1/indexes/products/browse", base)))
        .json(&json!({"cursor": cursor.unwrap()}))
        .send()
        .await
        .unwrap();
    assert!(res.status().is_success());
    let body2: serde_json::Value = res.json().await.unwrap();
    let hits2 = body2["hits"].as_array().unwrap();
    assert!(!hits2.is_empty(), "second page should have at least 1 hit");

    // IDs should be different from page 1
    let page1_ids: Vec<&str> = hits.iter().filter_map(|h| h["objectID"].as_str()).collect();
    let page2_ids: Vec<&str> = hits2
        .iter()
        .filter_map(|h| h["objectID"].as_str())
        .collect();
    for id in &page2_ids {
        assert!(
            !page1_ids.contains(id),
            "page 2 should have different IDs than page 1"
        );
    }
}

// ── optionalFilters ──────────────────────────────────────────────────

#[tokio::test]
async fn test_optional_filters_boost() {
    let (addr, _temp) = spawn_server().await;
    let client = algolia_client();
    let base = format!("http://{}", addr);

    // Configure faceting so category is in _json_filter
    h(client.put(format!("{}/1/indexes/products/settings", base)))
        .json(&json!({"attributesForFaceting": ["category"]}))
        .send()
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    seed_index(
        &base,
        "products",
        vec![
            json!({"objectID": "1", "name": "laptop computer", "category": "Electronics"}),
            json!({"objectID": "2", "name": "laptop bag", "category": "Accessories"}),
            json!({"objectID": "3", "name": "laptop stand", "category": "Electronics"}),
        ],
    )
    .await;

    // Search with optionalFilters boosting Electronics
    let res = h(client.post(format!("{}/1/indexes/products/query", base)))
        .json(&json!({
            "query": "laptop",
            "optionalFilters": ["category:Electronics"]
        }))
        .send()
        .await
        .unwrap();
    assert!(res.status().is_success());
    let body: serde_json::Value = res.json().await.unwrap();
    let hits = body["hits"].as_array().unwrap();

    // All 3 results should be present (optionalFilters don't exclude)
    assert_eq!(hits.len(), 3, "optionalFilters should not exclude results");

    // Verify all expected IDs are present regardless of order
    let ids: Vec<&str> = hits.iter().filter_map(|h| h["objectID"].as_str()).collect();
    assert!(ids.contains(&"1"), "Electronics item 1 should be present");
    assert!(ids.contains(&"2"), "Accessories item should be present");
    assert!(ids.contains(&"3"), "Electronics item 3 should be present");

    // Verify Electronics items are ranked higher than Accessories
    let pos = |id: &str| -> usize {
        ids.iter()
            .position(|&x| x == id)
            .expect("id must be present")
    };
    let acc_pos = pos("2"); // Accessories item
    assert!(
        pos("1") < acc_pos || pos("3") < acc_pos,
        "At least one Electronics item should rank above Accessories when boosted, got order: {:?}",
        ids
    );
}

#[tokio::test]
async fn test_optional_filters_no_exclusion() {
    let (addr, _temp) = spawn_server().await;
    let client = algolia_client();
    let base = format!("http://{}", addr);

    h(client.put(format!("{}/1/indexes/items/settings", base)))
        .json(&json!({"attributesForFaceting": ["brand"]}))
        .send()
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    seed_index(
        &base,
        "items",
        vec![
            json!({"objectID": "a", "name": "widget alpha", "brand": "Acme"}),
            json!({"objectID": "b", "name": "widget beta", "brand": "Globex"}),
        ],
    )
    .await;

    // Search without optionalFilters
    let res_plain = h(client.post(format!("{}/1/indexes/items/query", base)))
        .json(&json!({"query": "widget"}))
        .send()
        .await
        .unwrap();
    let plain: serde_json::Value = res_plain.json().await.unwrap();
    let plain_count = plain["nbHits"].as_u64().unwrap();

    // Search with optionalFilters for a non-matching brand
    let res_opt = h(client.post(format!("{}/1/indexes/items/query", base)))
        .json(&json!({
            "query": "widget",
            "optionalFilters": ["brand:NonExistent"]
        }))
        .send()
        .await
        .unwrap();
    let opt: serde_json::Value = res_opt.json().await.unwrap();
    let opt_count = opt["nbHits"].as_u64().unwrap();

    // Same number of results — optionalFilters never exclude
    assert_eq!(
        plain_count, opt_count,
        "optionalFilters should not change result count"
    );
}

#[tokio::test]
async fn test_optional_filters_score_weight() {
    let (addr, _temp) = spawn_server().await;
    let client = algolia_client();
    let base = format!("http://{}", addr);

    h(client.put(format!("{}/1/indexes/scored/settings", base)))
        .json(&json!({"attributesForFaceting": ["color"]}))
        .send()
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    seed_index(
        &base,
        "scored",
        vec![
            json!({"objectID": "r1", "name": "paint red bright", "color": "red"}),
            json!({"objectID": "b1", "name": "paint blue bright", "color": "blue"}),
        ],
    )
    .await;

    // Boost blue with high score
    let res = h(client.post(format!("{}/1/indexes/scored/query", base)))
        .json(&json!({
            "query": "paint",
            "optionalFilters": ["color:blue<score=5>"]
        }))
        .send()
        .await
        .unwrap();
    assert!(res.status().is_success());
    let body: serde_json::Value = res.json().await.unwrap();
    let hits = body["hits"].as_array().unwrap();
    assert_eq!(hits.len(), 2);
    // Blue should be first due to high boost
    assert_eq!(hits[0]["objectID"].as_str().unwrap(), "b1");
}

// ── enableSynonyms toggle ────────────────────────────────────────────

#[tokio::test]
async fn test_enable_synonyms_false() {
    let (addr, _temp) = spawn_server().await;
    let client = algolia_client();
    let base = format!("http://{}", addr);

    // Seed documents first
    seed_index(
        &base,
        "syn_test",
        vec![
            json!({"objectID": "1", "name": "mobile device"}),
            json!({"objectID": "2", "name": "phone case"}),
        ],
    )
    .await;

    // Add a synonym: phone <-> mobile (after documents so index exists)
    let syn_res = h(client.put(format!("{}/1/indexes/syn_test/synonyms/syn1", base)))
        .json(&json!({
            "objectID": "syn1",
            "type": "synonym",
            "synonyms": ["phone", "mobile"]
        }))
        .send()
        .await
        .unwrap();
    assert!(
        syn_res.status().is_success(),
        "saving synonym should succeed, got {}",
        syn_res.status()
    );
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    // Search "phone" WITH synonyms (default) — should find "phone case" at minimum
    let res = h(client.post(format!("{}/1/indexes/syn_test/query", base)))
        .json(&json!({"query": "phone"}))
        .send()
        .await
        .unwrap();
    assert!(res.status().is_success());
    let with_syn: serde_json::Value = res.json().await.unwrap();
    let with_count = with_syn["nbHits"].as_u64().unwrap();

    // Search "phone" WITHOUT synonyms
    let res = h(client.post(format!("{}/1/indexes/syn_test/query", base)))
        .json(&json!({"query": "phone", "enableSynonyms": false}))
        .send()
        .await
        .unwrap();
    assert!(res.status().is_success());
    let without_syn: serde_json::Value = res.json().await.unwrap();
    let without_count = without_syn["nbHits"].as_u64().unwrap();

    // Without synonyms, "phone" should match at least "phone case"
    assert!(
        without_count >= 1,
        "synonyms disabled: 'phone' should match 'phone case', got {} hits",
        without_count
    );

    // With synonyms enabled, should return >= without synonyms
    // (synonym expansion may additionally match "mobile device")
    assert!(
        with_count >= without_count,
        "synonyms enabled ({}) should return >= synonyms disabled ({})",
        with_count,
        without_count
    );

    // Verify enableSynonyms parameter is accepted and processed
    // (not just ignored — the response structure should be valid)
    assert!(
        without_syn.get("hits").is_some(),
        "response with enableSynonyms:false must have valid hits"
    );
}

// ── enableRules toggle ───────────────────────────────────────────────

#[tokio::test]
async fn test_enable_rules_false() {
    let (addr, _temp) = spawn_server().await;
    let client = algolia_client();
    let base = format!("http://{}", addr);

    // Add a rule that pins objectID "promo" to position 0
    h(client.put(format!("{}/1/indexes/rule_test/rules/rule1", base)))
        .json(&json!({
            "objectID": "rule1",
            "conditions": [{"anchoring": "is", "pattern": "laptop"}],
            "consequence": {
                "promote": [{"objectID": "promo", "position": 0}]
            }
        }))
        .send()
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    seed_index(
        &base,
        "rule_test",
        vec![
            json!({"objectID": "promo", "name": "promoted laptop"}),
            json!({"objectID": "regular", "name": "regular laptop"}),
        ],
    )
    .await;

    // Search with rules (default) — promo should be pinned at position 0
    let res = h(client.post(format!("{}/1/indexes/rule_test/query", base)))
        .json(&json!({"query": "laptop"}))
        .send()
        .await
        .unwrap();
    let with_rules: serde_json::Value = res.json().await.unwrap();
    let hits = with_rules["hits"]
        .as_array()
        .expect("response must have hits array");
    assert!(
        hits.len() >= 2,
        "rule_test should have at least 2 hits, got {}",
        hits.len()
    );
    // The rule pins "promo" to position 0
    assert_eq!(
        hits[0]["objectID"].as_str().unwrap(),
        "promo",
        "with rules enabled, 'promo' should be pinned at position 0"
    );

    // Search with rules disabled — promo should NOT necessarily be first
    let res = h(client.post(format!("{}/1/indexes/rule_test/query", base)))
        .json(&json!({"query": "laptop", "enableRules": false}))
        .send()
        .await
        .unwrap();
    assert!(res.status().is_success());
    let without_rules: serde_json::Value = res.json().await.unwrap();
    let hits_no_rules = without_rules["hits"]
        .as_array()
        .expect("response must have hits array");
    // Should still return results (rules don't affect what's found, just ordering/promotion)
    assert!(
        !hits_no_rules.is_empty(),
        "with rules disabled, search should still return results"
    );
}

// ── restrictSearchableAttributes ─────────────────────────────────────

#[tokio::test]
async fn test_restrict_searchable_attributes() {
    let (addr, _temp) = spawn_server().await;
    let client = algolia_client();
    let base = format!("http://{}", addr);

    seed_index(
        &base,
        "restrict_test",
        vec![
            json!({"objectID": "1", "title": "rust programming", "description": "learn rust language"}),
            json!({"objectID": "2", "title": "python guide", "description": "rust comparison"}),
        ],
    )
    .await;

    // Search "rust" across all fields — should find both
    let res = h(client.post(format!("{}/1/indexes/restrict_test/query", base)))
        .json(&json!({"query": "rust"}))
        .send()
        .await
        .unwrap();
    let all: serde_json::Value = res.json().await.unwrap();
    let all_count = all["nbHits"].as_u64().unwrap();
    assert_eq!(all_count, 2, "both docs match 'rust' somewhere");

    // Search "rust" restricted to title only — should find only doc 1
    let res = h(client.post(format!("{}/1/indexes/restrict_test/query", base)))
        .json(&json!({
            "query": "rust",
            "restrictSearchableAttributes": ["title"]
        }))
        .send()
        .await
        .unwrap();
    let restricted: serde_json::Value = res.json().await.unwrap();
    let restricted_count = restricted["nbHits"].as_u64().unwrap();
    assert!(
        restricted_count < all_count,
        "restricting to title should find fewer results: {} < {}",
        restricted_count,
        all_count
    );
}

// ── ruleContexts ─────────────────────────────────────────────────────

#[tokio::test]
async fn test_rule_contexts() {
    let (addr, _temp) = spawn_server().await;
    let client = algolia_client();
    let base = format!("http://{}", addr);

    seed_index(
        &base,
        "ctx_test",
        vec![
            json!({"objectID": "1", "name": "test item"}),
            json!({"objectID": "2", "name": "other item"}),
        ],
    )
    .await;

    // Search WITHOUT ruleContexts as baseline
    let res_baseline = h(client.post(format!("{}/1/indexes/ctx_test/query", base)))
        .json(&json!({"query": "test"}))
        .send()
        .await
        .unwrap();
    assert!(res_baseline.status().is_success());
    let baseline: serde_json::Value = res_baseline.json().await.unwrap();
    let baseline_hits = baseline["nbHits"].as_u64().unwrap();

    // Search WITH ruleContexts — should be accepted and return valid results
    let res = h(client.post(format!("{}/1/indexes/ctx_test/query", base)))
        .json(&json!({
            "query": "test",
            "ruleContexts": ["mobile", "homepage"]
        }))
        .send()
        .await
        .unwrap();
    assert!(
        res.status().is_success(),
        "ruleContexts param should be accepted"
    );
    let body: serde_json::Value = res.json().await.unwrap();
    let ctx_hits = body["nbHits"].as_u64().unwrap();
    assert!(ctx_hits > 0, "should return results with ruleContexts");

    // Without context-specific rules, results should match baseline
    assert_eq!(
        ctx_hits, baseline_hits,
        "without context-specific rules, results should match baseline"
    );

    // Verify response structure is valid search response
    assert!(body.get("hits").is_some(), "response must contain hits");
    assert!(
        body.get("processingTimeMS").is_some(),
        "response must contain processingTimeMS"
    );
}
