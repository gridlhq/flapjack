use flapjack::index::settings::IndexSettings;
use flapjack::types::{Document, FacetRequest, FieldValue};
use flapjack::IndexManager;
use std::collections::HashMap;
use std::sync::atomic::Ordering;
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

async fn setup_with_docs(count: usize) -> (TempDir, std::sync::Arc<IndexManager>) {
    let temp_dir = TempDir::new().unwrap();
    let manager = IndexManager::new(temp_dir.path());
    manager.create_tenant("t1").unwrap();

    let settings_path = temp_dir.path().join("t1").join("settings.json");
    let settings = IndexSettings {
        attributes_for_faceting: vec!["category".to_string(), "brand".to_string()],
        ..IndexSettings::default()
    };
    settings.save(&settings_path).unwrap();
    manager.invalidate_settings_cache("t1");

    let docs: Vec<Document> = (0..count)
        .map(|i| {
            doc(
                &format!("doc{}", i),
                vec![
                    ("name", text(&format!("product {}", i))),
                    ("category", text(&format!("cat{}", i % 10))),
                    ("brand", text(&format!("brand{}", i % 5))),
                ],
            )
        })
        .collect();

    manager.add_documents_sync("t1", docs).await.unwrap();
    (temp_dir, manager)
}

#[tokio::test]
async fn test_searchable_paths_warm_on_load() {
    let (_temp_dir, manager) = setup_with_docs(50).await;

    manager.unload(&"t1".to_string()).unwrap();

    let _index = manager.get_or_load("t1").unwrap();

    let index2 = manager.get_or_load("t1").unwrap();
    let t0 = std::time::Instant::now();
    let paths = index2.searchable_paths();
    let paths_time = t0.elapsed();

    assert!(!paths.is_empty(), "should have searchable paths");
    assert!(
        paths_time.as_micros() < 1000,
        "searchable_paths after get_or_load should be cached (<1ms), got {:?}",
        paths_time
    );
}

#[tokio::test]
async fn test_searchable_paths_warm_on_create_existing() {
    let (temp_dir, manager) = setup_with_docs(50).await;

    manager.unload(&"t1".to_string()).unwrap();

    let manager2 = IndexManager::new(temp_dir.path());
    manager2.create_tenant("t1").unwrap();

    let index = manager2.get_or_load("t1").unwrap();
    let t0 = std::time::Instant::now();
    let paths = index.searchable_paths();
    let paths_time = t0.elapsed();

    assert!(!paths.is_empty());
    assert!(
        paths_time.as_micros() < 1000,
        "searchable_paths after create_tenant(existing) should be cached (<1ms), got {:?}",
        paths_time
    );
}

async fn setup_with_cap(doc_count: usize, cap: usize) -> (TempDir, std::sync::Arc<IndexManager>) {
    let temp_dir = TempDir::new().unwrap();
    let manager = IndexManager::new(temp_dir.path());
    manager
        .facet_cache_cap
        .store(cap, std::sync::atomic::Ordering::Relaxed);
    manager.create_tenant("t1").unwrap();

    let settings_path = temp_dir.path().join("t1").join("settings.json");
    let settings = IndexSettings {
        attributes_for_faceting: vec!["category".to_string(), "brand".to_string()],
        ..IndexSettings::default()
    };
    settings.save(&settings_path).unwrap();
    manager.invalidate_settings_cache("t1");

    let docs: Vec<Document> = (0..doc_count)
        .map(|i| {
            doc(
                &format!("doc{}", i),
                vec![
                    ("name", text(&format!("product {}", i))),
                    ("category", text(&format!("cat{}", i % 5))),
                    ("brand", text(&format!("brand{}", i % 3))),
                ],
            )
        })
        .collect();

    manager.add_documents_sync("t1", docs).await.unwrap();
    (temp_dir, manager)
}

#[tokio::test]
async fn test_facet_cache_bounded_by_cap() {
    let temp_dir = TempDir::new().unwrap();
    let manager = IndexManager::new(temp_dir.path());
    manager.facet_cache_cap.store(15, Ordering::Relaxed);

    for i in 0..20 {
        manager.facet_cache.insert(
            format!("t1:q{}:category", i),
            std::sync::Arc::new((std::time::Instant::now(), 0, HashMap::new())),
        );
    }
    assert_eq!(
        manager.facet_cache.len(),
        20,
        "no eviction yet, just raw inserts"
    );

    let cap = manager.facet_cache_cap.load(Ordering::Relaxed);
    while manager.facet_cache.len() >= cap {
        let key = {
            let entry = manager.facet_cache.iter().next().unwrap();
            entry.key().clone()
        };
        manager.facet_cache.remove(&key);
    }

    assert_eq!(manager.facet_cache.len(), 14, "evicted down to cap-1");
}

#[tokio::test]
async fn test_facet_cache_no_eviction_under_cap() {
    let (_temp_dir, manager) = setup_with_cap(10, 50).await;

    let facets = vec![FacetRequest {
        field: "category".to_string(),
        path: "/category".to_string(),
    }];

    for i in 0..20 {
        let query = format!("q{}", i);
        let _ = manager.search_with_facets("t1", &query, None, None, 1, 0, Some(&facets));
    }

    // Time-based facet cache keys exclude query_text, so all 20 queries
    // with the same facets/filter share one cache entry.
    let cache_len = manager.facet_cache.len();
    assert_eq!(
        cache_len, 1,
        "all queries with same facets/filter should share one cache entry, got {}",
        cache_len
    );
}

#[tokio::test]
async fn test_facet_cache_still_returns_correct_results() {
    let (_temp_dir, manager) = setup_with_docs(100).await;

    let facets = vec![FacetRequest {
        field: "category".to_string(),
        path: "/category".to_string(),
    }];

    let r1 = manager
        .search_with_facets("t1", "product", None, None, 10, 0, Some(&facets))
        .unwrap();
    assert!(!r1.facets.is_empty(), "should have facet results");
    assert!(
        r1.facets.contains_key("category"),
        "should have category facet"
    );

    let r2 = manager
        .search_with_facets("t1", "product", None, None, 10, 0, Some(&facets))
        .unwrap();
    assert_eq!(
        r1.facets["category"].len(),
        r2.facets["category"].len(),
        "cached result should match"
    );
}

#[tokio::test]
async fn test_facet_cache_invalidated_on_write() {
    let (_temp_dir, manager) = setup_with_docs(50).await;

    let facets = vec![FacetRequest {
        field: "category".to_string(),
        path: "/category".to_string(),
    }];

    let _ = manager
        .search_with_facets("t1", "product", None, None, 10, 0, Some(&facets))
        .unwrap();
    assert!(!manager.facet_cache.is_empty(), "cache should have entries");

    manager
        .add_documents_sync(
            "t1",
            vec![doc(
                "newdoc",
                vec![("name", text("product new")), ("category", text("catnew"))],
            )],
        )
        .await
        .unwrap();

    let r = manager
        .search_with_facets("t1", "product", None, None, 100, 0, Some(&facets))
        .unwrap();
    let cat_paths: Vec<&str> = r.facets["category"]
        .iter()
        .map(|f| f.path.as_str())
        .collect();
    assert!(
        cat_paths.iter().any(|p| p.contains("catnew")),
        "new category should appear after cache invalidation"
    );
}
