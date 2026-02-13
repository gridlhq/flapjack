use flapjack::Index;
use serde_json::json;
use tempfile::TempDir;

#[test]
fn test_algolia_hierarchical_facets_top_level() {
    let temp_dir = TempDir::new().unwrap();
    let index = Index::create_in_dir(temp_dir.path()).unwrap();

    let docs = vec![
        json!({
            "_id": "1",
            "name": "Laptop",
            "categories": {
                "lvl0": "Electronics",
                "lvl1": "Electronics > Computers",
                "lvl2": "Electronics > Computers > Laptops"
            }
        }),
        json!({
            "_id": "2",
            "name": "Phone",
            "categories": {
                "lvl0": "Electronics",
                "lvl1": "Electronics > Phones"
            }
        }),
        json!({
            "_id": "3",
            "name": "Novel",
            "categories": {
                "lvl0": "Books",
                "lvl1": "Books > Fiction"
            }
        }),
    ];

    index.add_documents_simple(&docs).unwrap();

    let reader = index.reader();
    reader.reload().unwrap();
    let searcher = reader.searcher();

    use tantivy::query::AllQuery;
    let query = Box::new(AllQuery);

    let executor = flapjack::QueryExecutor::new(index.converter(), index.inner().schema());

    let facet_req = flapjack::types::FacetRequest {
        field: "categories.lvl0".to_string(),
        path: "/categories.lvl0".to_string(),
    };

    let result = executor
        .execute_with_facets(
            &searcher,
            query,
            None,
            None,
            10,
            0,
            false,
            Some(&[facet_req]),
        )
        .unwrap();

    assert_eq!(result.documents.len(), 3);
    assert!(result.facets.contains_key("categories.lvl0"));

    let category_facets = &result.facets["categories.lvl0"];
    assert_eq!(
        category_facets.len(),
        2,
        "Should return immediate children only"
    );

    let electronics_facet = category_facets
        .iter()
        .find(|f| f.path == "Electronics")
        .expect("Should have Electronics facet");
    assert_eq!(electronics_facet.count, 2);

    let books_facet = category_facets
        .iter()
        .find(|f| f.path == "Books")
        .expect("Should have Books facet");
    assert_eq!(books_facet.count, 1);
}

#[test]
fn test_facet_drill_down() {
    let temp_dir = TempDir::new().unwrap();
    let index = Index::create_in_dir(temp_dir.path()).unwrap();

    let docs = vec![
        json!({
            "_id": "1",
            "name": "Laptop",
            "categories": {
                "lvl0": "Electronics",
                "lvl1": "Electronics > Computers",
                "lvl2": "Electronics > Computers > Laptops"
            }
        }),
        json!({
            "_id": "2",
            "name": "Phone",
            "categories": {
                "lvl0": "Electronics",
                "lvl1": "Electronics > Phones"
            }
        }),
    ];

    index.add_documents_simple(&docs).unwrap();

    let reader = index.reader();
    reader.reload().unwrap();
    let searcher = reader.searcher();

    use tantivy::query::AllQuery;
    let query = Box::new(AllQuery);

    let executor = flapjack::QueryExecutor::new(index.converter(), index.inner().schema());

    let facet_req = flapjack::types::FacetRequest {
        field: "categories.lvl1".to_string(),
        path: "/categories.lvl1".to_string(),
    };

    let result = executor
        .execute_with_facets(
            &searcher,
            query,
            None,
            None,
            10,
            0,
            false,
            Some(&[facet_req]),
        )
        .unwrap();

    let category_facets = &result.facets["categories.lvl1"];
    assert_eq!(category_facets.len(), 2);

    let computers = category_facets
        .iter()
        .find(|f| f.path == "Electronics > Computers")
        .expect("Should have Electronics > Computers");
    assert_eq!(computers.count, 1);

    let phones = category_facets
        .iter()
        .find(|f| f.path == "Electronics > Phones")
        .expect("Should have Electronics > Phones");
    assert_eq!(phones.count, 1);
}
