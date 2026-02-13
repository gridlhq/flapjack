use serde_json::json;

mod common;

#[tokio::test]
async fn test_algolia_batch_format() {
    let (addr, _temp) = common::spawn_server().await;
    let client = reqwest::Client::new();

    let response = client
        .post(format!("http://{}/1/indexes/products/batch", addr))
        .header("x-algolia-api-key", "test")
        .header("x-algolia-application-id", "test")
        .json(&json!({
            "requests": [
                {"action": "addObject", "body": {"objectID": "1", "name": "Laptop", "price": 1200}},
                {"action": "addObject", "body": {"objectID": "2", "name": "Mouse", "price": 25}}
            ]
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);

    let body: serde_json::Value = response.json().await.unwrap();
    assert!(
        body.get("taskID").is_some(),
        "Response missing taskID: {:?}",
        body
    );
    assert!(
        body.get("objectIDs").is_some(),
        "Response missing objectIDs: {:?}",
        body
    );

    let object_ids = body["objectIDs"].as_array().unwrap();
    assert_eq!(object_ids.len(), 2);
}

#[tokio::test]
async fn test_algolia_query_format() {
    let (addr, _temp) = common::spawn_server().await;
    let client = reqwest::Client::new();

    client
        .post(format!("http://{}/1/indexes/products/batch", addr))
        .header("x-algolia-api-key", "test")
        .header("x-algolia-application-id", "test")
        .json(&json!({
            "requests": [
                {"action": "addObject", "body": {"objectID": "1", "title": "Gaming Laptop"}}
            ]
        }))
        .send()
        .await
        .unwrap();

    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    let response = client
        .post(format!("http://{}/1/indexes/products/query", addr))
        .header("x-algolia-api-key", "test")
        .header("x-algolia-application-id", "test")
        .json(&json!({"query": "laptop", "hitsPerPage": 10, "page": 0}))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);

    let body: serde_json::Value = response.json().await.unwrap();
    assert!(body.get("hits").is_some(), "Missing hits: {:?}", body);
    assert!(body.get("nbHits").is_some(), "Missing nbHits: {:?}", body);
    assert!(
        body.get("processingTimeMS").is_some(),
        "Missing processingTimeMS: {:?}",
        body
    );

    let hits = body["hits"].as_array().unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0]["objectID"], "1");
}

#[tokio::test]
async fn test_open_mode_no_headers_allowed() {
    let (addr, _temp) = common::spawn_server().await;
    let client = reqwest::Client::new();

    let response = client
        .post(format!("http://{}/1/indexes", addr))
        .json(&json!({"uid": "test_open"}))
        .send()
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        200,
        "Open mode (no FLAPJACK_ADMIN_KEY) should allow requests without auth headers"
    );

    let response = client
        .post(format!("http://{}/1/indexes/test_open/batch", addr))
        .json(&json!({"requests": [{"action": "addObject", "body": {"objectID": "1", "name": "test"}}]}))
        .send()
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        200,
        "Open mode should allow requests with partial headers too"
    );
}
#[tokio::test]
async fn test_secured_mode_rejects_missing_headers() {
    let (addr, _temp) = common::spawn_server_with_key(Some("admin_secret_123")).await;
    let client = reqwest::Client::new();

    let response = client
        .post(format!("http://{}/1/indexes", addr))
        .json(&json!({"uid": "products"}))
        .send()
        .await
        .unwrap();
    assert_eq!(
        response.status(),
        403,
        "Secured mode must reject requests without auth headers"
    );

    let response = client
        .post(format!("http://{}/1/indexes/products/batch", addr))
        .header("x-algolia-api-key", "wrong_key")
        .header("x-algolia-application-id", "test")
        .json(&json!({"requests": []}))
        .send()
        .await
        .unwrap();
    assert_eq!(
        response.status(),
        403,
        "Secured mode must reject invalid API key"
    );

    let response = client
        .post(format!("http://{}/1/indexes", addr))
        .header("x-algolia-api-key", "admin_secret_123")
        .header("x-algolia-application-id", "test")
        .json(&json!({"uid": "products"}))
        .send()
        .await
        .unwrap();
    assert_eq!(
        response.status(),
        200,
        "Secured mode must accept valid admin key"
    );
}
