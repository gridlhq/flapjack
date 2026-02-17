use flapjack::index::manager::IndexManager;
use flapjack::query::QueryParser;
use flapjack::types::{Document, FieldValue};
use std::collections::HashMap;
use tempfile::TempDir;

#[tokio::test]
async fn debug_query() -> flapjack::error::Result<()> {
    let temp_dir = TempDir::new()?;
    let manager = IndexManager::new(temp_dir.path());

    manager.create_tenant("tenant")?;

    let mut fields = HashMap::new();
    fields.insert(
        "tags".to_string(),
        FieldValue::Array(vec![FieldValue::Text("laptop".to_string())]),
    );

    let doc = Document {
        id: "1".to_string(),
        fields,
    };
    manager.add_documents_sync("tenant", vec![doc]).await?;

    let index = manager.get_or_load("tenant")?;
    let schema = index.inner().schema();
    let search_field = schema.get_field("_json_search").unwrap();

    let parser = QueryParser::new_with_weights(
        &schema,
        vec![search_field],
        vec![1.0],
        vec!["tags".to_string()],
    );

    let query = flapjack::types::Query {
        text: "laptop".to_string(),
    };
    let parsed = parser.parse(&query)?;

    eprintln!("Parsed query: {:?}", parsed);

    let reader = index.reader();
    reader.reload()?;
    let searcher = reader.searcher();
    let top_docs = searcher.search(&parsed, &tantivy::collector::TopDocs::with_limit(10))?;

    eprintln!("Results: {} hits", top_docs.len());

    Ok(())
}
