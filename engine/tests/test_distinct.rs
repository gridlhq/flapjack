use flapjack::index::settings::{DistinctValue, IndexSettings};
use flapjack::types::{Document, FieldValue};
use flapjack::IndexManager;
use tempfile::TempDir;

#[tokio::test]
async fn test_distinct_deduplicates_variants() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = TempDir::new()?;
    let manager = IndexManager::new(temp_dir.path());

    manager.create_tenant("test")?;

    let settings = IndexSettings {
        custom_ranking: Some(vec!["desc(popularity)".to_string()]),
        attribute_for_distinct: Some("product_id".to_string()),
        distinct: Some(DistinctValue::Bool(true)),
        ..Default::default()
    };
    settings.save(temp_dir.path().join("test/settings.json"))?;

    let docs = vec![
        create_doc("1", "Laptop Red", "laptop-1", 100),
        create_doc("2", "Laptop Blue", "laptop-1", 90),
        create_doc("3", "Laptop Green", "laptop-1", 80),
        create_doc("4", "Mouse Red", "mouse-1", 50),
        create_doc("5", "Mouse Blue", "mouse-1", 40),
    ];
    manager.add_documents_sync("test", docs).await?;

    let result_empty = manager.search("test", "", None, None, 10)?;
    eprintln!("EMPTY query: {} docs", result_empty.documents.len());
    for doc in &result_empty.documents {
        eprintln!(
            "  Doc {}: {:?}",
            doc.document.id,
            doc.document.fields.keys().collect::<Vec<_>>()
        );
    }

    let result_lap = manager.search("test", "lap", None, None, 10)?;
    eprintln!("'lap' query: {} docs", result_lap.documents.len());

    let result_laptop = manager.search("test", "laptop", None, None, 10)?;
    eprintln!("'laptop' query: {} docs", result_laptop.documents.len());

    let result_red = manager.search("test", "red", None, None, 10)?;
    eprintln!("'red' query: {} docs", result_red.documents.len());

    let index = manager.get_or_load("test")?;
    let reader = index.reader();
    reader.reload()?;
    let searcher = reader.searcher();
    let schema = index.inner().schema();
    let json_search = schema.get_field("_json_search").unwrap();

    eprintln!("\nIndexed terms sample:");
    let segment = &searcher.segment_readers()[0];
    let inv = segment.inverted_index(json_search).unwrap();
    let mut terms = inv.terms().stream().unwrap();
    let mut count = 0;
    while terms.advance() && count < 20 {
        let term = String::from_utf8_lossy(terms.key());
        eprintln!("  {}", term);
        count += 1;
    }

    let result_without_distinct = manager.search("test", "laptop", None, None, 10)?;
    eprintln!(
        "\n'laptop' query: {} docs",
        result_without_distinct.documents.len()
    );

    let result = manager.search_with_facets_and_distinct(
        "test",
        "laptop",
        None,
        None,
        10,
        0,
        None,
        Some(1),
    )?;

    eprintln!(
        "WITH distinct: {} docs, total={}",
        result.documents.len(),
        result.total
    );

    assert_eq!(result.total, 1, "Should count 1 group (laptop product)");
    assert_eq!(
        result.documents.len(),
        1,
        "Should return 1 doc (top variant)"
    );
    assert_eq!(result.documents[0].document.id, "1");

    Ok(())
}

#[tokio::test]
async fn test_distinct_keeps_top_n_per_group() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = TempDir::new()?;
    let manager = IndexManager::new(temp_dir.path());

    manager.create_tenant("test")?;

    let settings = IndexSettings {
        custom_ranking: Some(vec!["desc(popularity)".to_string()]),
        attribute_for_distinct: Some("product_id".to_string()),
        distinct: Some(DistinctValue::Integer(2)),
        ..Default::default()
    };
    settings.save(temp_dir.path().join("test/settings.json"))?;

    let docs = vec![
        create_doc("1", "Laptop Red", "laptop-1", 100),
        create_doc("2", "Laptop Blue", "laptop-1", 90),
        create_doc("3", "Laptop Green", "laptop-1", 80),
        create_doc("4", "Laptop Yellow", "laptop-1", 70),
    ];
    manager.add_documents_sync("test", docs).await?;

    let result = manager.search_with_facets_and_distinct(
        "test",
        "laptop",
        None,
        None,
        10,
        0,
        None,
        Some(2),
    )?;

    assert_eq!(result.total, 1, "Should count 1 group");
    assert_eq!(result.documents.len(), 2, "Should return top 2 variants");
    assert_eq!(
        result.documents[0].document.id, "1",
        "Highest popularity first"
    );
    assert_eq!(result.documents[1].document.id, "2", "Second highest");

    Ok(())
}

#[tokio::test]
async fn test_distinct_disabled_returns_all() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = TempDir::new()?;
    let manager = IndexManager::new(temp_dir.path());

    manager.create_tenant("test")?;

    let settings = IndexSettings {
        custom_ranking: Some(vec!["desc(popularity)".to_string()]),
        attribute_for_distinct: Some("product_id".to_string()),
        distinct: Some(DistinctValue::Bool(false)),
        ..Default::default()
    };
    settings.save(temp_dir.path().join("test/settings.json"))?;

    let docs = vec![
        create_doc("1", "Laptop Red", "laptop-1", 100),
        create_doc("2", "Laptop Blue", "laptop-1", 90),
        create_doc("3", "Laptop Green", "laptop-1", 80),
    ];
    manager.add_documents_sync("test", docs).await?;

    let result = manager.search_with_facets_and_distinct(
        "test",
        "laptop",
        None,
        None,
        10,
        0,
        None,
        Some(0),
    )?;

    assert_eq!(result.total, 3, "Should count all docs");
    assert_eq!(result.documents.len(), 3, "Should return all variants");

    Ok(())
}

#[tokio::test]
async fn test_distinct_missing_field_skips_doc() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = TempDir::new()?;
    let manager = IndexManager::new(temp_dir.path());

    manager.create_tenant("test")?;

    let settings = IndexSettings {
        custom_ranking: Some(vec!["desc(popularity)".to_string()]),
        attribute_for_distinct: Some("product_id".to_string()),
        distinct: Some(DistinctValue::Bool(true)),
        ..Default::default()
    };
    settings.save(temp_dir.path().join("test/settings.json"))?;

    let doc1 = create_doc("1", "Laptop Red", "laptop-1", 100);
    let doc2 = create_doc("2", "Laptop Blue", "laptop-1", 90);
    let mut doc3 = create_doc("3", "Mouse", "", 50);
    doc3.fields.remove("product_id");

    manager
        .add_documents_sync("test", vec![doc1, doc2, doc3])
        .await?;

    let result = manager.search_with_facets_and_distinct(
        "test",
        "laptop",
        None,
        None,
        10,
        0,
        None,
        Some(1),
    )?;

    assert_eq!(
        result.documents.len(),
        1,
        "Doc without product_id should be skipped"
    );
    assert_eq!(result.documents[0].document.id, "1");

    Ok(())
}

#[tokio::test]
async fn test_distinct_numeric_field_rounds() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = TempDir::new()?;
    let manager = IndexManager::new(temp_dir.path());

    manager.create_tenant("test")?;

    let settings = IndexSettings {
        custom_ranking: Some(vec!["desc(popularity)".to_string()]),
        attribute_for_distinct: Some("category_id".to_string()),
        distinct: Some(DistinctValue::Bool(true)),
        ..Default::default()
    };
    settings.save(temp_dir.path().join("test/settings.json"))?;

    let mut doc1 = create_doc("1", "Laptop Red", "", 100);
    doc1.fields
        .insert("category_id".to_string(), FieldValue::Integer(42));

    let mut doc2 = create_doc("2", "Laptop Blue", "", 90);
    doc2.fields
        .insert("category_id".to_string(), FieldValue::Integer(42));

    let mut doc3 = create_doc("3", "Mouse", "", 50);
    doc3.fields
        .insert("category_id".to_string(), FieldValue::Integer(99));

    manager
        .add_documents_sync("test", vec![doc1, doc2, doc3])
        .await?;

    let result = manager.search_with_facets_and_distinct(
        "test",
        "laptop",
        None,
        None,
        10,
        0,
        None,
        Some(1),
    )?;

    assert_eq!(result.total, 1, "Should group by integer category_id");
    assert_eq!(result.documents.len(), 1);
    assert_eq!(result.documents[0].document.id, "1");

    Ok(())
}

#[tokio::test]
async fn test_distinct_float_field_rounds() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = TempDir::new()?;
    let manager = IndexManager::new(temp_dir.path());

    manager.create_tenant("test")?;

    let settings = IndexSettings {
        custom_ranking: Some(vec!["desc(popularity)".to_string()]),
        attribute_for_distinct: Some("price_bucket".to_string()),
        distinct: Some(DistinctValue::Bool(true)),
        ..Default::default()
    };
    settings.save(temp_dir.path().join("test/settings.json"))?;

    let mut doc1 = create_doc("1", "Laptop Red", "", 100);
    doc1.fields
        .insert("price_bucket".to_string(), FieldValue::Float(99.2));

    let mut doc2 = create_doc("2", "Laptop Blue", "", 90);
    doc2.fields
        .insert("price_bucket".to_string(), FieldValue::Float(99.3));

    let mut doc3 = create_doc("3", "Mouse", "", 50);
    doc3.fields
        .insert("price_bucket".to_string(), FieldValue::Float(50.5));

    manager
        .add_documents_sync("test", vec![doc1, doc2, doc3])
        .await?;

    let result = manager.search_with_facets_and_distinct(
        "test",
        "laptop",
        None,
        None,
        10,
        0,
        None,
        Some(1),
    )?;

    assert_eq!(result.total, 1, "All laptops should be in same group");
    assert_eq!(result.documents.len(), 1);

    Ok(())
}

#[tokio::test]
async fn test_distinct_with_filters() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = TempDir::new()?;
    let manager = IndexManager::new(temp_dir.path());

    manager.create_tenant("test")?;

    let settings = IndexSettings {
        attributes_for_faceting: vec!["category".to_string()],
        searchable_attributes: None,
        ranking: None,
        custom_ranking: Some(vec!["desc(popularity)".to_string()]),
        attributes_to_retrieve: None,
        unretrievable_attributes: None,
        synonyms: None,
        attribute_for_distinct: Some("product_id".to_string()),
        distinct: Some(DistinctValue::Bool(true)),
        ..Default::default()
    };
    settings.save(temp_dir.path().join("test/settings.json"))?;

    let mut doc1 = create_doc("1", "Laptop Red", "laptop-1", 100);
    doc1.fields.insert(
        "category".to_string(),
        FieldValue::Text("electronics".to_string()),
    );

    let mut doc2 = create_doc("2", "Laptop Blue", "laptop-1", 90);
    doc2.fields.insert(
        "category".to_string(),
        FieldValue::Text("electronics".to_string()),
    );

    let mut doc3 = create_doc("3", "Laptop Stand", "stand-1", 50);
    doc3.fields.insert(
        "category".to_string(),
        FieldValue::Text("accessories".to_string()),
    );

    manager
        .add_documents_sync("test", vec![doc1, doc2, doc3])
        .await?;

    use flapjack::types::Filter;
    let filter = Filter::Equals {
        field: "category".to_string(),
        value: FieldValue::Text("electronics".to_string()),
    };

    let result = manager.search_with_facets_and_distinct(
        "test",
        "laptop",
        Some(&filter),
        None,
        10,
        0,
        None,
        Some(1),
    )?;

    assert_eq!(result.total, 1, "Should only count electronics group");
    assert_eq!(result.documents.len(), 1);
    assert_eq!(result.documents[0].document.id, "1");

    Ok(())
}

#[tokio::test]
async fn test_distinct_preserves_ranking() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = TempDir::new()?;
    let manager = IndexManager::new(temp_dir.path());

    manager.create_tenant("test")?;

    let settings = IndexSettings {
        custom_ranking: Some(vec!["desc(popularity)".to_string()]),
        attribute_for_distinct: Some("product_id".to_string()),
        distinct: Some(DistinctValue::Bool(true)),
        ..Default::default()
    };
    settings.save(temp_dir.path().join("test/settings.json"))?;

    let docs = vec![
        create_doc("1", "Laptop Red", "laptop-1", 100),
        create_doc("2", "Laptop Blue", "laptop-1", 90),
        create_doc("3", "Mouse Red", "mouse-1", 200),
        create_doc("4", "Mouse Blue", "mouse-1", 190),
    ];
    manager.add_documents_sync("test", docs).await?;

    let result =
        manager.search_with_facets_and_distinct("test", "red", None, None, 10, 0, None, Some(1))?;

    assert_eq!(
        result.documents.len(),
        2,
        "Should return 2 groups (laptop and mouse)"
    );
    assert_eq!(
        result.documents[0].document.id, "3",
        "Mouse (200) before Laptop (100)"
    );
    assert_eq!(result.documents[1].document.id, "1", "Laptop second");

    Ok(())
}

fn create_doc(id: &str, name: &str, product_id: &str, popularity: i64) -> Document {
    let mut fields = std::collections::HashMap::new();
    fields.insert("name".to_string(), FieldValue::Text(name.to_string()));
    if !product_id.is_empty() {
        fields.insert(
            "product_id".to_string(),
            FieldValue::Text(product_id.to_string()),
        );
    }
    fields.insert("popularity".to_string(), FieldValue::Integer(popularity));
    Document {
        id: id.to_string(),
        fields,
    }
}
