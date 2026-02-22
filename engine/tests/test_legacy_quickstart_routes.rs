use serde_json::json;

mod common;

fn with_auth(req: reqwest::RequestBuilder, api_key: &str) -> reqwest::RequestBuilder {
    req.header("x-algolia-application-id", "test")
        .header("x-algolia-api-key", api_key)
}

async fn assert_404(resp: reqwest::Response, context: &str) {
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    assert_eq!(
        status.as_u16(),
        404,
        "{} should return 404, got {} body={}",
        context,
        status,
        body
    );
}

#[tokio::test]
async fn legacy_quickstart_routes_are_removed_in_open_mode() {
    let (addr, _temp) = common::spawn_server().await;
    let client = reqwest::Client::new();
    let base = format!("http://{}", addr);

    assert_404(
        client
            .get(format!("{}/indexes", base))
            .send()
            .await
            .unwrap(),
        "GET /indexes",
    )
    .await;

    assert_404(
        client
            .get(format!("{}/indexes/products/search?q=laptop", base))
            .send()
            .await
            .unwrap(),
        "GET /indexes/:index/search",
    )
    .await;

    assert_404(
        client
            .post(format!("{}/indexes/products/documents", base))
            .json(&json!([{"objectID": "1", "name": "Laptop"}]))
            .send()
            .await
            .unwrap(),
        "POST /indexes/:index/documents",
    )
    .await;

    assert_404(
        client
            .get(format!("{}/tasks/1", base))
            .send()
            .await
            .unwrap(),
        "GET /tasks/:id",
    )
    .await;

    assert_404(
        client
            .post(format!("{}/migrate", base))
            .json(&json!({
                "appId": "any",
                "apiKey": "any",
                "sourceIndex": "any"
            }))
            .send()
            .await
            .unwrap(),
        "POST /migrate",
    )
    .await;
}

#[tokio::test]
async fn legacy_quickstart_routes_are_removed_even_with_valid_auth() {
    let (addr, _temp) = common::spawn_server_with_key(Some("admin_secret_123")).await;
    let client = reqwest::Client::new();
    let base = format!("http://{}", addr);
    let api_key = "admin_secret_123";

    assert_404(
        with_auth(client.get(format!("{}/indexes", base)), api_key)
            .send()
            .await
            .unwrap(),
        "GET /indexes with valid auth",
    )
    .await;

    assert_404(
        with_auth(
            client.post(format!("{}/indexes/products/documents", base)),
            api_key,
        )
        .json(&json!([{"objectID": "1", "name": "Laptop"}]))
        .send()
        .await
        .unwrap(),
        "POST /indexes/:index/documents with valid auth",
    )
    .await;

    assert_404(
        with_auth(client.post(format!("{}/migrate", base)), api_key)
            .json(&json!({
                "appId": "any",
                "apiKey": "any",
                "sourceIndex": "any"
            }))
            .send()
            .await
            .unwrap(),
        "POST /migrate with valid auth",
    )
    .await;
}

#[tokio::test]
async fn algolia_prefixed_routes_still_work_in_open_mode() {
    let (addr, _temp) = common::spawn_server().await;
    let client = reqwest::Client::new();
    let base = format!("http://{}", addr);

    let create_resp = client
        .post(format!("{}/1/indexes", base))
        .json(&json!({ "uid": "products" }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        create_resp.status(),
        200,
        "POST /1/indexes should work in open mode"
    );

    let batch_resp = client
        .post(format!("{}/1/indexes/products/batch", base))
        .json(&json!({
            "requests": [
                {"action": "addObject", "body": {"objectID": "1", "title": "Laptop Pro"}}
            ]
        }))
        .send()
        .await
        .unwrap();
    common::wait_for_response_task(&client, &addr, batch_resp).await;

    let query_resp = client
        .post(format!("{}/1/indexes/products/query", base))
        .json(&json!({ "query": "laptop" }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        query_resp.status(),
        200,
        "POST /1/indexes/:index/query failed"
    );
    let body: serde_json::Value = query_resp.json().await.unwrap();
    assert_eq!(
        body["nbHits"].as_u64().unwrap_or(0),
        1,
        "Expected one query hit from /1 endpoint"
    );
}
