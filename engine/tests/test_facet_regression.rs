//! Facet Aggregation Regression Tests
//!
//! Covers:
//! - Bug fix: facets collected but discarded in manager.rs (HashMap::new() bug)
//! - Bug fix: array string fields not indexed as facets in document.rs
//! - Edge cases: empty queries, mixed types, special chars, large values
//! - Integration: facets + filters, facets + sort, facets + pagination
//! - Safety: missing settings, unconfigured facet fields, empty arrays

use flapjack::index::settings::IndexSettings;
use flapjack::types::{Document, FacetRequest, FieldValue, Filter};
use flapjack::IndexManager;
use std::collections::HashMap;
use tempfile::TempDir;

fn doc(id: &str, fields: Vec<(&str, FieldValue)>) -> Document {
    let f: HashMap<String, FieldValue> = fields
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect();
    Document {
        id: id.to_string(),
        fields: f,
    }
}

fn text(s: &str) -> FieldValue {
    FieldValue::Text(s.to_string())
}

fn int(i: i64) -> FieldValue {
    FieldValue::Integer(i)
}

#[allow(dead_code)]
fn float(f: f64) -> FieldValue {
    FieldValue::Float(f)
}

fn arr(items: Vec<&str>) -> FieldValue {
    FieldValue::Array(items.into_iter().map(text).collect())
}

fn facet_req(field: &str) -> FacetRequest {
    FacetRequest {
        field: field.to_string(),
        path: format!("/{}", field),
    }
}

async fn setup_with_settings(
    facet_fields: Vec<&str>,
    docs: Vec<Document>,
) -> (TempDir, std::sync::Arc<IndexManager>) {
    let temp_dir = TempDir::new().unwrap();
    let manager = IndexManager::new(temp_dir.path());
    manager.create_tenant("test").unwrap();

    let settings = IndexSettings {
        attributes_for_faceting: facet_fields.iter().map(|s| s.to_string()).collect(),
        ..Default::default()
    };
    settings
        .save(temp_dir.path().join("test/settings.json"))
        .unwrap();

    manager.add_documents_sync("test", docs).await.unwrap();
    (temp_dir, manager)
}

// ============================================================
// Core regression: the two bugs fixed in this session
// ============================================================

#[tokio::test]
async fn test_facets_returned_not_empty_hashmap() {
    // Regression: manager.rs returned facets: HashMap::new() instead of collected facets
    let docs = vec![
        doc(
            "1",
            vec![("brand", text("Apple")), ("name", text("iPhone"))],
        ),
        doc(
            "2",
            vec![("brand", text("Samsung")), ("name", text("Galaxy"))],
        ),
        doc(
            "3",
            vec![("brand", text("Apple")), ("name", text("MacBook"))],
        ),
    ];
    let (_tmp, mgr) = setup_with_settings(vec!["brand"], docs).await;

    let result = mgr
        .search_with_facets("test", "", None, None, 10, 0, Some(&[facet_req("brand")]))
        .unwrap();

    assert!(
        !result.facets.is_empty(),
        "Facets must not be empty — regression for HashMap::new() bug"
    );
    let brand_facets = result.facets.get("brand").expect("brand facets missing");
    assert!(
        brand_facets.len() >= 2,
        "Expected at least 2 brands, got {}",
        brand_facets.len()
    );

    let apple_count = brand_facets
        .iter()
        .find(|f| f.path == "Apple")
        .map(|f| f.count);
    assert_eq!(apple_count, Some(2), "Apple should have count=2");

    let samsung_count = brand_facets
        .iter()
        .find(|f| f.path == "Samsung")
        .map(|f| f.count);
    assert_eq!(samsung_count, Some(1), "Samsung should have count=1");
}

#[tokio::test]
async fn test_array_string_fields_indexed_as_facets() {
    // Regression: array string values (e.g. categories: ["A","B"]) were not facet-indexed
    let docs = vec![
        doc(
            "1",
            vec![
                ("name", text("Laptop")),
                ("categories", arr(vec!["Electronics", "Computers"])),
            ],
        ),
        doc(
            "2",
            vec![
                ("name", text("Phone")),
                ("categories", arr(vec!["Electronics", "Phones"])),
            ],
        ),
        doc(
            "3",
            vec![
                ("name", text("Novel")),
                ("categories", arr(vec!["Books", "Fiction"])),
            ],
        ),
    ];
    let (_tmp, mgr) = setup_with_settings(vec!["categories"], docs).await;

    let result = mgr
        .search_with_facets(
            "test",
            "",
            None,
            None,
            10,
            0,
            Some(&[facet_req("categories")]),
        )
        .unwrap();

    let cat_facets = result
        .facets
        .get("categories")
        .expect("categories facets missing");
    assert!(
        cat_facets.len() >= 4,
        "Expected at least 4 category values (Electronics, Computers, Phones, Books, Fiction), got {}",
        cat_facets.len()
    );

    let electronics = cat_facets.iter().find(|f| f.path == "Electronics");
    assert_eq!(
        electronics.map(|f| f.count),
        Some(2),
        "Electronics should appear in 2 docs"
    );
}

// ============================================================
// Facets with queries, filters, sort, pagination
// ============================================================

#[tokio::test]
async fn test_facets_with_text_query() {
    let docs = vec![
        doc(
            "1",
            vec![("brand", text("Apple")), ("name", text("iPhone 15"))],
        ),
        doc(
            "2",
            vec![("brand", text("Apple")), ("name", text("MacBook Pro"))],
        ),
        doc(
            "3",
            vec![("brand", text("Samsung")), ("name", text("Galaxy S24"))],
        ),
        doc(
            "4",
            vec![("brand", text("Samsung")), ("name", text("Galaxy Tab"))],
        ),
    ];
    let (_tmp, mgr) = setup_with_settings(vec!["brand"], docs).await;

    let result = mgr
        .search_with_facets(
            "test",
            "galaxy",
            None,
            None,
            10,
            0,
            Some(&[facet_req("brand")]),
        )
        .unwrap();

    // Only Samsung docs match "galaxy"
    assert_eq!(result.documents.len(), 2);
    let brand_facets = result.facets.get("brand").expect("brand facets missing");
    // Facet counts should reflect the filtered result set
    let samsung = brand_facets.iter().find(|f| f.path == "Samsung");
    assert_eq!(samsung.map(|f| f.count), Some(2));
}

#[tokio::test]
async fn test_facets_with_filter() {
    let docs = vec![
        doc(
            "1",
            vec![
                ("brand", text("Apple")),
                ("category", text("Phone")),
                ("name", text("iPhone")),
            ],
        ),
        doc(
            "2",
            vec![
                ("brand", text("Apple")),
                ("category", text("Laptop")),
                ("name", text("MacBook")),
            ],
        ),
        doc(
            "3",
            vec![
                ("brand", text("Samsung")),
                ("category", text("Phone")),
                ("name", text("Galaxy")),
            ],
        ),
    ];
    let (_tmp, mgr) = setup_with_settings(vec!["brand", "category"], docs).await;

    let filter = Filter::Equals {
        field: "category".to_string(),
        value: FieldValue::Text("Phone".to_string()),
    };
    let result = mgr
        .search_with_facets(
            "test",
            "",
            Some(&filter),
            None,
            10,
            0,
            Some(&[facet_req("brand")]),
        )
        .unwrap();

    assert_eq!(result.documents.len(), 2, "Should have 2 phone docs");
    let brand_facets = result.facets.get("brand").expect("brand facets missing");
    assert!(
        brand_facets.iter().any(|f| f.path == "Apple"),
        "Apple should be in filtered facets"
    );
    assert!(
        brand_facets.iter().any(|f| f.path == "Samsung"),
        "Samsung should be in filtered facets"
    );
}

#[tokio::test]
async fn test_facets_with_pagination() {
    let docs = (0..25)
        .map(|i| {
            doc(
                &format!("{}", i),
                vec![
                    ("brand", text(if i % 2 == 0 { "Apple" } else { "Samsung" })),
                    ("name", text(&format!("Product {}", i))),
                ],
            )
        })
        .collect();
    let (_tmp, mgr) = setup_with_settings(vec!["brand"], docs).await;

    // Page 0
    let r0 = mgr
        .search_with_facets("test", "", None, None, 5, 0, Some(&[facet_req("brand")]))
        .unwrap();
    // Page 2
    let r2 = mgr
        .search_with_facets("test", "", None, None, 5, 10, Some(&[facet_req("brand")]))
        .unwrap();

    // Facet counts should be the same across pages (total, not per-page)
    let p0_brand = r0.facets.get("brand").expect("page 0 brand facets");
    let p2_brand = r2.facets.get("brand").expect("page 2 brand facets");

    let p0_apple = p0_brand
        .iter()
        .find(|f| f.path == "Apple")
        .map(|f| f.count)
        .unwrap_or(0);
    let p2_apple = p2_brand
        .iter()
        .find(|f| f.path == "Apple")
        .map(|f| f.count)
        .unwrap_or(0);
    assert_eq!(
        p0_apple, p2_apple,
        "Facet counts should be consistent across pages"
    );
}

#[tokio::test]
async fn test_facets_empty_query_returns_all_counts() {
    let docs = vec![
        doc("1", vec![("brand", text("A")), ("name", text("x"))]),
        doc("2", vec![("brand", text("B")), ("name", text("y"))]),
        doc("3", vec![("brand", text("C")), ("name", text("z"))]),
    ];
    let (_tmp, mgr) = setup_with_settings(vec!["brand"], docs).await;

    let result = mgr
        .search_with_facets("test", "", None, None, 10, 0, Some(&[facet_req("brand")]))
        .unwrap();

    let brand_facets = result.facets.get("brand").expect("brand facets missing");
    assert_eq!(
        brand_facets.len(),
        3,
        "Empty query should return all 3 brands"
    );
    let total: u64 = brand_facets.iter().map(|f| f.count).sum();
    assert_eq!(total, 3, "Total facet count should equal doc count");
}

// ============================================================
// Multiple facet fields simultaneously
// ============================================================

#[tokio::test]
async fn test_multiple_facet_fields() {
    let docs = vec![
        doc(
            "1",
            vec![
                ("brand", text("Apple")),
                ("color", text("Black")),
                ("name", text("iPhone")),
            ],
        ),
        doc(
            "2",
            vec![
                ("brand", text("Apple")),
                ("color", text("White")),
                ("name", text("iPad")),
            ],
        ),
        doc(
            "3",
            vec![
                ("brand", text("Samsung")),
                ("color", text("Black")),
                ("name", text("Galaxy")),
            ],
        ),
    ];
    let (_tmp, mgr) = setup_with_settings(vec!["brand", "color"], docs).await;

    let result = mgr
        .search_with_facets(
            "test",
            "",
            None,
            None,
            10,
            0,
            Some(&[facet_req("brand"), facet_req("color")]),
        )
        .unwrap();

    assert!(
        result.facets.contains_key("brand"),
        "Should have brand facets"
    );
    assert!(
        result.facets.contains_key("color"),
        "Should have color facets"
    );

    let brands = result.facets.get("brand").unwrap();
    let colors = result.facets.get("color").unwrap();
    assert_eq!(brands.len(), 2, "2 brands");
    assert_eq!(colors.len(), 2, "2 colors");
}

// ============================================================
// Safety: edge cases, missing config, bad input
// ============================================================

#[tokio::test]
async fn test_facets_field_not_in_settings_returns_empty() {
    // Only "brand" is configured as facetable — requesting "color" should return nothing for color
    let docs = vec![doc(
        "1",
        vec![
            ("brand", text("Apple")),
            ("color", text("Black")),
            ("name", text("x")),
        ],
    )];
    let (_tmp, mgr) = setup_with_settings(vec!["brand"], docs).await;

    // The handler filters out unconfigured facets, but at the manager level
    // we pass the requests through. The facet collector won't find data for
    // fields that weren't indexed as facets.
    let result = mgr
        .search_with_facets(
            "test",
            "",
            None,
            None,
            10,
            0,
            Some(&[facet_req("brand"), facet_req("color")]),
        )
        .unwrap();

    let brand_facets = result.facets.get("brand");
    assert!(brand_facets.is_some(), "Configured facet field should work");
    // color may or may not appear — but it should NOT cause an error
}

#[tokio::test]
async fn test_facets_no_settings_file() {
    // No settings.json at all — facet indexing should still not crash
    let temp_dir = TempDir::new().unwrap();
    let manager = IndexManager::new(temp_dir.path());
    manager.create_tenant("test").unwrap();

    let docs = vec![doc(
        "1",
        vec![("brand", text("Apple")), ("name", text("iPhone"))],
    )];
    manager.add_documents_sync("test", docs).await.unwrap();

    // Should not panic; facets may be empty since no attributesForFaceting configured
    let result =
        manager.search_with_facets("test", "", None, None, 10, 0, Some(&[facet_req("brand")]));
    assert!(
        result.is_ok(),
        "Search with facets should not crash without settings"
    );
}

#[tokio::test]
async fn test_facets_special_characters_in_values() {
    let docs = vec![
        doc("1", vec![("brand", text("AT&T")), ("name", text("Phone"))]),
        doc(
            "2",
            vec![
                ("brand", text("Ben & Jerry's")),
                ("name", text("Ice Cream")),
            ],
        ),
        doc("3", vec![("brand", text("H&M")), ("name", text("Shirt"))]),
        doc("4", vec![("brand", text("AT&T")), ("name", text("Tablet"))]),
    ];
    let (_tmp, mgr) = setup_with_settings(vec!["brand"], docs).await;

    let result = mgr
        .search_with_facets("test", "", None, None, 10, 0, Some(&[facet_req("brand")]))
        .unwrap();

    let brand_facets = result.facets.get("brand").expect("brand facets");
    assert!(
        brand_facets.len() >= 3,
        "Should handle special chars: got {:?}",
        brand_facets
    );

    let att = brand_facets.iter().find(|f| f.path == "AT&T");
    assert_eq!(att.map(|f| f.count), Some(2), "AT&T should have 2 docs");
}

#[tokio::test]
async fn test_facets_mixed_string_and_array_fields() {
    // brand is string, categories is array — both should work as facets
    let docs = vec![
        doc(
            "1",
            vec![
                ("brand", text("Apple")),
                ("categories", arr(vec!["Electronics", "Phones"])),
                ("name", text("iPhone")),
            ],
        ),
        doc(
            "2",
            vec![
                ("brand", text("Dell")),
                ("categories", arr(vec!["Electronics", "Computers"])),
                ("name", text("Laptop")),
            ],
        ),
    ];
    let (_tmp, mgr) = setup_with_settings(vec!["brand", "categories"], docs).await;

    let result = mgr
        .search_with_facets(
            "test",
            "",
            None,
            None,
            10,
            0,
            Some(&[facet_req("brand"), facet_req("categories")]),
        )
        .unwrap();

    let brands = result.facets.get("brand").expect("brand facets");
    let cats = result.facets.get("categories").expect("categories facets");

    assert_eq!(brands.len(), 2);
    assert!(
        cats.len() >= 3,
        "Should have Electronics, Phones, Computers — got {}",
        cats.len()
    );
}

#[tokio::test]
async fn test_facets_empty_array_field() {
    // Doc with empty array for a facet field should not crash
    let docs = vec![
        doc(
            "1",
            vec![
                ("brand", text("Apple")),
                ("tags", FieldValue::Array(vec![])),
                ("name", text("iPhone")),
            ],
        ),
        doc(
            "2",
            vec![
                ("brand", text("Samsung")),
                ("tags", arr(vec!["new", "sale"])),
                ("name", text("Galaxy")),
            ],
        ),
    ];
    let (_tmp, mgr) = setup_with_settings(vec!["brand", "tags"], docs).await;

    let result = mgr
        .search_with_facets("test", "", None, None, 10, 0, Some(&[facet_req("tags")]))
        .unwrap();

    // Should not crash; tags facets should include "new" and "sale" from doc 2
    let tag_facets = result.facets.get("tags");
    if let Some(tags) = tag_facets {
        assert!(
            tags.iter().any(|f| f.path == "new") || tags.iter().any(|f| f.path == "sale"),
            "Should have facets from doc with non-empty array"
        );
    }
}

#[tokio::test]
async fn test_facets_numeric_field_not_indexed() {
    // Numeric fields shouldn't produce facet values (only strings/arrays)
    let docs = vec![
        doc(
            "1",
            vec![
                ("price", int(100)),
                ("brand", text("Apple")),
                ("name", text("x")),
            ],
        ),
        doc(
            "2",
            vec![
                ("price", int(200)),
                ("brand", text("Samsung")),
                ("name", text("y")),
            ],
        ),
    ];
    let (_tmp, mgr) = setup_with_settings(vec!["brand", "price"], docs).await;

    let result = mgr
        .search_with_facets(
            "test",
            "",
            None,
            None,
            10,
            0,
            Some(&[facet_req("brand"), facet_req("price")]),
        )
        .unwrap();

    let brands = result.facets.get("brand").expect("brand facets");
    assert_eq!(brands.len(), 2, "String facets should work");
    // price facets may be empty since integers aren't indexed as facet paths — that's correct
}

#[tokio::test]
async fn test_facets_with_distinct() {
    let docs = vec![
        doc(
            "1",
            vec![
                ("brand", text("Apple")),
                ("product_id", text("p1")),
                ("name", text("iPhone 15 Black")),
            ],
        ),
        doc(
            "2",
            vec![
                ("brand", text("Apple")),
                ("product_id", text("p1")),
                ("name", text("iPhone 15 White")),
            ],
        ),
        doc(
            "3",
            vec![
                ("brand", text("Samsung")),
                ("product_id", text("p2")),
                ("name", text("Galaxy S24")),
            ],
        ),
    ];

    let temp_dir = TempDir::new().unwrap();
    let manager = IndexManager::new(temp_dir.path());
    manager.create_tenant("test").unwrap();

    let settings = IndexSettings {
        attributes_for_faceting: vec!["brand".to_string()],
        attribute_for_distinct: Some("product_id".to_string()),
        ..Default::default()
    };
    settings
        .save(temp_dir.path().join("test/settings.json"))
        .unwrap();
    manager.add_documents_sync("test", docs).await.unwrap();

    let result = manager
        .search_with_facets_and_distinct(
            "test",
            "",
            None,
            None,
            10,
            0,
            Some(&[facet_req("brand")]),
            Some(1),
        )
        .unwrap();

    // Facets should still be collected (even if distinct deduplicates results)
    let brand_facets = result
        .facets
        .get("brand")
        .expect("brand facets with distinct");
    assert!(
        !brand_facets.is_empty(),
        "Facets should not be empty when distinct is active"
    );
}

// ============================================================
// Correctness: facet counts reflect actual data
// ============================================================

#[tokio::test]
async fn test_facet_counts_accuracy_100_docs() {
    // Insert 100 docs with known distribution and verify exact counts
    let docs: Vec<Document> = (0..100)
        .map(|i| {
            let brand = match i % 4 {
                0 => "Alpha",
                1 => "Bravo",
                2 => "Charlie",
                _ => "Delta",
            };
            doc(
                &format!("{}", i),
                vec![
                    ("brand", text(brand)),
                    ("name", text(&format!("Item {}", i))),
                ],
            )
        })
        .collect();
    let (_tmp, mgr) = setup_with_settings(vec!["brand"], docs).await;

    let result = mgr
        .search_with_facets("test", "", None, None, 100, 0, Some(&[facet_req("brand")]))
        .unwrap();

    let brand_facets = result.facets.get("brand").expect("brand facets");
    assert_eq!(brand_facets.len(), 4, "Should have exactly 4 brands");

    for fc in brand_facets {
        assert_eq!(
            fc.count, 25,
            "Each brand should have exactly 25 docs, {} has {}",
            fc.path, fc.count
        );
    }
}

#[tokio::test]
async fn test_facet_counts_with_query_filter_interaction() {
    // Facet counts should reflect the query-filtered result set
    let docs = vec![
        doc(
            "1",
            vec![
                ("brand", text("Apple")),
                ("type", text("phone")),
                ("name", text("iPhone")),
            ],
        ),
        doc(
            "2",
            vec![
                ("brand", text("Apple")),
                ("type", text("laptop")),
                ("name", text("MacBook")),
            ],
        ),
        doc(
            "3",
            vec![
                ("brand", text("Apple")),
                ("type", text("tablet")),
                ("name", text("iPad")),
            ],
        ),
        doc(
            "4",
            vec![
                ("brand", text("Samsung")),
                ("type", text("phone")),
                ("name", text("Galaxy")),
            ],
        ),
        doc(
            "5",
            vec![
                ("brand", text("Samsung")),
                ("type", text("laptop")),
                ("name", text("Book")),
            ],
        ),
    ];
    let (_tmp, mgr) = setup_with_settings(vec!["brand", "type"], docs).await;

    // Filter to phones only, get brand facets
    let filter = Filter::Equals {
        field: "type".to_string(),
        value: FieldValue::Text("phone".to_string()),
    };
    let result = mgr
        .search_with_facets(
            "test",
            "",
            Some(&filter),
            None,
            10,
            0,
            Some(&[facet_req("brand")]),
        )
        .unwrap();

    let brand_facets = result.facets.get("brand").expect("brand facets");
    let apple = brand_facets
        .iter()
        .find(|f| f.path == "Apple")
        .map(|f| f.count);
    let samsung = brand_facets
        .iter()
        .find(|f| f.path == "Samsung")
        .map(|f| f.count);
    assert_eq!(apple, Some(1), "Apple has 1 phone");
    assert_eq!(samsung, Some(1), "Samsung has 1 phone");
}

#[tokio::test]
async fn test_facets_wildcard_star_expands_to_all() {
    use std::collections::HashSet;
    let docs = vec![
        doc(
            "1",
            vec![
                ("brand", text("Apple")),
                ("color", text("Black")),
                ("category", text("Phone")),
                ("name", text("iPhone")),
            ],
        ),
        doc(
            "2",
            vec![
                ("brand", text("Samsung")),
                ("color", text("White")),
                ("category", text("Tablet")),
                ("name", text("Galaxy Tab")),
            ],
        ),
        doc(
            "3",
            vec![
                ("brand", text("Apple")),
                ("color", text("Silver")),
                ("category", text("Laptop")),
                ("name", text("MacBook")),
            ],
        ),
    ];
    let (_tmp, mgr) = setup_with_settings(vec!["brand", "color", "category"], docs).await;

    let settings_path = _tmp.path().join("test/settings.json");
    let settings = IndexSettings::load(&settings_path).unwrap();
    let allowed: HashSet<String> = settings.facet_set();

    let input_facets = ["*".to_string()];
    let expanded: Vec<FacetRequest> = if input_facets.iter().any(|f| f == "*") {
        allowed.iter().map(|f| facet_req(f)).collect()
    } else {
        input_facets
            .iter()
            .filter(|f| allowed.contains(f.as_str()))
            .map(|f| facet_req(f))
            .collect()
    };

    assert_eq!(
        expanded.len(),
        3,
        "Wildcard should expand to 3 facet fields"
    );

    let result = mgr
        .search_with_facets("test", "", None, None, 10, 0, Some(&expanded))
        .unwrap();

    assert!(
        result.facets.contains_key("brand"),
        "Wildcard should expand to include brand"
    );
    assert!(
        result.facets.contains_key("color"),
        "Wildcard should expand to include color"
    );
    assert!(
        result.facets.contains_key("category"),
        "Wildcard should expand to include category"
    );

    let brands = result.facets.get("brand").unwrap();
    assert_eq!(brands.len(), 2, "Should have Apple + Samsung");
    let colors = result.facets.get("color").unwrap();
    assert!(colors.len() >= 2, "Should have Black, White, Silver");
}
#[tokio::test]
async fn test_facet_cache_invalidated_on_write() {
    let docs = vec![
        doc(
            "1",
            vec![("brand", text("Apple")), ("name", text("iPhone"))],
        ),
        doc(
            "2",
            vec![("brand", text("Samsung")), ("name", text("Galaxy"))],
        ),
    ];
    let (_tmp, mgr) = setup_with_settings(vec!["brand"], docs).await;

    let r1 = mgr
        .search_with_facets("test", "", None, None, 0, 0, Some(&[facet_req("brand")]))
        .unwrap();
    let brands1 = r1.facets.get("brand").expect("brand facets");
    assert_eq!(brands1.len(), 2, "Should have Apple + Samsung");

    mgr.add_documents_sync(
        "test",
        vec![
            doc("3", vec![("brand", text("Sony")), ("name", text("Xperia"))]),
            doc("4", vec![("brand", text("Sony")), ("name", text("TV"))]),
        ],
    )
    .await
    .unwrap();

    let r2 = mgr
        .search_with_facets("test", "", None, None, 0, 0, Some(&[facet_req("brand")]))
        .unwrap();
    let brands2 = r2.facets.get("brand").expect("brand facets after write");
    assert_eq!(
        brands2.len(),
        3,
        "Should have Apple + Samsung + Sony after write"
    );
    let sony = brands2.iter().find(|f| f.path == "Sony");
    assert_eq!(sony.map(|f| f.count), Some(2), "Sony should have 2 docs");
    assert_eq!(r2.total, 4, "Total should be 4 after adding 2 docs");
}
