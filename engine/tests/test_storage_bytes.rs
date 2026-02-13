use serde_json::json;

mod common;

/// After adding documents and waiting for indexing, the list_indices endpoint
/// must report a non-zero `dataSize` for the index.
#[tokio::test]
async fn test_list_indices_reports_nonzero_data_size() {
    let (addr, _temp) = common::spawn_server().await;
    let client = reqwest::Client::new();

    // Add documents to create and populate an index
    let resp = client
        .post(format!("http://{}/1/indexes/storage_test/batch", addr))
        .header("x-algolia-api-key", "test")
        .header("x-algolia-application-id", "test")
        .json(&json!({
            "requests": [
                {"action": "addObject", "body": {"objectID": "1", "title": "Hello World", "body": "This is a test document with some content."}},
                {"action": "addObject", "body": {"objectID": "2", "title": "Foo Bar", "body": "Another test document with different content."}},
                {"action": "addObject", "body": {"objectID": "3", "title": "Baz Qux", "body": "Yet another document for good measure."}},
            ]
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // Wait for the write queue to commit (100ms batch timeout + margin)
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    // List indices and check dataSize
    let resp = client
        .get(format!("http://{}/1/indexes", addr))
        .header("x-algolia-api-key", "test")
        .header("x-algolia-application-id", "test")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    let items = body["items"].as_array().expect("items should be an array");
    assert!(!items.is_empty(), "Should have at least one index");

    let index = items
        .iter()
        .find(|i| i["name"] == "storage_test")
        .expect("storage_test index should exist");

    let data_size = index["dataSize"]
        .as_u64()
        .expect("dataSize should be a number");
    let file_size = index["fileSize"]
        .as_u64()
        .expect("fileSize should be a number");
    let entries = index["entries"]
        .as_u64()
        .expect("entries should be a number");

    eprintln!(
        "storage_test: entries={}, dataSize={}, fileSize={}",
        entries, data_size, file_size
    );

    assert_eq!(entries, 3, "Should have 3 documents");
    assert!(
        data_size > 0,
        "dataSize should be > 0 after indexing documents, got {}",
        data_size
    );
    assert_eq!(data_size, file_size, "dataSize and fileSize should match");
}

/// Verify that numberOfPendingTasks reflects actual pending tasks.
#[tokio::test]
async fn test_list_indices_reports_pending_tasks() {
    let (addr, _temp) = common::spawn_server().await;
    let client = reqwest::Client::new();

    // Add documents — immediately after, tasks should be pending or already processed
    client
        .post(format!("http://{}/1/indexes/task_test/batch", addr))
        .header("x-algolia-api-key", "test")
        .header("x-algolia-application-id", "test")
        .json(&json!({
            "requests": [
                {"action": "addObject", "body": {"objectID": "1", "title": "Test"}}
            ]
        }))
        .send()
        .await
        .unwrap();

    // After processing completes, pending tasks should be 0
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    let resp = client
        .get(format!("http://{}/1/indexes", addr))
        .header("x-algolia-api-key", "test")
        .header("x-algolia-application-id", "test")
        .send()
        .await
        .unwrap();

    let body: serde_json::Value = resp.json().await.unwrap();
    let items = body["items"].as_array().unwrap();
    let index = items
        .iter()
        .find(|i| i["name"] == "task_test")
        .expect("task_test index should exist");

    let pending = index["numberOfPendingTasks"].as_u64().unwrap();
    assert_eq!(
        pending, 0,
        "After indexing completes, numberOfPendingTasks should be 0"
    );
}
