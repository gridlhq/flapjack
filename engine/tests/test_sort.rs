use flapjack::types::{Document, Sort, SortOrder};
use flapjack::IndexManager;
use flapjack_http::filter_parser::parse_filter;
use serde_json::json;
use std::sync::Arc;
use tempfile::TempDir;

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
        flapjack::types::FieldValue::Integer(n) => Some(*n),
        _ => None,
    })
}

fn get_field_str<'a>(doc: &'a Document, field: &str) -> Option<&'a str> {
    doc.fields.get(field).and_then(|v| v.as_text())
}

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
    let filter = parse_filter("price >= 50").unwrap();
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
