use serde_json::json;

mod common;
use common::spawn_server;

#[tokio::test]
async fn test_settings_endpoint_lifecycle() {
    let (addr, _temp) = spawn_server().await;
    let client = reqwest::Client::new();
    let base_url = format!("http://{}", addr);

    let response = client
        .post(format!("{}/1/indexes/products/settings", base_url))
        .header("x-algolia-application-id", "test-app")
        .header("x-algolia-api-key", "test-key")
        .json(&json!({
            "attributesForFaceting": ["category", "brand"],
            "ranking": ["typo", "geo"],
            "unsupportedParam": "ignored"
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 207);

    let json: serde_json::Value = response.json().await.unwrap();

    assert!(json["taskID"].is_number());
    assert!(json["updatedAt"].is_string());
    let unsupported = json["unsupportedParams"].as_array().unwrap();
    assert!(unsupported.contains(&json!("ranking")));
    assert!(unsupported.contains(&json!("unsupportedParam")));

    let response = client
        .get(format!("{}/1/indexes/products/settings", base_url))
        .header("x-algolia-application-id", "test-app")
        .header("x-algolia-api-key", "test-key")
        .send()
        .await
        .unwrap();

    assert!(response.status().is_success());

    let settings: serde_json::Value = response.json().await.unwrap();

    assert_eq!(
        settings["attributesForFaceting"].as_array().unwrap().len(),
        2
    );
    assert!(settings["attributesForFaceting"]
        .as_array()
        .unwrap()
        .contains(&json!("category")));
}

#[tokio::test]
async fn test_facet_filter_enforcement() {
    let (addr, _temp) = spawn_server().await;
    let client = reqwest::Client::new();
    let base_url = format!("http://{}", addr);

    client
        .post(format!("{}/1/indexes/books/batch", base_url))
        .header("x-algolia-application-id", "test-app")
        .header("x-algolia-api-key", "test-key")
        .json(&json!({
            "requests": [
                {"action": "addObject", "body": {"objectID": "1", "genre": "scifi", "title": "Dune"}}
            ]
        }))
        .send()
        .await
        .unwrap();

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let response = client
        .post(format!("{}/1/indexes/books/query", base_url))
        .header("x-algolia-application-id", "test-app")
        .header("x-algolia-api-key", "test-key")
        .json(&json!({"query": "", "filters": "genre:scifi"}))
        .send()
        .await
        .unwrap();

    let result: serde_json::Value = response.json().await.unwrap();

    assert_eq!(
        result["nbHits"], 0,
        "Should return 0 without attributesForFaceting"
    );

    client
        .post(format!("{}/1/indexes/books/settings", base_url))
        .header("x-algolia-application-id", "test-app")
        .header("x-algolia-api-key", "test-key")
        .json(&json!({"attributesForFaceting": ["genre"]}))
        .send()
        .await
        .unwrap();

    let response = client
        .post(format!("{}/1/indexes/books/query", base_url))
        .header("x-algolia-application-id", "test-app")
        .header("x-algolia-api-key", "test-key")
        .json(&json!({"query": "", "filters": "genre:scifi"}))
        .send()
        .await
        .unwrap();

    let result: serde_json::Value = response.json().await.unwrap();

    assert_eq!(result["nbHits"], 1, "Should return 1 after declaring genre");
}
