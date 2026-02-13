mod common;

#[tokio::test]
async fn test_and_of_ors_returns_correct_results() {
    let (addr, _temp) = common::spawn_server().await;
    let client = reqwest::Client::new();

    let docs = [
        serde_json::json!({"objectID": "1", "brand": "Nike", "category": "Shoes"}),
        serde_json::json!({"objectID": "2", "brand": "Adidas", "category": "Shoes"}),
        serde_json::json!({"objectID": "3", "brand": "Nike", "category": "Apparel"}),
        serde_json::json!({"objectID": "4", "brand": "Puma", "category": "Shoes"}),
    ];

    client
        .post(format!("http://{}/1/indexes/products/settings", addr))
        .header("x-algolia-api-key", "test")
        .header("x-algolia-application-id", "test")
        .json(&serde_json::json!({"attributesForFaceting": ["brand", "category"]}))
        .send()
        .await
        .unwrap();

    client
        .post(format!("http://{}/1/indexes/products/batch", addr))
        .header("x-algolia-api-key", "test")
        .header("x-algolia-application-id", "test")
        .json(&serde_json::json!({
            "requests": docs.iter().map(|d|
                serde_json::json!({"action": "addObject", "body": d})
            ).collect::<Vec<_>>()
        }))
        .send()
        .await
        .unwrap();

    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    // (brand:Nike OR brand:Adidas) AND category:Shoes
    // Should match: 1 (Nike+Shoes), 2 (Adidas+Shoes)
    // Should NOT match: 3 (Nike+Apparel), 4 (Puma+Shoes)
    let response = client
        .post(format!("http://{}/1/indexes/products/query", addr))
        .header("x-algolia-api-key", "test")
        .header("x-algolia-application-id", "test")
        .json(&serde_json::json!({
            "query": "",
            "filters": "(brand:Nike OR brand:Adidas) AND category:Shoes"
        }))
        .send()
        .await
        .unwrap();

    let body: serde_json::Value = response.json().await.unwrap();
    let hits = body["hits"].as_array().unwrap();

    assert_eq!(hits.len(), 2, "Should match exactly 2 docs");

    let ids: Vec<&str> = hits
        .iter()
        .map(|h| h["objectID"].as_str().unwrap())
        .collect();

    assert!(ids.contains(&"1"), "Should include Nike+Shoes");
    assert!(ids.contains(&"2"), "Should include Adidas+Shoes");
}

#[tokio::test]
async fn test_nested_precedence() {
    let (addr, _temp) = common::spawn_server().await;
    let client = reqwest::Client::new();

    let docs = [
        serde_json::json!({"objectID": "1", "a": "x", "b": "y", "c": "z"}),
        serde_json::json!({"objectID": "2", "a": "x", "b": "w", "c": "z"}),
        serde_json::json!({"objectID": "3", "a": "v", "b": "y", "c": "z"}),
        serde_json::json!({"objectID": "4", "a": "v", "b": "w", "c": "q"}),
    ];

    client
        .post(format!("http://{}/1/indexes/test/settings", addr))
        .header("x-algolia-api-key", "test")
        .header("x-algolia-application-id", "test")
        .json(&serde_json::json!({"attributesForFaceting": ["a", "b", "c"]}))
        .send()
        .await
        .unwrap();

    client
        .post(format!("http://{}/1/indexes/test/batch", addr))
        .header("x-algolia-api-key", "test")
        .header("x-algolia-application-id", "test")
        .json(&serde_json::json!({
            "requests": docs.iter().map(|d|
                serde_json::json!({"action": "addObject", "body": d})
            ).collect::<Vec<_>>()
        }))
        .send()
        .await
        .unwrap();

    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    // ((a:x OR a:v) AND b:y) OR c:q
    // Should match: 1 (x+y), 3 (v+y), 4 (q)
    let response = client
        .post(format!("http://{}/1/indexes/test/query", addr))
        .header("x-algolia-api-key", "test")
        .header("x-algolia-application-id", "test")
        .json(&serde_json::json!({
            "query": "",
            "filters": "((a:x OR a:v) AND b:y) OR c:q"
        }))
        .send()
        .await
        .unwrap();

    let body: serde_json::Value = response.json().await.unwrap();
    let hits = body["hits"].as_array().unwrap();

    assert_eq!(hits.len(), 3, "Should match exactly 3 docs");

    let ids: Vec<&str> = hits
        .iter()
        .map(|h| h["objectID"].as_str().unwrap())
        .collect();

    assert!(ids.contains(&"1"));
    assert!(ids.contains(&"3"));
    assert!(ids.contains(&"4"));
}
