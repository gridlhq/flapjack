/// Integration tests for embedding Flapjack as a library.
///
/// Verifies core search functionality works without HTTP features:
/// - Index-level simple API (JSON-based, auto-commit)
/// - Index-level manual writer API (Document type, explicit commit)
/// - IndexManager-level multi-tenant search
/// - Persistence across open/close cycles
use flapjack::index::{schema::Schema, Index};
use flapjack::types::{Document, FieldValue};
use flapjack::IndexManager;
use serde_json::json;
use std::collections::HashMap;
use tempfile::TempDir;

// ============================================================
// Index-level: Simple JSON API (add_documents_simple)
// ============================================================

#[test]
fn test_simple_api_add_and_verify() {
    let temp_dir = TempDir::new().unwrap();
    let index = Index::create_in_dir(temp_dir.path()).unwrap();

    let docs = vec![
        json!({
            "objectID": "1",
            "title": "MacBook Pro",
            "description": "Powerful laptop for developers",
            "price": 2399
        }),
        json!({
            "objectID": "2",
            "title": "iPhone 15",
            "description": "Latest smartphone with advanced features",
            "price": 999
        }),
    ];

    index.add_documents_simple(&docs).unwrap();

    let reader = index.reader();
    let searcher = reader.searcher();
    let num_docs: usize = searcher
        .segment_readers()
        .iter()
        .map(|r| r.num_docs() as usize)
        .sum();
    assert_eq!(num_docs, 2);
}

#[test]
fn test_simple_api_accepts_underscore_id() {
    // _id is the internal convention; must also work
    let temp_dir = TempDir::new().unwrap();
    let index = Index::create_in_dir(temp_dir.path()).unwrap();

    index
        .add_documents_simple(&[
            json!({"_id": "a1", "name": "Alpha"}),
            json!({"_id": "b2", "name": "Beta"}),
        ])
        .unwrap();

    let reader = index.reader();
    let searcher = reader.searcher();
    let num_docs: usize = searcher
        .segment_readers()
        .iter()
        .map(|r| r.num_docs() as usize)
        .sum();
    assert_eq!(num_docs, 2);
}

// ============================================================
// Index-level: Manual writer API (Document type)
// ============================================================

#[test]
fn test_manual_writer_with_document_type() {
    let temp_dir = TempDir::new().unwrap();
    let schema = Schema::builder().build();
    let index = Index::create(temp_dir.path(), schema).unwrap();

    // Build a Document struct (the type add_document expects)
    let mut fields = HashMap::new();
    fields.insert(
        "content".to_string(),
        FieldValue::Text("This is a test document".to_string()),
    );
    fields.insert(
        "category".to_string(),
        FieldValue::Text("testing".to_string()),
    );
    let doc = Document {
        id: "test-1".to_string(),
        fields,
    };

    let mut writer = index.writer().unwrap();
    index.add_document(&mut writer, doc).unwrap();
    writer.commit().unwrap();

    let reader = index.reader();
    reader.reload().unwrap();
    let searcher = reader.searcher();
    let num_docs: usize = searcher
        .segment_readers()
        .iter()
        .map(|r| r.num_docs() as usize)
        .sum();
    assert_eq!(num_docs, 1);
}

// ============================================================
// Multiple isolated indexes
// ============================================================

#[test]
fn test_multiple_isolated_indexes() {
    let temp_dir = TempDir::new().unwrap();

    let products_path = temp_dir.path().join("products");
    let customers_path = temp_dir.path().join("customers");

    let products_index = Index::create_in_dir(&products_path).unwrap();
    let customers_index = Index::create_in_dir(&customers_path).unwrap();

    products_index
        .add_documents_simple(&[
            json!({"objectID": "p1", "name": "Widget"}),
            json!({"objectID": "p2", "name": "Gadget"}),
        ])
        .unwrap();

    customers_index
        .add_documents_simple(&[json!({"objectID": "c1", "name": "Alice"})])
        .unwrap();

    // Verify isolation
    let p_count: usize = products_index
        .reader()
        .searcher()
        .segment_readers()
        .iter()
        .map(|r| r.num_docs() as usize)
        .sum();
    let c_count: usize = customers_index
        .reader()
        .searcher()
        .segment_readers()
        .iter()
        .map(|r| r.num_docs() as usize)
        .sum();

    assert_eq!(p_count, 2);
    assert_eq!(c_count, 1);
}

// ============================================================
// Persistence across open/close
// ============================================================

#[test]
fn test_persistence_across_reopen() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().to_owned();

    // Create and populate
    {
        let index = Index::create_in_dir(&path).unwrap();
        index
            .add_documents_simple(&[
                json!({"objectID": "1", "title": "Persistent Document"}),
                json!({"objectID": "2", "title": "Another Document"}),
            ])
            .unwrap();
    }

    // Reopen (simulates app restart)
    {
        let index = Index::open(&path).unwrap();
        let reader = index.reader();
        reader.reload().unwrap();
        let searcher = reader.searcher();
        let num_docs: usize = searcher
            .segment_readers()
            .iter()
            .map(|r| r.num_docs() as usize)
            .sum();
        assert_eq!(num_docs, 2);
    }
}

// ============================================================
// IndexManager: Multi-tenant search
// ============================================================

#[tokio::test]
async fn test_manager_add_and_search() {
    let temp_dir = TempDir::new().unwrap();
    let manager = IndexManager::new(temp_dir.path());
    manager.create_tenant("products").unwrap();

    let docs = vec![
        Document {
            id: "1".to_string(),
            fields: HashMap::from([
                (
                    "title".to_string(),
                    FieldValue::Text("MacBook Pro laptop".to_string()),
                ),
                ("price".to_string(), FieldValue::Integer(2399)),
            ]),
        },
        Document {
            id: "2".to_string(),
            fields: HashMap::from([
                (
                    "title".to_string(),
                    FieldValue::Text("iPhone smartphone".to_string()),
                ),
                ("price".to_string(), FieldValue::Integer(999)),
            ]),
        },
    ];
    manager.add_documents_sync("products", docs).await.unwrap();

    // Search
    let results = manager
        .search("products", "laptop", None, None, 10)
        .unwrap();
    assert!(results.total > 0, "Expected search results for 'laptop'");
    assert!(
        results.documents.iter().any(|d| d.document.id == "1"),
        "Expected MacBook Pro in results"
    );
}

#[tokio::test]
async fn test_manager_tenant_isolation() {
    let temp_dir = TempDir::new().unwrap();
    let manager = IndexManager::new(temp_dir.path());
    manager.create_tenant("alpha").unwrap();
    manager.create_tenant("beta").unwrap();

    let alpha_docs = vec![Document {
        id: "a1".to_string(),
        fields: HashMap::from([(
            "name".to_string(),
            FieldValue::Text("Alpha item".to_string()),
        )]),
    }];
    let beta_docs = vec![Document {
        id: "b1".to_string(),
        fields: HashMap::from([(
            "name".to_string(),
            FieldValue::Text("Beta item".to_string()),
        )]),
    }];

    manager
        .add_documents_sync("alpha", alpha_docs)
        .await
        .unwrap();
    manager.add_documents_sync("beta", beta_docs).await.unwrap();

    let alpha_results = manager.search("alpha", "Alpha", None, None, 10).unwrap();
    let beta_results = manager.search("beta", "Alpha", None, None, 10).unwrap();

    assert!(alpha_results.total > 0, "Alpha tenant should find 'Alpha'");
    assert_eq!(beta_results.total, 0, "Beta tenant should not find 'Alpha'");
}
