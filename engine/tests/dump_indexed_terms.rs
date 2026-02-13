use flapjack::index::manager::IndexManager;
use flapjack::types::{Document, FieldValue};
use std::collections::HashMap;
use tempfile::TempDir;

#[tokio::test]
async fn dump_terms() -> flapjack::error::Result<()> {
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
        fields,
    };
    manager.add_documents_sync("tenant", vec![doc]).await?;

    let index = manager.get_or_load("tenant")?;
    let reader = index.reader();
    reader.reload()?;
    let searcher = reader.searcher();
    let schema = index.inner().schema();

    let search_field = schema.get_field("_json_search").unwrap();

    eprintln!("\n=== INDEXED TERMS IN _json_search ===");
    for segment in searcher.segment_readers() {
        let inv_index = segment.inverted_index(search_field).unwrap();
        let mut terms = inv_index.terms().stream().unwrap();

        while terms.advance() {
            let term_bytes = terms.key();
            eprintln!("Term: {:?}", String::from_utf8_lossy(term_bytes));
        }
    }

    Ok(())
}
