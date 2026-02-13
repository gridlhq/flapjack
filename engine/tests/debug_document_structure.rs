use flapjack::index::manager::IndexManager;
use flapjack::types::{Document, FieldValue};
use std::collections::HashMap;
use tempfile::TempDir;

#[tokio::test]
async fn debug_array_in_document() -> flapjack::error::Result<()> {
    let temp_dir = TempDir::new()?;
    let manager = IndexManager::new(temp_dir.path());

    manager.create_tenant("tenant")?;

    let mut fields = HashMap::new();
    fields.insert("name".to_string(), FieldValue::Text("Test".to_string()));
    fields.insert(
        "tags".to_string(),
        FieldValue::Array(vec![
            FieldValue::Text("laptop".to_string()),
            FieldValue::Text("gaming".to_string()),
        ]),
    );

    let doc = Document {
        id: "1".to_string(),
        fields: fields.clone(),
    };

    manager.add_documents_sync("tenant", vec![doc]).await?;

    let retrieved = manager
        .get_document("tenant", "1")?
        .expect("Document should exist");

    eprintln!("Original fields: {:?}", fields);
    eprintln!("Retrieved fields: {:?}", retrieved.fields);
    eprintln!("Has tags? {}", retrieved.fields.contains_key("tags"));

    if let Some(tags) = retrieved.fields.get("tags") {
        eprintln!("Tags value: {:?}", tags);
    }

    eprintln!("\n=== SEARCH TEST ===");
    let result = manager.search("tenant", "laptop", None, None, 10)?;
    eprintln!("Search result count: {}", result.total);
    eprintln!("Query was: 'laptop'");
    if let Some(doc) = result.documents.first() {
        eprintln!("Search doc fields: {:?}", doc.document.fields);
    }

    assert!(
        retrieved.fields.contains_key("tags"),
        "tags field missing after round-trip"
    );

    Ok(())
}
