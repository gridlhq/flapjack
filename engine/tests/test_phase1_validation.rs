use flapjack::IndexManager;
use serde_json::json;
use tempfile::TempDir;

#[tokio::test]
async fn test_schemaless_prefix_search_end_to_end() {
    let temp_dir = TempDir::new().unwrap();
    let manager = IndexManager::new(temp_dir.path());

    manager.create_tenant("test").unwrap();

    let docs = vec![
        json!({"_id": "1", "title": "Gaming Laptop", "price": 1200}),
        json!({"_id": "2", "title": "Office Laptop", "price": 800}),
        json!({"_id": "3", "title": "Laptop Stand", "price": 50}),
    ]
    .into_iter()
    .map(|v| flapjack::types::Document::from_json(&v).unwrap())
    .collect();
    manager.add_documents_sync("test", docs).await.unwrap();

    let results = manager.search("test", "lap", None, None, 10).unwrap();
    assert_eq!(
        results.documents.len(),
        3,
        "All docs with 'laptop' should match"
    );

    let results = manager.search("test", "gam", None, None, 10).unwrap();
    assert_eq!(
        results.documents.len(),
        1,
        "Only Gaming Laptop should match"
    );
    assert_eq!(
        results.documents[0]
            .document
            .fields
            .get("title")
            .unwrap()
            .as_text()
            .unwrap(),
        "Gaming Laptop"
    );

    let results = manager.search("test", "lpatop", None, None, 10).unwrap();
    assert_eq!(
        results.documents.len(),
        3,
        "Typo tolerance should match all laptops"
    );
}

#[tokio::test]
async fn test_filter_numeric_range() {
    let temp_dir = TempDir::new().unwrap();
    let manager = IndexManager::new(temp_dir.path());

    manager.create_tenant("test").unwrap();

    let docs = vec![
        json!({"_id": "1", "title": "Laptop", "price": 1200}),
        json!({"_id": "2", "title": "Laptop", "price": 800}),
        json!({"_id": "3", "title": "Laptop", "price": 50}),
    ]
    .into_iter()
    .map(|v| flapjack::types::Document::from_json(&v).unwrap())
    .collect();
    manager.add_documents_sync("test", docs).await.unwrap();

    let filter = flapjack::types::Filter::GreaterThanOrEqual {
        field: "price".to_string(),
        value: flapjack::types::FieldValue::Integer(1000),
    };

    let results = manager
        .search("test", "laptop", Some(&filter), None, 10)
        .unwrap();
    assert_eq!(results.documents.len(), 1, "Only doc with price >= 1000");
    assert_eq!(results.documents[0].document.id, "1");
}
#[tokio::test]
async fn test_filter_and_combination() {
    let temp_dir = TempDir::new().unwrap();
    let manager = IndexManager::new(temp_dir.path());

    let _ = manager.delete_tenant(&"test".to_string()).await;
    manager.create_tenant("test").unwrap();

    let settings =
        flapjack::index::settings::IndexSettings::default_with_facets(vec!["category".to_string()]);
    let settings_path = temp_dir.path().join("test").join("settings.json");
    settings.save(&settings_path).unwrap();

    let docs = vec![
        json!({"_id": "1", "title": "Gaming Laptop", "price": 1200, "category": "electronics"}),
        json!({"_id": "2", "title": "Office Laptop", "price": 800, "category": "electronics"}),
        json!({"_id": "3", "title": "Laptop Stand", "price": 50, "category": "accessories"}),
    ]
    .into_iter()
    .map(|v| flapjack::types::Document::from_json(&v).unwrap())
    .collect();
    manager.add_documents_sync("test", docs).await.unwrap();

    let filter = flapjack::types::Filter::And(vec![
        flapjack::types::Filter::GreaterThanOrEqual {
            field: "price".to_string(),
            value: flapjack::types::FieldValue::Integer(1000),
        },
        flapjack::types::Filter::Equals {
            field: "category".to_string(),
            value: flapjack::types::FieldValue::Text("electronics".to_string()),
        },
    ]);

    let results = manager
        .search("test", "laptop", Some(&filter), None, 10)
        .unwrap();
    assert_eq!(
        results.documents.len(),
        1,
        "Only Gaming Laptop matches both filters"
    );
    assert_eq!(results.documents[0].document.id, "1");
}
#[tokio::test]
async fn test_nested_field_queries() {
    let temp_dir = TempDir::new().unwrap();
    let manager = IndexManager::new(temp_dir.path());

    manager.create_tenant("test").unwrap();

    let docs = vec![
        json!({"_id": "1", "product": {"name": "Laptop", "brand": "Dell"}}),
        json!({"_id": "2", "product": {"name": "Mouse", "brand": "Logitech"}}),
    ]
    .into_iter()
    .map(|v| flapjack::types::Document::from_json(&v).unwrap())
    .collect();
    manager.add_documents_sync("test", docs).await.unwrap();

    let results = manager.search("test", "laptop", None, None, 10).unwrap();
    assert_eq!(
        results.documents.len(),
        1,
        "Should find nested product.name='Laptop'"
    );
    assert_eq!(results.documents[0].document.id, "1");
}

#[tokio::test]
async fn test_mixed_type_documents() {
    let temp_dir = TempDir::new().unwrap();
    let manager = IndexManager::new(temp_dir.path());

    manager.create_tenant("test").unwrap();

    let docs = vec![
        json!({"_id": "1", "title": "Laptop", "price": 1200}),
        json!({"_id": "2", "title": "Mouse", "price": "expensive"}),
        json!({"_id": "3", "title": "Keyboard", "price": 50}),
    ]
    .into_iter()
    .map(|v| flapjack::types::Document::from_json(&v).unwrap())
    .collect();
    manager.add_documents_sync("test", docs).await.unwrap();

    let filter = flapjack::types::Filter::GreaterThanOrEqual {
        field: "price".to_string(),
        value: flapjack::types::FieldValue::Integer(1000),
    };

    let results = manager
        .search("test", "laptop", Some(&filter), None, 10)
        .unwrap();
    assert_eq!(
        results.documents.len(),
        1,
        "Only numeric price >= 1000 should match (Algolia silent fail behavior)"
    );
    assert_eq!(results.documents[0].document.id, "1");
}

#[tokio::test]
async fn test_null_handling() {
    let temp_dir = TempDir::new().unwrap();
    let manager = IndexManager::new(temp_dir.path());

    manager.create_tenant("test").unwrap();

    let settings =
        flapjack::index::settings::IndexSettings::default_with_facets(vec!["tags".to_string()]);
    let settings_path = temp_dir.path().join("test").join("settings.json");
    settings.save(&settings_path).unwrap();

    let docs = vec![
        json!({"_id": "1", "title": "Product A", "description": null}),
        json!({"_id": "2", "title": "Product B", "tags": [null, "sale", null]}),
        json!({"_id": "3", "title": "Product C", "tags": ["new", "featured"]}),
    ]
    .into_iter()
    .map(|v| flapjack::types::Document::from_json(&v).unwrap())
    .collect();
    manager.add_documents_sync("test", docs).await.unwrap();

    let results = manager.search("test", "prod", None, None, 10).unwrap();
    assert_eq!(
        results.documents.len(),
        3,
        "All products should be indexed despite nulls"
    );

    let filter = flapjack::types::Filter::Equals {
        field: "tags".to_string(),
        value: flapjack::types::FieldValue::Text("sale".to_string()),
    };
    let filtered = manager
        .search("test", "prod", Some(&filter), None, 10)
        .unwrap();
    assert_eq!(
        filtered.documents.len(),
        1,
        "Should filter on non-null array values"
    );
    assert_eq!(filtered.documents[0].document.id, "2");
}
