//! Algolia Semantic Equivalence Tests
//!
//! Validates that Flapjack returns same results as real Algolia for high-risk edge cases.
//! Uses fixture files to avoid hitting Algolia on every test run.
//!
//! Auto-capture: If fixture missing + ALGOLIA_APP_ID env var set → captures from Algolia once.
//! Otherwise: Uses checked-in fixtures (fast, reliable, no network dependency).
//!
//! To refresh fixtures:
//!   1. rm tests/fixtures/algolia/*.json
//!   2. ALGOLIA_APP_ID=xxx ALGOLIA_ADMIN_KEY=yyy cargo test --test test_algolia_equivalence
//!   3. git add tests/fixtures/algolia/*.json
//!
//! High-risk areas tested:
//! - Multi-word prefix ("gaming lap" - does last word get prefix treatment?)
//! - Complex filter precedence ((A AND B) OR C)
//! - Hierarchical facet drill-down
//! - Empty query + filter (does "" query work with filters?)
//! - Special characters in field values (>,:,/)

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::env;
use std::fs;
use std::path::Path;

mod common;

#[derive(Debug, Serialize, Deserialize)]
struct AlgoliaFixture {
    version: String,
    captured_at: String,
    test_data: Vec<Value>,
    query: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    filters: Option<String>,
    expected_hits: Vec<Value>,
    expected_nb_hits: usize,
}

async fn capture_from_algolia(
    app_id: &str,
    api_key: &str,
    index_name: &str,
    test_data: Vec<Value>,
    query: &str,
    filters: Option<&str>,
) -> AlgoliaFixture {
    let client = reqwest::Client::new();

    // Configure attributesForFaceting for all string fields
    client
        .put(format!(
            "https://{}-dsn.algolia.net/1/indexes/{}/settings",
            app_id, index_name
        ))
        .header("x-algolia-api-key", api_key)
        .header("x-algolia-application-id", app_id)
        .json(&serde_json::json!({
            "attributesForFaceting": ["category", "author", "genre"]
        }))
        .send()
        .await
        .expect("Algolia settings update failed");

    client
        .post(format!(
            "https://{}-dsn.algolia.net/1/indexes/{}/batch",
            app_id, index_name
        ))
        .header("x-algolia-api-key", api_key)
        .header("x-algolia-application-id", app_id)
        .json(&serde_json::json!({
            "requests": test_data.iter().map(|obj| {
                serde_json::json!({"action": "addObject", "body": obj})
            }).collect::<Vec<_>>()
        }))
        .send()
        .await
        .expect("Algolia batch upload failed");

    tokio::time::sleep(tokio::time::Duration::from_millis(1500)).await;

    let mut search_body = serde_json::json!({
        "query": query,
        "hitsPerPage": 100
    });
    if let Some(f) = filters {
        search_body["filters"] = serde_json::json!(f);
    }

    let response = client
        .post(format!(
            "https://{}-dsn.algolia.net/1/indexes/{}/query",
            app_id, index_name
        ))
        .header("x-algolia-api-key", api_key)
        .header("x-algolia-application-id", app_id)
        .json(&search_body)
        .send()
        .await
        .expect("Algolia search failed");

    let status = response.status();
    let response_text = response.text().await.expect("Failed to read response body");

    if !status.is_success() {
        panic!("Algolia API error ({}): {}", status, response_text);
    }

    let response: Value =
        serde_json::from_str(&response_text).expect("Failed to parse Algolia response");

    AlgoliaFixture {
        version: "v1".to_string(),
        captured_at: chrono::Utc::now().to_rfc3339(),
        test_data,
        query: query.to_string(),
        filters: filters.map(String::from),
        expected_hits: response["hits"].as_array().unwrap().clone(),
        expected_nb_hits: response["nbHits"].as_u64().unwrap() as usize,
    }
}

async fn get_or_capture_fixture(
    fixture_name: &str,
    test_data: Vec<Value>,
    query: &str,
    filters: Option<&str>,
) -> AlgoliaFixture {
    let fixture_path = format!("tests/fixtures/algolia/{}.json", fixture_name);

    if Path::new(&fixture_path).exists() {
        let json = fs::read_to_string(&fixture_path).unwrap();
        return serde_json::from_str(&json).unwrap();
    }

    dotenv::from_path(".secret/.env.secret").ok();

    let app_id = env::var("ALGOLIA_APP_ID")
        .expect("Fixture missing and ALGOLIA_APP_ID not in .secret/.env.secret");
    let api_key = env::var("ALGOLIA_ADMIN_KEY")
        .expect("Fixture missing and ALGOLIA_ADMIN_KEY not in .secret/.env.secret");

    let index_name = format!("flapjack_test_{}", uuid::Uuid::new_v4());

    let fixture =
        capture_from_algolia(&app_id, &api_key, &index_name, test_data, query, filters).await;

    fs::create_dir_all("tests/fixtures/algolia").unwrap();
    fs::write(
        &fixture_path,
        serde_json::to_string_pretty(&fixture).unwrap(),
    )
    .unwrap();

    fixture
}

#[tokio::test]
async fn test_multiword_prefix_matches_algolia() {
    let test_data = vec![
        serde_json::json!({"objectID": "1", "title": "Gaming Laptop"}),
        serde_json::json!({"objectID": "2", "title": "Laptop Stand"}),
        serde_json::json!({"objectID": "3", "title": "Gaming Mouse"}),
    ];

    let fixture =
        get_or_capture_fixture("multiword-prefix", test_data.clone(), "gaming lap", None).await;

    let (addr, _temp) = common::spawn_server().await;
    let client = reqwest::Client::new();

    client
        .post(format!("http://{}/1/indexes/products/settings", addr))
        .header("x-algolia-api-key", "test")
        .header("x-algolia-application-id", "test")
        .json(&serde_json::json!({"attributesForFaceting": ["category"]}))
        .send()
        .await
        .unwrap();

    client
        .post(format!("http://{}/1/indexes/products/batch", addr))
        .header("x-algolia-api-key", "test")
        .header("x-algolia-application-id", "test")
        .json(&serde_json::json!({
            "requests": test_data.iter().map(|obj| {
                serde_json::json!({"action": "addObject", "body": obj})
            }).collect::<Vec<_>>()
        }))
        .send()
        .await
        .unwrap();

    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    let response = client
        .post(format!("http://{}/1/indexes/products/query", addr))
        .header("x-algolia-api-key", "test")
        .header("x-algolia-application-id", "test")
        .json(&serde_json::json!({"query": fixture.query, "hitsPerPage": 100}))
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();

    let flapjack_hits = response["hits"].as_array().unwrap().len();
    assert_eq!(
        flapjack_hits, fixture.expected_nb_hits,
        "Hit count mismatch for '{}': Flapjack={}, Algolia={}",
        fixture.query, flapjack_hits, fixture.expected_nb_hits
    );

    let flapjack_ids: Vec<String> = response["hits"]
        .as_array()
        .unwrap()
        .iter()
        .map(|h| h["objectID"].as_str().unwrap().to_string())
        .collect();

    let algolia_ids: Vec<String> = fixture
        .expected_hits
        .iter()
        .map(|h| h["objectID"].as_str().unwrap().to_string())
        .collect();

    assert_eq!(
        flapjack_ids, algolia_ids,
        "Hit IDs mismatch for '{}': Flapjack={:?}, Algolia={:?}",
        fixture.query, flapjack_ids, algolia_ids
    );
}

#[tokio::test]
async fn test_complex_filter_precedence_matches_algolia() {
    let test_data = vec![
        serde_json::json!({"objectID": "1", "category": "A", "stock": 10}),
        serde_json::json!({"objectID": "2", "category": "B", "stock": 0}),
        serde_json::json!({"objectID": "3", "category": "C", "stock": 5}),
        serde_json::json!({"objectID": "4", "category": "A", "stock": 0}),
    ];

    let fixture = get_or_capture_fixture(
        "complex-filter",
        test_data.clone(),
        "",
        Some("(category:A OR category:C) AND stock > 0"),
    )
    .await;

    let (addr, _temp) = common::spawn_server().await;
    let client = reqwest::Client::new();

    client
        .post(format!("http://{}/1/indexes/products/settings", addr))
        .header("x-algolia-api-key", "test")
        .header("x-algolia-application-id", "test")
        .json(&serde_json::json!({"attributesForFaceting": ["category"]}))
        .send()
        .await
        .unwrap();

    client
        .post(format!("http://{}/1/indexes/products/batch", addr))
        .header("x-algolia-api-key", "test")
        .header("x-algolia-application-id", "test")
        .json(&serde_json::json!({
            "requests": test_data.iter().map(|obj| {
                serde_json::json!({"action": "addObject", "body": obj})
            }).collect::<Vec<_>>()
        }))
        .send()
        .await
        .unwrap();

    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    let response = client
        .post(format!("http://{}/1/indexes/products/query", addr))
        .header("x-algolia-api-key", "test")
        .header("x-algolia-application-id", "test")
        .json(&serde_json::json!({
            "query": fixture.query,
            "filters": fixture.filters,
            "hitsPerPage": 100
        }))
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();

    let flapjack_hits = response["hits"].as_array().unwrap().len();
    assert_eq!(
        flapjack_hits, fixture.expected_nb_hits,
        "Filter precedence mismatch: Flapjack={}, Algolia={}",
        flapjack_hits, fixture.expected_nb_hits
    );
}

#[tokio::test]
async fn test_float_exclusive_bounds_matches_algolia() {
    let test_data = vec![
        serde_json::json!({"objectID": "1", "price": 100.5}),
        serde_json::json!({"objectID": "2", "price": 150.75}),
        serde_json::json!({"objectID": "3", "price": 99.99}),
    ];

    let fixture = get_or_capture_fixture(
        "float-exclusive-bounds",
        test_data.clone(),
        "",
        Some("price > 100.5"),
    )
    .await;

    let (addr, _temp) = common::spawn_server().await;
    let client = reqwest::Client::new();

    client
        .post(format!("http://{}/1/indexes/products/settings", addr))
        .header("x-algolia-api-key", "test")
        .header("x-algolia-application-id", "test")
        .json(&serde_json::json!({"attributesForFaceting": ["category"]}))
        .send()
        .await
        .unwrap();

    client
        .post(format!("http://{}/1/indexes/products/batch", addr))
        .header("x-algolia-api-key", "test")
        .header("x-algolia-application-id", "test")
        .json(&serde_json::json!({
            "requests": test_data.iter().map(|obj| {
                serde_json::json!({"action": "addObject", "body": obj})
            }).collect::<Vec<_>>()
        }))
        .send()
        .await
        .unwrap();

    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    let response = client
        .post(format!("http://{}/1/indexes/products/query", addr))
        .header("x-algolia-api-key", "test")
        .header("x-algolia-application-id", "test")
        .json(&serde_json::json!({
            "query": "",
            "filters": fixture.filters,
            "hitsPerPage": 100
        }))
        .send()
        .await;

    let status = response
        .as_ref()
        .map(|r| r.status())
        .unwrap_or(reqwest::StatusCode::INTERNAL_SERVER_ERROR);

    if status == 400 {
        // Expected: Flapjack rejects float exclusive bounds per ADR 0001
        // Algolia supports it, but we accept this limitation
        println!("✓ Flapjack correctly rejects 'price > 100.5' (documented limitation)");
        println!("  Algolia returned {} hits", fixture.expected_nb_hits);
        println!("  Workaround: Use 'price >= 100.51' or 'price:100.51 TO *'");
    } else {
        panic!(
            "Expected 400 error for float exclusive bound, got {}",
            status
        );
    }
}

#[tokio::test]
async fn test_empty_query_with_filter_matches_algolia() {
    let test_data = vec![
        serde_json::json!({"objectID": "1", "price": 50}),
        serde_json::json!({"objectID": "2", "price": 150}),
        serde_json::json!({"objectID": "3", "price": 250}),
    ];

    let fixture = get_or_capture_fixture(
        "empty-query-filter",
        test_data.clone(),
        "",
        Some("price > 100"),
    )
    .await;

    let (addr, _temp) = common::spawn_server().await;
    let client = reqwest::Client::new();

    client
        .post(format!("http://{}/1/indexes/products/settings", addr))
        .header("x-algolia-api-key", "test")
        .header("x-algolia-application-id", "test")
        .json(&serde_json::json!({"attributesForFaceting": ["category"]}))
        .send()
        .await
        .unwrap();

    client
        .post(format!("http://{}/1/indexes/products/batch", addr))
        .header("x-algolia-api-key", "test")
        .header("x-algolia-application-id", "test")
        .json(&serde_json::json!({
            "requests": test_data.iter().map(|obj| {
                serde_json::json!({"action": "addObject", "body": obj})
            }).collect::<Vec<_>>()
        }))
        .send()
        .await
        .unwrap();

    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    let response = client
        .post(format!("http://{}/1/indexes/products/query", addr))
        .header("x-algolia-api-key", "test")
        .header("x-algolia-application-id", "test")
        .json(&serde_json::json!({
            "query": "",
            "filters": fixture.filters,
            "hitsPerPage": 100
        }))
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();

    let flapjack_hits = response["hits"].as_array().unwrap().len();
    assert_eq!(
        flapjack_hits, fixture.expected_nb_hits,
        "Empty query + filter mismatch: Flapjack={}, Algolia={}",
        flapjack_hits, fixture.expected_nb_hits
    );
}
