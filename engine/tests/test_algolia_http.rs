//! HTTP API Compatibility Test
//!
//! Validates Flapjack HTTP endpoints match Algolia's actual wire protocol.
//! Tests raw HTTP requests/responses, not SDK behavior.
//!
//! Purpose: Ensure any HTTP client (SDK, curl, fetch) can interact with Flapjack.
//! Success criteria: Same requests that work on Algolia work on Flapjack.

use serde_json::{json, Value};

mod common;

#[tokio::test]
async fn test_batch_upload_format() {
    let (addr, _temp) = common::spawn_server().await;
    let client = reqwest::Client::new();

    let response = client
        .post(format!("http://{}/1/indexes/products/batch", addr))
        .header("x-algolia-api-key", "test")
        .header("x-algolia-application-id", "test")
        .json(&json!({
            "requests": [
                {"action": "addObject", "body": {"objectID": "1", "name": "Laptop"}},
                {"action": "addObject", "body": {"objectID": "2", "name": "Mouse"}}
            ]
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200, "Batch upload should succeed");

    let body: Value = response.json().await.unwrap();
    assert!(body.get("taskID").is_some(), "Missing taskID: {:?}", body);
    assert_eq!(
        body["objectIDs"].as_array().unwrap().len(),
        2,
        "Expected 2 objectIDs"
    );
}

#[tokio::test]
async fn test_search_response_format() {
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

    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    let response = client
        .post(format!("http://{}/1/indexes/products/query", addr))
        .header("x-algolia-api-key", "test")
        .header("x-algolia-application-id", "test")
        .json(&json!({"query": "lap", "hitsPerPage": 10, "page": 0}))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);

    let body: Value = response.json().await.unwrap();

    assert!(body.get("hits").is_some(), "Missing hits: {:?}", body);
    assert!(body.get("nbHits").is_some(), "Missing nbHits: {:?}", body);
    assert!(body.get("page").is_some(), "Missing page: {:?}", body);
    assert!(body.get("nbPages").is_some(), "Missing nbPages: {:?}", body);
    assert!(
        body.get("hitsPerPage").is_some(),
        "Missing hitsPerPage: {:?}",
        body
    );
    assert!(
        body.get("processingTimeMS").is_some(),
        "Missing processingTimeMS: {:?}",
        body
    );

    let hits = body["hits"].as_array().unwrap();
    assert_eq!(hits.len(), 1, "Expected 1 hit for 'lap'");
    assert_eq!(hits[0]["objectID"], "1");
}

#[tokio::test]
async fn test_filter_query_format() {
    let (addr, _temp) = common::spawn_server().await;
    let client = reqwest::Client::new();

    client
        .post(format!("http://{}/1/indexes/products/batch", addr))
        .header("x-algolia-api-key", "test")
        .header("x-algolia-application-id", "test")
        .json(&json!({
            "requests": [
                {"action": "addObject", "body": {"objectID": "1", "price": 1200}},
                {"action": "addObject", "body": {"objectID": "2", "price": 25}}
            ]
        }))
        .send()
        .await
        .unwrap();

    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    let response = client
        .post(format!("http://{}/1/indexes/products/query", addr))
        .header("x-algolia-api-key", "test")
        .header("x-algolia-application-id", "test")
        .json(&json!({"query": "", "filters": "price > 100", "hitsPerPage": 10}))
        .send()
        .await
        .unwrap();

    if response.status() != 200 {
        let error_body = response.text().await.unwrap();
        panic!("Filter query failed (400): {}", error_body);
    }

    let body: Value = response.json().await.unwrap();
    let hits = body["hits"].as_array().unwrap();
    assert_eq!(hits.len(), 1, "Expected 1 hit for price > 100");
    assert_eq!(hits[0]["objectID"], "1");
}

#[tokio::test]
async fn test_multiword_prefix_search() {
    let (addr, _temp) = common::spawn_server().await;
    let client = reqwest::Client::new();

    client
        .post(format!("http://{}/1/indexes/products/batch", addr))
        .header("x-algolia-api-key", "test")
        .header("x-algolia-application-id", "test")
        .json(&json!({
            "requests": [
                {"action": "addObject", "body": {"objectID": "1", "title": "Gaming Laptop"}},
                {"action": "addObject", "body": {"objectID": "2", "title": "Laptop Stand"}},
                {"action": "addObject", "body": {"objectID": "3", "title": "Gaming Mouse"}}
            ]
        }))
        .send()
        .await
        .unwrap();

    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    let response = client
        .post(format!("http://{}/1/indexes/products/query", addr))
        .header("x-algolia-api-key", "test")
        .header("x-algolia-application-id", "test")
        .json(&json!({"query": "gaming lap", "hitsPerPage": 10}))
        .send()
        .await
        .unwrap();

    let body: Value = response.json().await.unwrap();
    let hits = body["hits"].as_array().unwrap();

    assert_eq!(
        hits.len(),
        1,
        "Expected 'gaming lap' to match only 'Gaming Laptop'"
    );
    assert_eq!(hits[0]["objectID"], "1");
}

#[tokio::test]
async fn test_complex_filter_precedence() {
    let (addr, _temp) = common::spawn_server().await;
    let client = reqwest::Client::new();

    client
        .post(format!("http://{}/1/indexes/test/settings", addr))
        .header("x-algolia-api-key", "test")
        .header("x-algolia-application-id", "test")
        .json(&json!({"attributesForFaceting": ["category"]}))
        .send()
        .await
        .unwrap();

    client
        .post(format!("http://{}/1/indexes/test/batch", addr))
        .header("x-algolia-api-key", "test")
        .header("x-algolia-application-id", "test")
        .json(&json!({
            "requests": [
                {"action": "addObject", "body": {"objectID": "1", "category": "A", "stock": 10}},
                {"action": "addObject", "body": {"objectID": "2", "category": "B", "stock": 0}},
                {"action": "addObject", "body": {"objectID": "3", "category": "C", "stock": 5}},
                {"action": "addObject", "body": {"objectID": "4", "category": "A", "stock": 0}}
            ]
        }))
        .send()
        .await
        .unwrap();

    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    let response = client
        .post(format!("http://{}/1/indexes/test/query", addr))
        .header("x-algolia-api-key", "test")
        .header("x-algolia-application-id", "test")
        .json(&json!({
            "query": "",
            "filters": "(category:A OR category:C) AND stock > 0",
            "hitsPerPage": 100
        }))
        .send()
        .await
        .unwrap();

    if response.status() != 200 {
        let error_body = response.text().await.unwrap();
        panic!("Complex filter failed: {}", error_body);
    }

    let body: Value = response.json().await.unwrap();
    let hits = body["hits"].as_array().unwrap_or_else(|| {
        panic!("No hits array in response: {:?}", body);
    });

    let ids: Vec<String> = hits
        .iter()
        .map(|h| h["objectID"].as_str().unwrap().to_string())
        .collect();

    assert_eq!(ids.len(), 2, "Expected 2 hits: (A OR C) AND stock>0");
    assert!(
        ids.contains(&"1".to_string()),
        "Should include ID 1 (A AND stock>0)"
    );
    assert!(
        ids.contains(&"3".to_string()),
        "Should include ID 3 (C AND stock>0)"
    );
}
#[tokio::test]
async fn test_nbhits_not_capped_at_limit() {
    let (addr, _temp) = common::spawn_server().await;
    let client = reqwest::Client::new();

    let mut requests = Vec::new();
    for i in 0..150 {
        requests.push(json!({
            "action": "addObject",
            "body": {"objectID": format!("prod_{}", i), "name": "Samsung Galaxy Phone", "brand": "Samsung"}
        }));
    }

    let response = client
        .post(format!("http://{}/1/indexes/electronics/batch", addr))
        .header("x-algolia-api-key", "test")
        .header("x-algolia-application-id", "test")
        .json(&json!({"requests": requests}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let search_response = client
        .post(format!("http://{}/1/indexes/electronics/query", addr))
        .header("x-algolia-api-key", "test")
        .header("x-algolia-application-id", "test")
        .json(&json!({"query": "samsung", "hitsPerPage": 20}))
        .send()
        .await
        .unwrap();

    let body: Value = search_response.json().await.unwrap();
    let nb_hits = body["nbHits"].as_u64().unwrap();
    let hits_len = body["hits"].as_array().unwrap().len();

    assert!(nb_hits >= 150, "nbHits should be >= 150, got {}", nb_hits);
    assert_eq!(hits_len, 20, "hits array should respect hitsPerPage");
}

#[tokio::test]
async fn test_attributes_to_retrieve_filters_response() {
    let (addr, _temp) = common::spawn_server().await;
    let client = reqwest::Client::new();

    client
        .post(format!("http://{}/1/indexes/items/batch", addr))
        .header("x-algolia-api-key", "test")
        .header("x-algolia-application-id", "test")
        .json(&json!({
            "requests": [
                {"action": "addObject", "body": {
                    "objectID": "1",
                    "name": "Test Product",
                    "description": "A long description here",
                    "price": 99,
                    "category": "electronics",
                    "internal_notes": "secret stuff"
                }}
            ]
        }))
        .send()
        .await
        .unwrap();

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let search_response = client
        .post(format!("http://{}/1/indexes/items/query", addr))
        .header("x-algolia-api-key", "test")
        .header("x-algolia-application-id", "test")
        .json(&json!({
            "query": "test",
            "attributesToRetrieve": ["name", "price"]
        }))
        .send()
        .await
        .unwrap();

    let body: Value = search_response.json().await.unwrap();
    let hit = &body["hits"][0];

    assert!(hit.get("name").is_some(), "name should be present");
    assert!(hit.get("price").is_some(), "price should be present");
    assert!(hit.get("objectID").is_some(), "objectID always present");
    assert!(
        hit.get("description").is_none(),
        "description should be filtered out"
    );
    assert!(
        hit.get("category").is_none(),
        "category should be filtered out"
    );
    assert!(
        hit.get("internal_notes").is_none(),
        "internal_notes should be filtered out"
    );
}

#[tokio::test]
async fn test_attributes_to_retrieve_wildcard() {
    let (addr, _temp) = common::spawn_server().await;
    let client = reqwest::Client::new();

    client
        .post(format!("http://{}/1/indexes/wild/batch", addr))
        .header("x-algolia-api-key", "test")
        .header("x-algolia-application-id", "test")
        .json(&json!({
            "requests": [
                {"action": "addObject", "body": {
                    "objectID": "1",
                    "name": "Widget",
                    "color": "blue",
                    "size": "large"
                }}
            ]
        }))
        .send()
        .await
        .unwrap();

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let search_response = client
        .post(format!("http://{}/1/indexes/wild/query", addr))
        .header("x-algolia-api-key", "test")
        .header("x-algolia-application-id", "test")
        .json(&json!({
            "query": "widget",
            "attributesToRetrieve": ["*"]
        }))
        .send()
        .await
        .unwrap();

    let body: Value = search_response.json().await.unwrap();
    let hit = &body["hits"][0];

    assert!(
        hit.get("name").is_some(),
        "name should be present with wildcard"
    );
    assert!(
        hit.get("color").is_some(),
        "color should be present with wildcard"
    );
    assert!(
        hit.get("size").is_some(),
        "size should be present with wildcard"
    );
}
#[tokio::test]
async fn test_attributes_to_highlight_empty_omits_highlight_result() {
    let (addr, _temp) = common::spawn_server().await;
    let client = reqwest::Client::new();

    client
        .post(format!("http://{}/1/indexes/highlight_test/batch", addr))
        .header("x-algolia-api-key", "test")
        .header("x-algolia-application-id", "test")
        .json(&json!({
            "requests": [
                {"action": "addObject", "body": {"objectID": "1", "name": "Gaming Laptop", "brand": "Dell"}}
            ]
        }))
        .send()
        .await
        .unwrap();

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let response = client
        .post(format!("http://{}/1/indexes/highlight_test/query", addr))
        .header("x-algolia-api-key", "test")
        .header("x-algolia-application-id", "test")
        .json(&json!({"query": "laptop", "attributesToHighlight": []}))
        .send()
        .await
        .unwrap();

    let body: Value = response.json().await.unwrap();
    let hit = &body["hits"][0];

    assert!(
        hit.get("_highlightResult").is_none(),
        "should omit _highlightResult when attributesToHighlight=[]"
    );
    assert!(
        hit.get("objectID").is_some(),
        "should still include objectID"
    );
}

#[tokio::test]
async fn test_minimal_response_size() {
    let (addr, _temp) = common::spawn_server().await;
    let client = reqwest::Client::new();

    client
        .post(format!("http://{}/1/indexes/size_test/batch", addr))
        .header("x-algolia-api-key", "test")
        .header("x-algolia-application-id", "test")
        .json(&json!({
            "requests": (0..10).map(|i| json!({
                "action": "addObject",
                "body": {
                    "objectID": i.to_string(),
                    "name": format!("Product {} with a longer name", i),
                    "description": "This is a detailed description with many words to increase size",
                    "category": "electronics",
                    "price": 99.99,
                    "tags": ["tag1", "tag2", "tag3"]
                }
            })).collect::<Vec<_>>()
        }))
        .send()
        .await
        .unwrap();

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let full_response = client
        .post(format!("http://{}/1/indexes/size_test/query", addr))
        .header("x-algolia-api-key", "test")
        .header("x-algolia-application-id", "test")
        .json(&json!({"query": "product", "hitsPerPage": 5}))
        .send()
        .await
        .unwrap();
    let full_bytes = full_response.bytes().await.unwrap().len();

    let minimal_response = client
        .post(format!("http://{}/1/indexes/size_test/query", addr))
        .header("x-algolia-api-key", "test")
        .header("x-algolia-application-id", "test")
        .json(&json!({
            "query": "product",
            "hitsPerPage": 5,
            "attributesToRetrieve": ["name"],
            "attributesToHighlight": []
        }))
        .send()
        .await
        .unwrap();
    let minimal_bytes = minimal_response.bytes().await.unwrap().len();

    assert!(
        minimal_bytes < full_bytes,
        "minimal ({} bytes) should be smaller than full ({} bytes)",
        minimal_bytes,
        full_bytes
    );
}
