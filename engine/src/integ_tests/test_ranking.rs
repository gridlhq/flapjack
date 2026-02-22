//! Consolidated ranking integration tests moved inline from engine/tests/test_ranking.rs.
//!
//! Merged from (all deleted):
//!   - test_sort.rs        (explicit sort by field: asc, desc, missing values, nested, float)
//!   - test_distinct.rs    (attribute_for_distinct: dedup, top-N, disabled, filters, ranking)
//!   - test_custom_ranking.rs (custom_ranking setting: desc, asc, multiple, missing values)

use crate::index::settings::{DistinctValue, IndexSettings};
use crate::types::{Document, FieldValue, Filter, Sort, SortOrder};
use crate::IndexManager;
use serde_json::json;
use std::sync::Arc;
use tempfile::TempDir;

// ============================================================
// Helpers (from test_sort.rs)
// ============================================================

struct SortFixture {
    _tmp: TempDir,
    mgr: Arc<IndexManager>,
}

static PRICE_FIXTURE: tokio::sync::OnceCell<SortFixture> = tokio::sync::OnceCell::const_new();

async fn get_price_fixture() -> &'static SortFixture {
    PRICE_FIXTURE.get_or_init(|| async {
        let tmp = TempDir::new().unwrap();
        let mgr = IndexManager::new(tmp.path());
        mgr.create_tenant("test").unwrap();

        let docs: Vec<Document> = vec![
            json!({"_id": "1", "objectID": "1", "title": "Cheap Widget", "price": 10, "brand": "Acme"}),
            json!({"_id": "2", "objectID": "2", "title": "Mid Widget", "price": 50, "brand": "Acme"}),
            json!({"_id": "3", "objectID": "3", "title": "Expensive Widget", "price": 100, "brand": "Luxe"}),
            json!({"_id": "4", "objectID": "4", "title": "Budget Item", "price": 5, "brand": "Value"}),
            json!({"_id": "5", "objectID": "5", "title": "Premium Item", "price": 200, "brand": "Luxe"}),
        ].into_iter().map(|v| Document::from_json(&v).unwrap()).collect();
        mgr.add_documents_sync("test", docs).await.unwrap();

        SortFixture { _tmp: tmp, mgr }
    }).await
}

fn sort_asc(field: &str) -> Sort {
    Sort::ByField {
        field: field.to_string(),
        order: SortOrder::Asc,
    }
}

fn sort_desc(field: &str) -> Sort {
    Sort::ByField {
        field: field.to_string(),
        order: SortOrder::Desc,
    }
}

fn get_field_i64(doc: &Document, field: &str) -> Option<i64> {
    doc.fields.get(field).and_then(|v| match v {
        crate::types::FieldValue::Integer(n) => Some(*n),
        _ => None,
    })
}

fn get_field_str<'a>(doc: &'a Document, field: &str) -> Option<&'a str> {
    doc.fields.get(field).and_then(|v| v.as_text())
}

// Helper (from test_distinct.rs)
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

// ============================================================
// From test_sort.rs — explicit sort-by-field tests
// ============================================================

#[tokio::test]
async fn test_sort_price_asc() {
    let f = get_price_fixture().await;
    let sort = sort_asc("price");
    let results = f.mgr.search("test", "", None, Some(&sort), 100).unwrap();
    let prices: Vec<i64> = results
        .documents
        .iter()
        .filter_map(|d| get_field_i64(&d.document, "price"))
        .collect();
    assert_eq!(prices, vec![5, 10, 50, 100, 200]);
}

#[tokio::test]
async fn test_sort_price_desc() {
    let f = get_price_fixture().await;
    let sort = sort_desc("price");
    let results = f.mgr.search("test", "", None, Some(&sort), 100).unwrap();
    let prices: Vec<i64> = results
        .documents
        .iter()
        .filter_map(|d| get_field_i64(&d.document, "price"))
        .collect();
    assert_eq!(prices, vec![200, 100, 50, 10, 5]);
}

#[tokio::test]
async fn test_sort_with_text_query() {
    let f = get_price_fixture().await;
    let sort = sort_asc("price");
    let results = f
        .mgr
        .search("test", "widget", None, Some(&sort), 100)
        .unwrap();
    assert_eq!(results.documents.len(), 3, "Should match 3 widgets");
    let prices: Vec<i64> = results
        .documents
        .iter()
        .filter_map(|d| get_field_i64(&d.document, "price"))
        .collect();
    assert_eq!(prices, vec![10, 50, 100]);
}

#[tokio::test]
async fn test_sort_with_numeric_filter() {
    let f = get_price_fixture().await;
    let sort = sort_desc("price");
    let filter = Filter::GreaterThanOrEqual {
        field: "price".into(),
        value: FieldValue::Integer(50),
    };
    let results = f
        .mgr
        .search("test", "", Some(&filter), Some(&sort), 100)
        .unwrap();
    assert_eq!(results.documents.len(), 3);
    let prices: Vec<i64> = results
        .documents
        .iter()
        .filter_map(|d| get_field_i64(&d.document, "price"))
        .collect();
    assert_eq!(prices, vec![200, 100, 50]);
}

#[tokio::test]
async fn test_sort_string_field() {
    let f = get_price_fixture().await;
    let sort = sort_asc("title");
    let results = f.mgr.search("test", "", None, Some(&sort), 100).unwrap();
    let titles: Vec<&str> = results
        .documents
        .iter()
        .filter_map(|d| get_field_str(&d.document, "title"))
        .collect();
    assert_eq!(titles[0], "Budget Item");
    assert_eq!(titles[1], "Cheap Widget");
}

#[tokio::test]
async fn test_sort_missing_field_handled() {
    let tmp = TempDir::new().unwrap();
    let mgr = IndexManager::new(tmp.path());
    mgr.create_tenant("test").unwrap();

    let docs: Vec<Document> = vec![
        json!({"_id": "1", "objectID": "1", "title": "Has Price", "price": 50}),
        json!({"_id": "2", "objectID": "2", "title": "No Price"}),
        json!({"_id": "3", "objectID": "3", "title": "Also Has Price", "price": 10}),
    ]
    .into_iter()
    .map(|v| Document::from_json(&v).unwrap())
    .collect();
    mgr.add_documents_sync("test", docs).await.unwrap();

    let sort = sort_asc("price");
    let results = mgr.search("test", "", None, Some(&sort), 100).unwrap();
    assert_eq!(results.documents.len(), 3);
    let first_title = get_field_str(&results.documents[0].document, "title").unwrap();
    assert_eq!(first_title, "No Price", "Missing values sort first in asc");
}

#[tokio::test]
async fn test_sort_nested_field() {
    let tmp = TempDir::new().unwrap();
    let mgr = IndexManager::new(tmp.path());
    mgr.create_tenant("test").unwrap();

    let docs: Vec<Document> = vec![
        json!({"_id": "1", "objectID": "1", "meta": {"score": 75}}),
        json!({"_id": "2", "objectID": "2", "meta": {"score": 25}}),
        json!({"_id": "3", "objectID": "3", "meta": {"score": 100}}),
    ]
    .into_iter()
    .map(|v| Document::from_json(&v).unwrap())
    .collect();
    mgr.add_documents_sync("test", docs).await.unwrap();

    let sort = sort_desc("meta.score");
    let results = mgr.search("test", "", None, Some(&sort), 100).unwrap();
    let ids: Vec<&str> = results
        .documents
        .iter()
        .map(|d| d.document.id.as_str())
        .collect();
    assert_eq!(ids, vec!["3", "1", "2"]);
}

#[tokio::test]
async fn test_sort_float_field() {
    let tmp = TempDir::new().unwrap();
    let mgr = IndexManager::new(tmp.path());
    mgr.create_tenant("test").unwrap();

    let docs: Vec<Document> = vec![
        json!({"_id": "1", "objectID": "1", "rating": 4.5}),
        json!({"_id": "2", "objectID": "2", "rating": 3.2}),
        json!({"_id": "3", "objectID": "3", "rating": 4.9}),
    ]
    .into_iter()
    .map(|v| Document::from_json(&v).unwrap())
    .collect();
    mgr.add_documents_sync("test", docs).await.unwrap();

    let sort = sort_desc("rating");
    let results = mgr.search("test", "", None, Some(&sort), 100).unwrap();
    let ids: Vec<&str> = results
        .documents
        .iter()
        .map(|d| d.document.id.as_str())
        .collect();
    assert_eq!(ids, vec!["3", "1", "2"]);
}

// ============================================================
// From test_distinct.rs — attribute_for_distinct tests
// ============================================================

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

// ============================================================
// From test_custom_ranking.rs — custom_ranking setting tests
// ============================================================

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
        .map(|v| crate::types::Document::from_json(&v).unwrap())
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
        .map(|v| crate::types::Document::from_json(&v).unwrap())
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
        .map(|v| crate::types::Document::from_json(&v).unwrap())
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
        .map(|v| crate::types::Document::from_json(&v).unwrap())
        .collect();
    manager.add_documents_sync("test", docs).await.unwrap();

    let results = manager.search("test", "product", None, None, 10).unwrap();

    assert_eq!(results.documents.len(), 3);
    assert_eq!(results.documents[0].document.id, "3");
    assert_eq!(results.documents[1].document.id, "1");
    assert_eq!(results.documents[2].document.id, "2");
}
