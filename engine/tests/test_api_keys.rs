use serde_json::json;
use std::time::Duration;

mod common;

const ADMIN_KEY: &str = "test-admin-key-abc123";

async fn setup() -> (String, tempfile::TempDir, String) {
    let (addr, temp) = common::spawn_server_with_key(Some(ADMIN_KEY)).await;
    let client = reqwest::Client::new();
    let resp = client
        .get(format!("http://{}/1/keys", addr))
        .header("x-algolia-api-key", ADMIN_KEY)
        .header("x-algolia-application-id", "test")
        .send()
        .await
        .unwrap();
    let body: serde_json::Value = resp.json().await.unwrap();
    let search_key = body["keys"]
        .as_array()
        .unwrap()
        .iter()
        .find(|k| k["description"] == "Default Search API Key")
        .unwrap()["value"]
        .as_str()
        .unwrap()
        .to_string();
    (addr, temp, search_key)
}

fn authed(client: &reqwest::Client, method: &str, url: &str, key: &str) -> reqwest::RequestBuilder {
    let builder = match method {
        "GET" => client.get(url),
        "POST" => client.post(url),
        "PUT" => client.put(url),
        "DELETE" => client.delete(url),
        _ => panic!("unknown method"),
    };
    builder
        .header("x-algolia-api-key", key)
        .header("x-algolia-application-id", "test")
}

async fn create_index(client: &reqwest::Client, addr: &str, index: &str, key: &str) {
    authed(client, "POST", &format!("http://{}/1/indexes/{}/batch", addr, index), key)
        .json(&json!({"requests": [{"action": "addObject", "body": {"objectID": "1", "name": "Test Doc"}}]}))
        .send().await.unwrap();
    tokio::time::sleep(Duration::from_millis(200)).await;
}

#[tokio::test]
async fn test_no_key_rejected() {
    let (addr, _temp, _) = setup().await;
    let client = reqwest::Client::new();

    let resp = client
        .post(format!("http://{}/1/indexes/test/query", addr))
        .header("x-algolia-application-id", "test")
        .header("content-type", "application/json")
        .body(r#"{"query":"test"}"#)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 403);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(
        body["message"].as_str().unwrap().contains("Invalid"),
        "should return auth error message"
    );
}

#[tokio::test]
async fn test_no_app_id_rejected() {
    let (addr, _temp, _) = setup().await;
    let client = reqwest::Client::new();

    let resp = client
        .post(format!("http://{}/1/indexes/test/query", addr))
        .header("x-algolia-api-key", ADMIN_KEY)
        .header("content-type", "application/json")
        .body(r#"{"query":"test"}"#)
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        403,
        "missing application-id should be rejected"
    );
}

#[tokio::test]
async fn test_wrong_key_rejected() {
    let (addr, _temp, _) = setup().await;
    let client = reqwest::Client::new();

    let resp = authed(
        &client,
        "POST",
        &format!("http://{}/1/indexes/test/query", addr),
        "wrong-key",
    )
    .header("content-type", "application/json")
    .body(r#"{"query":"test"}"#)
    .send()
    .await
    .unwrap();
    assert_eq!(resp.status(), 403);
}

#[tokio::test]
async fn test_admin_key_full_access() {
    let (addr, _temp, _) = setup().await;
    let client = reqwest::Client::new();

    let resp = authed(
        &client,
        "POST",
        &format!("http://{}/1/indexes/products/batch", addr),
        ADMIN_KEY,
    )
    .json(
        &json!({"requests": [{"action": "addObject", "body": {"objectID": "1", "name": "Test"}}]}),
    )
    .send()
    .await
    .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body.get("taskID").is_some(), "batch should return taskID");

    let resp = authed(
        &client,
        "GET",
        &format!("http://{}/1/keys", addr),
        ADMIN_KEY,
    )
    .send()
    .await
    .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(
        body["keys"].as_array().unwrap().len() >= 2,
        "should list keys"
    );
}

#[tokio::test]
async fn test_search_key_acl_enforced() {
    let (addr, _temp, search_key) = setup().await;
    let client = reqwest::Client::new();

    create_index(&client, &addr, "test", ADMIN_KEY).await;

    let resp = authed(
        &client,
        "POST",
        &format!("http://{}/1/indexes/test/query", addr),
        &search_key,
    )
    .json(&json!({"query": "test"}))
    .send()
    .await
    .unwrap();
    assert_eq!(resp.status(), 200, "search key should allow search");
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(
        body.get("hits").is_some(),
        "search should return hits array"
    );

    let resp = authed(
        &client,
        "POST",
        &format!("http://{}/1/indexes/test/batch", addr),
        &search_key,
    )
    .json(&json!({"requests": [{"action": "addObject", "body": {"objectID": "2"}}]}))
    .send()
    .await
    .unwrap();
    assert_eq!(resp.status(), 403, "search key should block addObject");

    let resp = authed(
        &client,
        "DELETE",
        &format!("http://{}/1/indexes/test", addr),
        &search_key,
    )
    .send()
    .await
    .unwrap();
    assert_eq!(resp.status(), 403, "search key should block deleteIndex");

    let resp = authed(
        &client,
        "POST",
        &format!("http://{}/1/indexes/test/settings", addr),
        &search_key,
    )
    .json(&json!({"searchableAttributes": ["name"]}))
    .send()
    .await
    .unwrap();
    assert_eq!(resp.status(), 403, "search key should block editSettings");

    let resp = authed(
        &client,
        "GET",
        &format!("http://{}/1/indexes/test/settings", addr),
        &search_key,
    )
    .send()
    .await
    .unwrap();
    assert_eq!(resp.status(), 403, "search key should block settings read");

    let resp = authed(
        &client,
        "GET",
        &format!("http://{}/1/keys", addr),
        &search_key,
    )
    .send()
    .await
    .unwrap();
    assert_eq!(resp.status(), 403, "search key should block key management");
}

#[tokio::test]
async fn test_create_and_use_scoped_key() {
    let (addr, _temp, _) = setup().await;
    let client = reqwest::Client::new();

    create_index(&client, &addr, "products", ADMIN_KEY).await;
    create_index(&client, &addr, "secret", ADMIN_KEY).await;

    let resp = authed(&client, "POST", &format!("http://{}/1/keys", addr), ADMIN_KEY)
        .json(&json!({"acl": ["search", "addObject"], "indexes": ["products"], "description": "Scoped key"}))
        .send().await.unwrap();
    assert_eq!(resp.status(), 201);
    let body: serde_json::Value = resp.json().await.unwrap();
    let scoped_key = body["key"].as_str().unwrap().to_string();
    assert!(!scoped_key.is_empty(), "created key should have a value");

    let resp = authed(
        &client,
        "POST",
        &format!("http://{}/1/indexes/products/query", addr),
        &scoped_key,
    )
    .json(&json!({"query": "test"}))
    .send()
    .await
    .unwrap();
    assert_eq!(resp.status(), 200, "scoped key should search allowed index");
    assert!(resp
        .json::<serde_json::Value>()
        .await
        .unwrap()
        .get("hits")
        .is_some());

    let resp = authed(
        &client,
        "POST",
        &format!("http://{}/1/indexes/secret/query", addr),
        &scoped_key,
    )
    .json(&json!({"query": "test"}))
    .send()
    .await
    .unwrap();
    assert_eq!(
        resp.status(),
        403,
        "scoped key should be blocked from other index"
    );

    let resp = authed(&client, "POST", &format!("http://{}/1/indexes/products/batch", addr), &scoped_key)
        .json(&json!({"requests": [{"action": "addObject", "body": {"objectID": "2", "name": "Scoped Write"}}]}))
        .send().await.unwrap();
    assert_eq!(
        resp.status(),
        200,
        "scoped key should write to allowed index"
    );
    assert!(resp
        .json::<serde_json::Value>()
        .await
        .unwrap()
        .get("taskID")
        .is_some());

    let resp = authed(
        &client,
        "DELETE",
        &format!("http://{}/1/indexes/products", addr),
        &scoped_key,
    )
    .send()
    .await
    .unwrap();
    assert_eq!(
        resp.status(),
        403,
        "scoped key without deleteIndex ACL should not delete"
    );
}

#[tokio::test]
async fn test_wildcard_index_patterns() {
    let (addr, _temp, _) = setup().await;
    let client = reqwest::Client::new();

    create_index(&client, &addr, "dev_products", ADMIN_KEY).await;
    create_index(&client, &addr, "dev_users", ADMIN_KEY).await;
    create_index(&client, &addr, "prod_products", ADMIN_KEY).await;

    let resp = authed(
        &client,
        "POST",
        &format!("http://{}/1/keys", addr),
        ADMIN_KEY,
    )
    .json(&json!({"acl": ["search"], "indexes": ["dev_*"], "description": "Dev wildcard"}))
    .send()
    .await
    .unwrap();
    assert_eq!(resp.status(), 201);
    let dev_key = resp.json::<serde_json::Value>().await.unwrap()["key"]
        .as_str()
        .unwrap()
        .to_string();

    let resp = authed(
        &client,
        "POST",
        &format!("http://{}/1/indexes/dev_products/query", addr),
        &dev_key,
    )
    .json(&json!({"query": "test"}))
    .send()
    .await
    .unwrap();
    assert_eq!(resp.status(), 200, "dev_* key should access dev_products");

    let resp = authed(
        &client,
        "POST",
        &format!("http://{}/1/indexes/dev_users/query", addr),
        &dev_key,
    )
    .json(&json!({"query": "test"}))
    .send()
    .await
    .unwrap();
    assert_eq!(resp.status(), 200, "dev_* key should access dev_users");

    let resp = authed(
        &client,
        "POST",
        &format!("http://{}/1/indexes/prod_products/query", addr),
        &dev_key,
    )
    .json(&json!({"query": "test"}))
    .send()
    .await
    .unwrap();
    assert_eq!(
        resp.status(),
        403,
        "dev_* key should NOT access prod_products"
    );

    let resp = authed(
        &client,
        "POST",
        &format!("http://{}/1/keys", addr),
        ADMIN_KEY,
    )
    .json(
        &json!({"acl": ["search"], "indexes": ["*products*"], "description": "Contains wildcard"}),
    )
    .send()
    .await
    .unwrap();
    let contains_key = resp.json::<serde_json::Value>().await.unwrap()["key"]
        .as_str()
        .unwrap()
        .to_string();

    let resp = authed(
        &client,
        "POST",
        &format!("http://{}/1/indexes/dev_products/query", addr),
        &contains_key,
    )
    .json(&json!({"query": "test"}))
    .send()
    .await
    .unwrap();
    assert_eq!(resp.status(), 200, "*products* should match dev_products");

    let resp = authed(
        &client,
        "POST",
        &format!("http://{}/1/indexes/prod_products/query", addr),
        &contains_key,
    )
    .json(&json!({"query": "test"}))
    .send()
    .await
    .unwrap();
    assert_eq!(resp.status(), 200, "*products* should match prod_products");

    let resp = authed(
        &client,
        "POST",
        &format!("http://{}/1/indexes/dev_users/query", addr),
        &contains_key,
    )
    .json(&json!({"query": "test"}))
    .send()
    .await
    .unwrap();
    assert_eq!(resp.status(), 403, "*products* should NOT match dev_users");
}

#[tokio::test]
async fn test_key_ttl_expiry() {
    let (addr, _temp, _) = setup().await;
    let client = reqwest::Client::new();

    create_index(&client, &addr, "test", ADMIN_KEY).await;

    let resp = authed(
        &client,
        "POST",
        &format!("http://{}/1/keys", addr),
        ADMIN_KEY,
    )
    .json(&json!({"acl": ["search"], "validity": 1, "description": "Expires in 1 second"}))
    .send()
    .await
    .unwrap();
    assert_eq!(resp.status(), 201);
    let expiring_key = resp.json::<serde_json::Value>().await.unwrap()["key"]
        .as_str()
        .unwrap()
        .to_string();

    let resp = authed(
        &client,
        "POST",
        &format!("http://{}/1/indexes/test/query", addr),
        &expiring_key,
    )
    .json(&json!({"query": "test"}))
    .send()
    .await
    .unwrap();
    assert_eq!(resp.status(), 200, "key should work before expiry");

    tokio::time::sleep(Duration::from_millis(1100)).await;

    let resp = authed(
        &client,
        "POST",
        &format!("http://{}/1/indexes/test/query", addr),
        &expiring_key,
    )
    .json(&json!({"query": "test"}))
    .send()
    .await
    .unwrap();
    assert_eq!(resp.status(), 403, "expired key should be rejected");
}

#[tokio::test]
async fn test_key_crud_lifecycle() {
    let (addr, _temp, _) = setup().await;
    let client = reqwest::Client::new();

    create_index(&client, &addr, "test", ADMIN_KEY).await;

    let resp = authed(
        &client,
        "POST",
        &format!("http://{}/1/keys", addr),
        ADMIN_KEY,
    )
    .json(&json!({"acl": ["search"], "description": "Temp key"}))
    .send()
    .await
    .unwrap();
    assert_eq!(resp.status(), 201);
    let body: serde_json::Value = resp.json().await.unwrap();
    let key_val = body["key"].as_str().unwrap().to_string();
    assert!(
        body.get("createdAt").is_some(),
        "create should return createdAt"
    );

    let resp = authed(
        &client,
        "GET",
        &format!("http://{}/1/keys/{}", addr, key_val),
        ADMIN_KEY,
    )
    .send()
    .await
    .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["description"], "Temp key");
    assert_eq!(body["acl"].as_array().unwrap().len(), 1);

    let resp = authed(
        &client,
        "PUT",
        &format!("http://{}/1/keys/{}", addr, key_val),
        ADMIN_KEY,
    )
    .json(&json!({"acl": ["search", "browse"], "description": "Updated key"}))
    .send()
    .await
    .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(
        body.get("updatedAt").is_some(),
        "update should return updatedAt"
    );

    let resp = authed(
        &client,
        "GET",
        &format!("http://{}/1/keys/{}", addr, key_val),
        ADMIN_KEY,
    )
    .send()
    .await
    .unwrap();
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["description"], "Updated key");
    assert_eq!(body["acl"].as_array().unwrap().len(), 2);

    let resp = authed(
        &client,
        "DELETE",
        &format!("http://{}/1/keys/{}", addr, key_val),
        ADMIN_KEY,
    )
    .send()
    .await
    .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(
        body.get("deletedAt").is_some(),
        "delete should return deletedAt"
    );

    let resp = authed(
        &client,
        "POST",
        &format!("http://{}/1/indexes/test/query", addr),
        &key_val,
    )
    .json(&json!({"query": "test"}))
    .send()
    .await
    .unwrap();
    assert_eq!(resp.status(), 403, "deleted key should be rejected");

    let resp = authed(
        &client,
        "POST",
        &format!("http://{}/1/keys/{}/restore", addr, key_val),
        ADMIN_KEY,
    )
    .send()
    .await
    .unwrap();
    assert_eq!(resp.status(), 200);

    let resp = authed(
        &client,
        "POST",
        &format!("http://{}/1/indexes/test/query", addr),
        &key_val,
    )
    .json(&json!({"query": "test"}))
    .send()
    .await
    .unwrap();
    assert_eq!(resp.status(), 200, "restored key should work");
    assert!(resp
        .json::<serde_json::Value>()
        .await
        .unwrap()
        .get("hits")
        .is_some());
}

#[tokio::test]
async fn test_cannot_delete_admin_key() {
    let (addr, _temp, _) = setup().await;
    let client = reqwest::Client::new();

    let resp = authed(
        &client,
        "DELETE",
        &format!("http://{}/1/keys/{}", addr, ADMIN_KEY),
        ADMIN_KEY,
    )
    .send()
    .await
    .unwrap();
    assert_eq!(resp.status(), 403);
}

#[tokio::test]
async fn test_default_keys_created() {
    let (addr, _temp, _) = setup().await;
    let client = reqwest::Client::new();

    let resp = authed(
        &client,
        "GET",
        &format!("http://{}/1/keys", addr),
        ADMIN_KEY,
    )
    .send()
    .await
    .unwrap();
    let body: serde_json::Value = resp.json().await.unwrap();
    let keys = body["keys"].as_array().unwrap();
    assert_eq!(keys.len(), 2);

    let admin = keys
        .iter()
        .find(|k| k["description"] == "Admin API Key")
        .unwrap();
    assert_eq!(admin["value"], ADMIN_KEY);
    assert!(admin["acl"].as_array().unwrap().len() > 10);

    let search = keys
        .iter()
        .find(|k| k["description"] == "Default Search API Key")
        .unwrap();
    assert_eq!(search["acl"].as_array().unwrap().len(), 1);
    assert_eq!(search["acl"][0], "search");
}

#[tokio::test]
async fn test_open_mode_no_admin_key() {
    let (addr, _temp) = common::spawn_server().await;
    let client = reqwest::Client::new();

    create_index(&client, &addr, "test", "literally-anything").await;

    let resp = client
        .post(format!("http://{}/1/indexes/test/query", addr))
        .header("x-algolia-api-key", "literally-anything")
        .header("x-algolia-application-id", "test")
        .json(&json!({"query": "test"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200, "open mode should allow any key");
    assert!(resp
        .json::<serde_json::Value>()
        .await
        .unwrap()
        .get("hits")
        .is_some());
}

#[tokio::test]
async fn test_any_valid_key_can_read_own_key() {
    let (addr, _temp, search_key) = setup().await;
    let client = reqwest::Client::new();

    let resp = authed(
        &client,
        "GET",
        &format!("http://{}/1/keys/{}", addr, search_key),
        &search_key,
    )
    .send()
    .await
    .unwrap();
    assert_eq!(
        resp.status(),
        200,
        "any valid key should read its own key info"
    );
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["value"], search_key);
}

#[tokio::test]
async fn test_non_admin_cannot_read_other_keys() {
    let (addr, _temp, search_key) = setup().await;
    let client = reqwest::Client::new();

    let resp = authed(
        &client,
        "GET",
        &format!("http://{}/1/keys/{}", addr, ADMIN_KEY),
        &search_key,
    )
    .send()
    .await
    .unwrap();
    assert_eq!(resp.status(), 403, "non-admin should not read admin key");

    let resp = authed(
        &client,
        "POST",
        &format!("http://{}/1/keys", addr),
        ADMIN_KEY,
    )
    .json(&json!({"acl": ["search"], "description": "Other key"}))
    .send()
    .await
    .unwrap();
    let other_key = resp.json::<serde_json::Value>().await.unwrap()["key"]
        .as_str()
        .unwrap()
        .to_string();

    let resp = authed(
        &client,
        "GET",
        &format!("http://{}/1/keys/{}", addr, other_key),
        &search_key,
    )
    .send()
    .await
    .unwrap();
    assert_eq!(
        resp.status(),
        403,
        "non-admin should not read other non-admin key"
    );
}

#[tokio::test]
async fn test_query_param_auth() {
    let (addr, _temp, _) = setup().await;
    let client = reqwest::Client::new();

    create_index(&client, &addr, "test", ADMIN_KEY).await;

    let resp = client
        .post(format!(
            "http://{}/1/indexes/test/query?x-algolia-api-key={}&x-algolia-application-id=test",
            addr, ADMIN_KEY
        ))
        .json(&json!({"query": "test"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200, "query param auth should work");
}

#[tokio::test]
async fn test_health_no_auth_required() {
    let (addr, _temp, _) = setup().await;
    let client = reqwest::Client::new();

    let resp = client
        .get(format!("http://{}/health", addr))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        200,
        "health endpoint should not require auth"
    );
}

#[tokio::test]
async fn test_search_key_cannot_escalate_via_settings() {
    let (addr, _temp, search_key) = setup().await;
    let client = reqwest::Client::new();

    create_index(&client, &addr, "test", ADMIN_KEY).await;

    let resp = authed(
        &client,
        "POST",
        &format!("http://{}/1/indexes/test/synonyms/batch", addr),
        &search_key,
    )
    .json(&json!([{"objectID": "syn1", "type": "synonym", "synonyms": ["a", "b"]}]))
    .send()
    .await
    .unwrap();
    assert_eq!(resp.status(), 403, "search key should block synonym writes");

    let resp = authed(
        &client,
        "POST",
        &format!("http://{}/1/indexes/test/rules/batch", addr),
        &search_key,
    )
    .json(&json!([{"objectID": "r1", "consequence": {"params": {"query": "x"}}}]))
    .send()
    .await
    .unwrap();
    assert_eq!(resp.status(), 403, "search key should block rules writes");

    let resp = authed(
        &client,
        "POST",
        &format!("http://{}/1/indexes/test/deleteByQuery", addr),
        &search_key,
    )
    .json(&json!({"params": "filters=name:test"}))
    .send()
    .await
    .unwrap();
    assert_eq!(resp.status(), 403, "search key should block deleteByQuery");

    let resp = authed(
        &client,
        "POST",
        &format!("http://{}/1/indexes/test/clear", addr),
        &search_key,
    )
    .json(&json!({}))
    .send()
    .await
    .unwrap();
    assert_eq!(resp.status(), 403, "search key should block clear index");
}
