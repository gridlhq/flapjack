use flapjack::index::manager::IndexManager;
use flapjack::index::schema::Schema;

use flapjack::types::Document;
use std::collections::HashMap;
use tempfile::TempDir;

#[tokio::test]
async fn test_basic_highlighting() -> flapjack::error::Result<()> {
    let temp_dir = TempDir::new()?;
    let manager = IndexManager::new(temp_dir.path());

    let _schema = Schema::builder().build();
    manager.create_tenant("tenant")?;

    let mut fields = HashMap::new();
    fields.insert(
        "name".to_string(),
        flapjack::types::FieldValue::Text("Gaming Laptop".to_string()),
    );
    fields.insert(
        "brand".to_string(),
        flapjack::types::FieldValue::Text("Dell".to_string()),
    );

    let doc = Document {
        id: "1".to_string(),
        fields,
    };

    manager.add_documents_sync("tenant", vec![doc]).await?;

    let result = manager.search("tenant", "laptop", None, None, 10)?;

    assert_eq!(result.total, 1);
    Ok(())
}

#[tokio::test]
async fn test_highlighting_structure() -> flapjack::error::Result<()> {
    let temp_dir = TempDir::new()?;
    let manager = IndexManager::new(temp_dir.path());

    manager.create_tenant("tenant")?;

    let mut fields = HashMap::new();
    fields.insert(
        "title".to_string(),
        flapjack::types::FieldValue::Text("Action Movies".to_string()),
    );

    let doc = Document {
        id: "1".to_string(),
        fields,
    };

    manager.add_documents_sync("tenant", vec![doc]).await?;

    let result = manager.search("tenant", "action", None, None, 10)?;
    assert_eq!(result.total, 1);

    Ok(())
}

#[tokio::test]
async fn test_highlighting_arrays() -> flapjack::error::Result<()> {
    let temp_dir = TempDir::new()?;
    let manager = IndexManager::new(temp_dir.path());

    manager.create_tenant("tenant")?;

    let mut fields = HashMap::new();
    fields.insert(
        "tags".to_string(),
        flapjack::types::FieldValue::Array(vec![
            flapjack::types::FieldValue::Text("laptop".to_string()),
            flapjack::types::FieldValue::Text("gaming".to_string()),
        ]),
    );

    let doc = Document {
        id: "1".to_string(),
        fields,
    };

    manager.add_documents_sync("tenant", vec![doc]).await?;

    let result = manager.search("tenant", "laptop", None, None, 10)?;
    assert_eq!(result.total, 1);

    Ok(())
}

#[tokio::test]
async fn test_match_level_full() -> flapjack::error::Result<()> {
    let temp_dir = TempDir::new()?;
    let manager = IndexManager::new(temp_dir.path());

    manager.create_tenant("tenant")?;

    let mut fields = HashMap::new();
    fields.insert(
        "name".to_string(),
        flapjack::types::FieldValue::Text("Gaming Laptop".to_string()),
    );

    let doc = Document {
        id: "1".to_string(),
        fields,
    };

    manager.add_documents_sync("tenant", vec![doc]).await?;

    let result = manager.search("tenant", "gaming laptop", None, None, 10)?;
    assert_eq!(result.total, 1);

    Ok(())
}

#[tokio::test]
async fn test_match_level_partial() -> flapjack::error::Result<()> {
    let temp_dir = TempDir::new()?;
    let manager = IndexManager::new(temp_dir.path());

    manager.create_tenant("tenant")?;

    let mut fields = HashMap::new();
    fields.insert(
        "name".to_string(),
        flapjack::types::FieldValue::Text("Gaming Laptop".to_string()),
    );

    let doc = Document {
        id: "1".to_string(),
        fields,
    };

    manager.add_documents_sync("tenant", vec![doc]).await?;

    let result = manager.search("tenant", "gaming", None, None, 10)?;
    assert_eq!(result.total, 1);

    Ok(())
}

#[tokio::test]
async fn test_no_highlight_on_empty_query() -> flapjack::error::Result<()> {
    let temp_dir = TempDir::new()?;
    let manager = IndexManager::new(temp_dir.path());

    manager.create_tenant("tenant")?;

    let mut fields = HashMap::new();
    fields.insert(
        "name".to_string(),
        flapjack::types::FieldValue::Text("Product".to_string()),
    );

    let doc = Document {
        id: "1".to_string(),
        fields,
    };

    manager.add_documents_sync("tenant", vec![doc]).await?;

    let result = manager.search("tenant", "", None, None, 10)?;
    assert_eq!(result.total, 1);

    Ok(())
}
#[tokio::test]
async fn test_highlight_only_searchable_attributes() -> flapjack::error::Result<()> {
    use flapjack::query::highlighter::{extract_query_words, Highlighter};
    use flapjack::types::{Document, FieldValue};
    use std::collections::HashMap;

    let mut fields = HashMap::new();
    fields.insert(
        "name".to_string(),
        FieldValue::Text("Samsung Galaxy".to_string()),
    );
    fields.insert("brand".to_string(), FieldValue::Text("Samsung".to_string()));
    fields.insert("price".to_string(), FieldValue::Text("999".to_string()));
    fields.insert(
        "url".to_string(),
        FieldValue::Text("https://samsung.com/galaxy".to_string()),
    );
    fields.insert(
        "image".to_string(),
        FieldValue::Text("samsung-logo.png".to_string()),
    );

    let doc = Document {
        id: "1".to_string(),
        fields,
    };
    let highlighter = Highlighter::default();
    let query_words = extract_query_words("samsung");
    let searchable = vec!["name".to_string(), "brand".to_string()];

    let result = highlighter.highlight_document(&doc, &query_words, &searchable);

    assert!(
        result.contains_key("name"),
        "should highlight searchable field 'name'"
    );
    assert!(
        result.contains_key("brand"),
        "should highlight searchable field 'brand'"
    );
    // Algolia includes ALL attributes in _highlightResult (not just searchable
    // ones).  Non-searchable fields are highlighted with query words too.
    assert!(
        result.contains_key("price"),
        "non-searchable fields should be included in _highlightResult"
    );
    assert!(
        result.contains_key("url"),
        "non-searchable fields should be included in _highlightResult"
    );
    assert!(
        result.contains_key("image"),
        "non-searchable fields should be included in _highlightResult"
    );

    Ok(())
}

#[tokio::test]
async fn test_highlight_all_when_no_searchable_specified() -> flapjack::error::Result<()> {
    use flapjack::query::highlighter::{extract_query_words, Highlighter};
    use flapjack::types::{Document, FieldValue};
    use std::collections::HashMap;

    let mut fields = HashMap::new();
    fields.insert(
        "name".to_string(),
        FieldValue::Text("Samsung Galaxy".to_string()),
    );
    fields.insert(
        "url".to_string(),
        FieldValue::Text("https://samsung.com".to_string()),
    );

    let doc = Document {
        id: "1".to_string(),
        fields,
    };
    let highlighter = Highlighter::default();
    let query_words = extract_query_words("samsung");
    let searchable: Vec<String> = vec![];

    let result = highlighter.highlight_document(&doc, &query_words, &searchable);

    assert!(
        result.contains_key("name"),
        "should highlight all fields when searchable is empty"
    );
    assert!(
        result.contains_key("url"),
        "should highlight all fields when searchable is empty"
    );

    Ok(())
}
