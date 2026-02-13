/// Smoke test: fast, broad coverage across all workspace crates.
///
/// Crate coverage:
///   flapjack       - indexing, search, filters, sort, facets, delete, persistence,
///                    multi-tenant, oplog, synonyms, memory safety (budget, observer, pressure)
///   flapjack-http  - batch/search/health HTTP, auth/secured keys, internal endpoints,
///                    filter parser, DTO serialization
///   flapjack-ssl   - config env parsing
///   flapjack-replication - internal replication endpoint (in-process)
///
/// Target: < 4 seconds total.
use flapjack::index::settings::IndexSettings;
use flapjack::index::synonyms::{Synonym, SynonymStore};
use flapjack::types::{Document, FacetRequest, FieldValue, Filter, Sort, SortOrder};
use flapjack::IndexManager;
use flapjack_http::auth;
use flapjack_http::filter_parser::parse_filter;
use serde_json::json;
use std::collections::HashMap;
use tempfile::TempDir;

mod common;

fn doc(id: &str, title: &str, category: &str, price: i64) -> Document {
    Document {
        id: id.to_string(),
        fields: HashMap::from([
            ("title".to_string(), FieldValue::Text(title.to_string())),
            (
                "category".to_string(),
                FieldValue::Text(category.to_string()),
            ),
            ("price".to_string(), FieldValue::Integer(price)),
        ]),
    }
}

/// Core library: indexing, search, filters, sort, facets, delete,
/// multi-tenant isolation, persistence, oplog, synonyms.
#[tokio::test]
async fn smoke_library() {
    let tmp = TempDir::new().unwrap();
    let mgr = IndexManager::new(tmp.path());

    // -- Create tenant + settings --
    mgr.create_tenant("products").unwrap();
    let settings = IndexSettings {
        attributes_for_faceting: vec!["category".to_string()],
        searchable_attributes: Some(vec!["title".to_string()]),
        ..Default::default()
    };
    settings
        .save(tmp.path().join("products/settings.json"))
        .unwrap();

    // -- Index documents --
    let docs = vec![
        doc("1", "Gaming Laptop", "Electronics", 1200),
        doc("2", "Office Laptop", "Electronics", 800),
        doc("3", "Running Shoes", "Footwear", 150),
        doc("4", "Dress Shoes", "Footwear", 200),
        doc("5", "Mechanical Keyboard", "Electronics", 100),
    ];
    mgr.add_documents_sync("products", docs).await.unwrap();

    // -- Text search --
    let r = mgr.search("products", "laptop", None, None, 10).unwrap();
    assert_eq!(r.total, 2, "text search: expected 2 laptops");

    // -- Filter: price <= 200 --
    let filter = Filter::LessThanOrEqual {
        field: "price".to_string(),
        value: FieldValue::Integer(200),
    };
    let r = mgr.search("products", "", Some(&filter), None, 10).unwrap();
    assert_eq!(r.total, 3, "filter: expected 3 docs with price <= 200");

    // -- Sort by price ascending --
    let sort = Sort::ByField {
        field: "price".to_string(),
        order: SortOrder::Asc,
    };
    let r = mgr.search("products", "", None, Some(&sort), 10).unwrap();
    let prices: Vec<i64> = r
        .documents
        .iter()
        .filter_map(|d| d.document.fields.get("price")?.as_integer())
        .collect();
    assert_eq!(prices, vec![100, 150, 200, 800, 1200], "sort: wrong order");

    // -- Facets --
    let facets = [FacetRequest {
        field: "category".to_string(),
        path: "/category".to_string(),
    }];
    let r = mgr
        .search_with_facets("products", "", None, None, 10, 0, Some(&facets))
        .unwrap();
    assert_eq!(r.total, 5, "facets: should return all docs");
    let cat_facets = &r.facets["category"];
    assert!(
        cat_facets.len() >= 2,
        "facets: expected at least 2 categories, got {}",
        cat_facets.len()
    );

    // -- Combined: text + filter + sort --
    let filter_elec = Filter::Equals {
        field: "category".to_string(),
        value: FieldValue::Text("Electronics".to_string()),
    };
    let sort_desc = Sort::ByField {
        field: "price".to_string(),
        order: SortOrder::Desc,
    };
    let r = mgr
        .search("products", "", Some(&filter_elec), Some(&sort_desc), 10)
        .unwrap();
    assert_eq!(r.total, 3, "combined: expected 3 electronics");
    let prices: Vec<i64> = r
        .documents
        .iter()
        .filter_map(|d| d.document.fields.get("price")?.as_integer())
        .collect();
    assert_eq!(prices, vec![1200, 800, 100], "combined: wrong sort order");

    // -- Delete --
    mgr.delete_documents_sync("products", vec!["1".to_string()])
        .await
        .unwrap();
    let r = mgr.search("products", "laptop", None, None, 10).unwrap();
    assert_eq!(r.total, 1, "delete: should have 1 laptop left");
    assert_eq!(r.documents[0].document.id, "2");

    // -- Multi-tenant isolation --
    mgr.create_tenant("other").unwrap();
    mgr.add_documents_sync(
        "other",
        vec![Document {
            id: "x".to_string(),
            fields: HashMap::from([(
                "title".to_string(),
                FieldValue::Text("Unrelated".to_string()),
            )]),
        }],
    )
    .await
    .unwrap();
    let r = mgr.search("other", "laptop", None, None, 10).unwrap();
    assert_eq!(r.total, 0, "isolation: other tenant should not see laptops");
    let r = mgr.search("products", "", None, None, 10).unwrap();
    assert_eq!(
        r.total, 4,
        "isolation: products tenant should still have 4 docs"
    );

    // -- Oplog: verify writes were recorded --
    let oplog = mgr
        .get_or_create_oplog("products")
        .expect("oplog should exist");
    let ops = oplog.read_since(0).unwrap();
    assert!(!ops.is_empty(), "oplog: should have entries after writes");
    assert!(
        ops.iter().any(|e| e.op_type == "upsert"),
        "oplog: should contain upsert"
    );
    assert!(
        ops.iter().any(|e| e.op_type == "delete"),
        "oplog: should contain delete"
    );

    // -- Persistence: drop manager, reopen, verify data survives --
    drop(mgr);
    let mgr2 = IndexManager::new(tmp.path());
    let r = mgr2.search("products", "laptop", None, None, 10).unwrap();
    assert_eq!(r.total, 1, "persistence: laptop should survive restart");
    let r = mgr2.search("products", "", None, Some(&sort), 10).unwrap();
    assert_eq!(r.total, 4, "persistence: all 4 docs should survive");
}

/// Filter string parser (flapjack-http) + synonym engine (flapjack core).
/// Pure unit tests, no IndexManager needed.
#[test]
fn smoke_parser_and_synonyms() {
    // -- Filter parser: single, compound, numeric --
    let f = parse_filter("category:Electronics").unwrap();
    assert!(matches!(f, Filter::Equals { ref field, .. } if field == "category"));

    let f = parse_filter("price > 100").unwrap();
    assert!(matches!(f, Filter::GreaterThan { ref field, .. } if field == "price"));

    let f = parse_filter("price > 100 AND category:Electronics").unwrap();
    assert!(matches!(f, Filter::And(_)));

    let f = parse_filter("category:A OR category:B").unwrap();
    assert!(matches!(f, Filter::Or(_)));

    assert!(
        parse_filter("").is_err() || parse_filter("").is_ok(),
        "parser handles empty input without panic"
    );

    // -- Synonyms: regular + one-way + expand_query --
    let mut store = SynonymStore::new();
    store.insert(Synonym::Regular {
        object_id: "laptop-notebook".to_string(),
        synonyms: vec!["laptop".to_string(), "notebook".to_string()],
    });
    store.insert(Synonym::OneWay {
        object_id: "tablet-ipad".to_string(),
        input: "tablet".to_string(),
        synonyms: vec!["ipad".to_string()],
    });

    let expanded = store.expand_query("laptop bag");
    assert!(
        expanded.iter().any(|q| q.contains("notebook")),
        "synonyms: 'laptop' should expand to include 'notebook'"
    );

    let expanded = store.expand_query("tablet case");
    assert!(
        expanded.iter().any(|q| q.contains("ipad")),
        "synonyms: one-way 'tablet' should expand to 'ipad'"
    );

    // Reverse direction should NOT expand
    let expanded = store.expand_query("ipad case");
    assert!(
        !expanded.iter().any(|q| q.contains("tablet")),
        "synonyms: one-way should not expand in reverse"
    );
}

/// Auth & secured keys (flapjack-http auth module).
/// Pure unit tests, no HTTP.
#[test]
fn smoke_auth() {
    let tmp = TempDir::new().unwrap();
    let store = auth::KeyStore::load_or_create(tmp.path(), "admin_key_1234567890abcdef");

    // KeyStore creates default search + admin keys
    let keys = store.list_all();
    assert!(keys.len() >= 2, "auth: should have at least 2 default keys");

    let search_key = keys
        .iter()
        .find(|k| k.description == "Default Search API Key")
        .expect("auth: missing default search key");

    // Generate secured key, validate it
    let secured = auth::generate_secured_api_key(
        &search_key.value,
        "filters=category%3AElectronics&validUntil=9999999999",
    );
    let result = auth::validate_secured_key(&secured, &store);
    assert!(result.is_some(), "auth: secured key should validate");

    // Expired key rejected
    let expired = auth::generate_secured_api_key(&search_key.value, "validUntil=1000000000");
    assert!(
        auth::validate_secured_key(&expired, &store).is_none(),
        "auth: expired key should be rejected"
    );

    // Index pattern matching
    assert!(auth::index_pattern_matches(
        &["products".to_string()],
        "products"
    ));
    assert!(!auth::index_pattern_matches(
        &["products".to_string()],
        "users"
    ));
    assert!(auth::index_pattern_matches(
        &["dev_*".to_string()],
        "dev_products"
    ));
}

/// HTTP round-trip: spawn server, batch upload, search, health check.
/// Covers flapjack-http handlers + DTO serialization.
#[tokio::test]
async fn smoke_http() {
    let (addr, _temp) = common::spawn_server().await;
    let client = reqwest::Client::new();

    // Upload docs via batch API
    let resp = client
        .post(format!("http://{}/1/indexes/products/batch", addr))
        .header("x-algolia-api-key", "test")
        .header("x-algolia-application-id", "test")
        .json(&json!({
            "requests": [
                {"action": "addObject", "body": {"objectID": "1", "title": "Laptop", "price": 999}},
                {"action": "addObject", "body": {"objectID": "2", "title": "Mouse", "price": 49}}
            ]
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200, "batch upload failed");
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body.get("taskID").is_some(), "missing taskID");

    // Wait for async indexing
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    // Search
    let resp = client
        .post(format!("http://{}/1/indexes/products/query", addr))
        .header("x-algolia-api-key", "test")
        .header("x-algolia-application-id", "test")
        .json(&json!({"query": "laptop", "hitsPerPage": 10}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200, "search failed");
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body.get("hits").is_some(), "missing hits");
    assert!(body.get("nbHits").is_some(), "missing nbHits");
    let hits = body["hits"].as_array().unwrap();
    assert!(!hits.is_empty(), "search should return at least 1 hit");

    // Health check
    let resp = client
        .get(format!("http://{}/health", addr))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200, "health check failed");
}

/// Internal replication endpoint (flapjack-replication via flapjack-http).
/// In-process via tower::ServiceExt, no TCP.
#[tokio::test]
async fn smoke_internal_endpoint() {
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use axum::Router;
    use tower::ServiceExt;

    let tmp = TempDir::new().unwrap();
    let mgr = IndexManager::new(tmp.path());

    let state = std::sync::Arc::new(flapjack_http::handlers::AppState {
        manager: mgr,
        key_store: None,
        replication_manager: None,
        ssl_manager: None,
    });

    let internal = Router::new()
        .route(
            "/internal/status",
            axum::routing::get(flapjack_http::handlers::internal::replication_status),
        )
        .with_state(state);

    let resp = internal
        .oneshot(
            Request::builder()
                .uri("/internal/status")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "internal status endpoint should work without auth"
    );
}

/// Memory safety primitives: MemoryBudget validation, MemoryObserver,
/// pressure level override. Pure unit tests, no HTTP, no big allocations.
#[test]
fn smoke_memory_safety() {
    use flapjack::{MemoryBudget, MemoryBudgetConfig, MemoryObserver, PressureLevel};

    // -- MemoryBudget: document size validation --
    let budget = MemoryBudget::new(MemoryBudgetConfig::default()); // 3 MB default
    assert!(
        budget.validate_document_size(1024).is_ok(),
        "memory: 1 KB doc should be accepted"
    );
    assert!(
        budget.validate_document_size(4 * 1024 * 1024).is_err(),
        "memory: 4 MB doc should be rejected"
    );

    // -- MemoryObserver: global singleton returns sane stats --
    let observer = MemoryObserver::global();
    let stats = observer.stats();
    assert!(
        matches!(
            stats.pressure_level,
            PressureLevel::Normal | PressureLevel::Elevated | PressureLevel::Critical
        ),
        "memory: pressure_level should be a valid variant"
    );
    assert!(
        stats.allocator == "jemalloc" || stats.allocator == "system",
        "memory: allocator should be 'jemalloc' or 'system', got '{}'",
        stats.allocator
    );

    // -- Pressure override roundtrip --
    observer.set_pressure_override(Some(PressureLevel::Critical));
    assert_eq!(observer.pressure_level(), PressureLevel::Critical);
    observer.set_pressure_override(Some(PressureLevel::Elevated));
    assert_eq!(observer.pressure_level(), PressureLevel::Elevated);
    observer.set_pressure_override(Some(PressureLevel::Normal));
    assert_eq!(observer.pressure_level(), PressureLevel::Normal);
    observer.set_pressure_override(None); // clear
}

/// SSL config parsing (flapjack-ssl crate).
#[test]
#[serial_test::serial]
fn smoke_ssl_config() {
    // Missing env vars should error
    std::env::remove_var("FLAPJACK_SSL_EMAIL");
    std::env::remove_var("FLAPJACK_PUBLIC_IP");
    assert!(
        flapjack::SslConfig::from_env().is_err(),
        "ssl: missing env should error"
    );

    // Valid env vars should parse
    std::env::set_var("FLAPJACK_SSL_EMAIL", "test@example.com");
    std::env::set_var("FLAPJACK_PUBLIC_IP", "192.0.2.1");
    let config = flapjack::SslConfig::from_env().expect("ssl: valid env should parse");
    assert_eq!(config.email, "test@example.com");
    assert_eq!(config.public_ip.to_string(), "192.0.2.1");

    // Cleanup
    std::env::remove_var("FLAPJACK_SSL_EMAIL");
    std::env::remove_var("FLAPJACK_PUBLIC_IP");
}
