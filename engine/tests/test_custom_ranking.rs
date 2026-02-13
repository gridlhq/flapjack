use flapjack::index::settings::IndexSettings;
use flapjack::IndexManager;
use serde_json::json;
use tempfile::TempDir;

#[tokio::test]
async fn test_custom_ranking_desc() {
    let temp = TempDir::new().unwrap();
    let manager = IndexManager::new(temp.path());
    manager.create_tenant("test").unwrap();

    let settings = IndexSettings {
        custom_ranking: Some(vec!["desc(popularity)".to_string()]),
        ..Default::default()
    };
    settings
        .save(temp.path().join("test/settings.json"))
        .unwrap();

    let docs = vec![
        json!({"_id": "1", "name": "Product A", "popularity": 100}),
        json!({"_id": "2", "name": "Product B", "popularity": 500}),
        json!({"_id": "3", "name": "Product C", "popularity": 200}),
    ];

    let docs: Vec<_> = docs
        .into_iter()
        .map(|v| flapjack::types::Document::from_json(&v).unwrap())
        .collect();
    manager.add_documents_sync("test", docs).await.unwrap();

    let results = manager.search("test", "product", None, None, 10).unwrap();

    assert_eq!(results.documents.len(), 3);
    assert_eq!(results.documents[0].document.id, "2");
    assert_eq!(results.documents[1].document.id, "3");
    assert_eq!(results.documents[2].document.id, "1");
}

#[tokio::test]
async fn test_custom_ranking_asc() {
    let temp = TempDir::new().unwrap();
    let manager = IndexManager::new(temp.path());
    manager.create_tenant("test").unwrap();

    let settings = IndexSettings {
        custom_ranking: Some(vec!["asc(price)".to_string()]),
        ..Default::default()
    };
    settings
        .save(temp.path().join("test/settings.json"))
        .unwrap();

    let docs = vec![
        json!({"_id": "1", "name": "Product A", "price": 100}),
        json!({"_id": "2", "name": "Product B", "price": 50}),
        json!({"_id": "3", "name": "Product C", "price": 200}),
    ];

    let docs: Vec<_> = docs
        .into_iter()
        .map(|v| flapjack::types::Document::from_json(&v).unwrap())
        .collect();
    manager.add_documents_sync("test", docs).await.unwrap();

    let results = manager.search("test", "product", None, None, 10).unwrap();

    assert_eq!(results.documents.len(), 3);
    assert_eq!(results.documents[0].document.id, "2");
    assert_eq!(results.documents[1].document.id, "1");
    assert_eq!(results.documents[2].document.id, "3");
}

#[tokio::test]
async fn test_custom_ranking_multiple() {
    let temp = TempDir::new().unwrap();
    let manager = IndexManager::new(temp.path());
    manager.create_tenant("test").unwrap();

    let settings = IndexSettings {
        custom_ranking: Some(vec![
            "desc(category_rank)".to_string(),
            "asc(price)".to_string(),
        ]),
        ..Default::default()
    };
    settings
        .save(temp.path().join("test/settings.json"))
        .unwrap();

    let docs = vec![
        json!({"_id": "1", "name": "Product A", "category_rank": 1, "price": 100}),
        json!({"_id": "2", "name": "Product B", "category_rank": 2, "price": 50}),
        json!({"_id": "3", "name": "Product C", "category_rank": 2, "price": 200}),
        json!({"_id": "4", "name": "Product D", "category_rank": 1, "price": 80}),
    ];

    let docs: Vec<_> = docs
        .into_iter()
        .map(|v| flapjack::types::Document::from_json(&v).unwrap())
        .collect();
    manager.add_documents_sync("test", docs).await.unwrap();

    let results = manager.search("test", "product", None, None, 10).unwrap();

    assert_eq!(results.documents.len(), 4);
    assert_eq!(results.documents[0].document.id, "2");
    assert_eq!(results.documents[1].document.id, "3");
    assert_eq!(results.documents[2].document.id, "4");
    assert_eq!(results.documents[3].document.id, "1");
}

#[tokio::test]
async fn test_custom_ranking_missing_values() {
    let temp = TempDir::new().unwrap();
    let manager = IndexManager::new(temp.path());
    manager.create_tenant("test").unwrap();

    let settings = IndexSettings {
        custom_ranking: Some(vec!["desc(popularity)".to_string()]),
        ..Default::default()
    };
    settings
        .save(temp.path().join("test/settings.json"))
        .unwrap();

    let docs = vec![
        json!({"_id": "1", "name": "Product A", "popularity": 100}),
        json!({"_id": "2", "name": "Product B"}),
        json!({"_id": "3", "name": "Product C", "popularity": 200}),
    ];

    let docs: Vec<_> = docs
        .into_iter()
        .map(|v| flapjack::types::Document::from_json(&v).unwrap())
        .collect();
    manager.add_documents_sync("test", docs).await.unwrap();

    let results = manager.search("test", "product", None, None, 10).unwrap();

    assert_eq!(results.documents.len(), 3);
    assert_eq!(results.documents[0].document.id, "3");
    assert_eq!(results.documents[1].document.id, "1");
    assert_eq!(results.documents[2].document.id, "2");
}
