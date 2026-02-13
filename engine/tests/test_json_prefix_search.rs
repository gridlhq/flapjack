use flapjack::index::manager::IndexManager;
use flapjack::types::Document;
use serde_json::json;
use tempfile::TempDir;

#[tokio::test]
async fn test_single_word_prefix_on_json_field() {
    let temp_dir = TempDir::new().unwrap();
    let manager = IndexManager::new(temp_dir.path());

    manager.create_tenant("products").unwrap();

    let docs = [
        json!({"_id": "1", "title": "Gaming Laptop"}),
        json!({"_id": "2", "title": "Laptop Stand"}),
    ];

    let doc_objs: Vec<Document> = docs
        .iter()
        .map(|d| Document::from_json(d).unwrap())
        .collect();

    manager
        .add_documents_sync("products", doc_objs)
        .await
        .unwrap();

    let results = manager.search("products", "lap", None, None, 10).unwrap();

    assert_eq!(
        results.documents.len(),
        2,
        "Expected 'lap' prefix to match both 'Gaming Laptop' and 'Laptop Stand'. Found {} docs",
        results.documents.len()
    );
}

#[tokio::test]
async fn test_multi_word_query_structure() {
    let temp_dir = TempDir::new().unwrap();
    let manager = IndexManager::new(temp_dir.path());

    manager.create_tenant("products").unwrap();

    let docs = [
        json!({"_id": "1", "title": "Gaming Laptop"}),
        json!({"_id": "2", "title": "Laptop Gaming Mouse"}),
        json!({"_id": "3", "title": "Gaming Mouse"}),
    ];

    let doc_objs: Vec<Document> = docs
        .iter()
        .map(|d| Document::from_json(d).unwrap())
        .collect();

    manager
        .add_documents_sync("products", doc_objs)
        .await
        .unwrap();

    let results = manager
        .search("products", "gaming lap", None, None, 10)
        .unwrap();

    println!(
        "Multi-word query 'gaming lap' returned {} results",
        results.documents.len()
    );
    for (i, doc) in results.documents.iter().enumerate() {
        println!("  Result {}: {:?}", i, doc.document.fields);
    }

    assert!(
        !results.documents.is_empty(),
        "Expected 'gaming lap' to match at least 'Gaming Laptop'"
    );
}
