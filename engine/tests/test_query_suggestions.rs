/// Integration tests for Query Suggestions API.
///
/// TDD: these tests define the expected API behaviour.
/// Run `cargo test --test test_query_suggestions` to execute.
use serde_json::{json, Value};

mod common;
use common::{spawn_server, spawn_server_with_qs_analytics};

// ── helpers ──────────────────────────────────────────────────────────────────

fn client() -> reqwest::Client {
    reqwest::Client::new()
}

fn auth(rb: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
    rb.header("x-algolia-application-id", "test-app")
        .header("x-algolia-api-key", "test-key")
}

async fn post_config(base: &str, config: Value) -> reqwest::Response {
    auth(client().post(format!("{}/1/configs", base)))
        .json(&config)
        .send()
        .await
        .unwrap()
}

async fn get_config(base: &str, index_name: &str) -> reqwest::Response {
    auth(client().get(format!("{}/1/configs/{}", base, index_name)))
        .send()
        .await
        .unwrap()
}

async fn get_status(base: &str, index_name: &str) -> Value {
    auth(client().get(format!("{}/1/configs/{}/status", base, index_name)))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap()
}

/// Poll status until isRunning is false or timeout.
async fn wait_for_build(base: &str, index_name: &str) {
    for _ in 0..200 {
        let status = get_status(base, index_name).await;
        if status["isRunning"].as_bool() == Some(false) {
            return;
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    }
    panic!("Build for '{}' did not complete within 10s", index_name);
}

fn basic_config(suggestions_index: &str, source_index: &str) -> Value {
    json!({
        "indexName": suggestions_index,
        "sourceIndices": [{
            "indexName": source_index,
            "minHits": 5,
            "minLetters": 4
        }]
    })
}

// ── config CRUD ───────────────────────────────────────────────────────────────

#[tokio::test]
async fn config_crud_roundtrip() {
    let (addr, _tmp) = spawn_server().await;
    let base = format!("http://{}", addr);

    // Create
    let resp = post_config(&base, basic_config("my_suggestions", "products")).await;
    assert_eq!(resp.status(), 200, "create config should return 200");
    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["status"], 200);

    // Get
    let resp = get_config(&base, "my_suggestions").await;
    assert_eq!(resp.status(), 200, "get config should return 200");
    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["indexName"], "my_suggestions");
    assert_eq!(body["sourceIndices"][0]["indexName"], "products");

    // List
    let resp = auth(client().get(format!("{}/1/configs", base)))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let list: Value = resp.json().await.unwrap();
    assert!(
        list.as_array().unwrap().len() >= 1,
        "list should have ≥1 config"
    );

    // Update
    let updated = json!({
        "indexName": "my_suggestions",
        "sourceIndices": [{
            "indexName": "products",
            "minHits": 10,
            "minLetters": 5
        }]
    });
    let resp = auth(client().put(format!("{}/1/configs/my_suggestions", base)))
        .json(&updated)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200, "update should return 200");

    let resp = get_config(&base, "my_suggestions").await;
    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["sourceIndices"][0]["minHits"], 10);

    // Delete
    let resp = auth(client().delete(format!("{}/1/configs/my_suggestions", base)))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200, "delete should return 200");

    // Gone after delete
    let resp = get_config(&base, "my_suggestions").await;
    assert_eq!(resp.status(), 404, "config should be gone after delete");
}

#[tokio::test]
async fn create_duplicate_config_returns_409() {
    let (addr, _tmp) = spawn_server().await;
    let base = format!("http://{}", addr);

    post_config(&base, basic_config("dupe_test", "products")).await;
    let resp = post_config(&base, basic_config("dupe_test", "products")).await;
    assert_eq!(resp.status(), 409, "duplicate create should return 409");
}

#[tokio::test]
async fn get_nonexistent_config_returns_404() {
    let (addr, _tmp) = spawn_server().await;
    let base = format!("http://{}", addr);

    let resp = get_config(&base, "does_not_exist").await;
    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn update_nonexistent_config_returns_404() {
    let (addr, _tmp) = spawn_server().await;
    let base = format!("http://{}", addr);

    let resp = auth(client().put(format!("{}/1/configs/no_such_thing", base)))
        .json(&basic_config("no_such_thing", "products"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn delete_nonexistent_config_returns_404() {
    let (addr, _tmp) = spawn_server().await;
    let base = format!("http://{}", addr);

    let resp = auth(client().delete(format!("{}/1/configs/no_such_thing", base)))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);
}

// ── status ────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn status_returns_404_for_unknown_config() {
    let (addr, _tmp) = spawn_server().await;
    let base = format!("http://{}", addr);

    let resp = auth(client().get(format!("{}/1/configs/ghost/status", base)))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn build_status_reflects_last_build() {
    let (addr, _tmp) = spawn_server_with_qs_analytics("products").await;
    let base = format!("http://{}", addr);

    post_config(&base, basic_config("status_test", "products")).await;
    wait_for_build(&base, "status_test").await;

    let status = get_status(&base, "status_test").await;
    assert_eq!(status["isRunning"], false, "build should not be running");
    assert!(
        status["lastBuiltAt"].is_string(),
        "lastBuiltAt should be set after build"
    );
    assert!(
        status["lastSuccessfulBuiltAt"].is_string(),
        "lastSuccessfulBuiltAt should be set"
    );
}

// ── build from analytics ──────────────────────────────────────────────────────

#[tokio::test]
async fn build_from_analytics_data_populates_suggestions() {
    let (addr, _tmp) = spawn_server_with_qs_analytics("products").await;
    let base = format!("http://{}", addr);

    post_config(&base, basic_config("analytics_build_test", "products")).await;
    wait_for_build(&base, "analytics_build_test").await;

    // Search the suggestions index — should have records
    let resp = auth(client().post(format!("{}/1/indexes/analytics_build_test/query", base)))
        .json(&json!({"query": "", "hitsPerPage": 100}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.unwrap();
    let hits = body["hits"].as_array().unwrap();
    assert!(
        !hits.is_empty(),
        "suggestions index should have records after build"
    );

    // Every hit should have query, nb_words, popularity
    for hit in hits {
        assert!(
            hit["query"].is_string(),
            "hit missing 'query' field: {:?}",
            hit
        );
        assert!(
            hit["nb_words"].is_number(),
            "hit missing 'nb_words': {:?}",
            hit
        );
        assert!(
            hit["popularity"].is_number(),
            "hit missing 'popularity': {:?}",
            hit
        );
    }
}

#[tokio::test]
async fn nb_words_correct() {
    // "my_store" does NOT match "product/movie/shop" patterns → uses DEFAULT_QUERIES,
    // which includes "running shoes" (2 words), "blue dress" (2 words), etc.
    // Using "products" would map to PRODUCT_QUERIES (all single-word), making this
    // test vacuously pass because "running shoes" never appears in those suggestions.
    let (addr, _tmp) = spawn_server_with_qs_analytics("my_store").await;
    let base = format!("http://{}", addr);

    post_config(&base, basic_config("nb_words_test", "my_store")).await;
    wait_for_build(&base, "nb_words_test").await;

    // Fetch all suggestions
    let resp = auth(client().post(format!("{}/1/indexes/nb_words_test/query", base)))
        .json(&json!({"query": "", "hitsPerPage": 200}))
        .send()
        .await
        .unwrap();
    let body: Value = resp.json().await.unwrap();
    let hits = body["hits"].as_array().unwrap();

    // Every hit must have nb_words matching its actual word count
    assert!(!hits.is_empty(), "suggestions index should have records");
    for hit in hits {
        let q = hit["query"].as_str().unwrap_or("");
        let expected_words = q.split_whitespace().count() as i64;
        let got_words = hit["nb_words"].as_i64().expect("nb_words must be integer");
        assert_eq!(
            got_words, expected_words,
            "nb_words mismatch for query '{}': expected {}, got {}",
            q, expected_words, got_words
        );
    }

    // Specifically assert the 2-word query "running shoes" appears and has nb_words=2
    let running_shoes = hits
        .iter()
        .find(|h| h["query"].as_str() == Some("running shoes"));
    assert!(
        running_shoes.is_some(),
        "'running shoes' must appear in DEFAULT_QUERIES suggestions (it has high search volume)"
    );
    assert_eq!(
        running_shoes.unwrap()["nb_words"],
        2,
        "'running shoes' should have nb_words=2"
    );
}

#[tokio::test]
async fn min_letters_filter_excludes_short_queries() {
    let (addr, _tmp) = spawn_server_with_qs_analytics("products").await;
    let base = format!("http://{}", addr);

    // minLetters=6: any query shorter than 6 chars should be excluded
    let config = json!({
        "indexName": "min_letters_test",
        "sourceIndices": [{
            "indexName": "products",
            "minHits": 1,
            "minLetters": 6
        }]
    });
    post_config(&base, config).await;
    wait_for_build(&base, "min_letters_test").await;

    let resp = auth(client().post(format!("{}/1/indexes/min_letters_test/query", base)))
        .json(&json!({"query": "", "hitsPerPage": 200}))
        .send()
        .await
        .unwrap();
    let body: Value = resp.json().await.unwrap();
    let hits = body["hits"].as_array().unwrap();

    for hit in hits {
        let q = hit["query"].as_str().unwrap_or("");
        assert!(
            q.chars().count() >= 6,
            "Suggestion '{}' is shorter than minLetters=6",
            q
        );
    }
}

#[tokio::test]
async fn exclude_list_filter_removes_words() {
    let (addr, _tmp) = spawn_server_with_qs_analytics("products").await;
    let base = format!("http://{}", addr);

    let config = json!({
        "indexName": "exclude_test",
        "sourceIndices": [{
            "indexName": "products",
            "minHits": 1,
            "minLetters": 4
        }],
        "exclude": ["laptop", "samsung"]
    });
    post_config(&base, config).await;
    wait_for_build(&base, "exclude_test").await;

    let resp = auth(client().post(format!("{}/1/indexes/exclude_test/query", base)))
        .json(&json!({"query": "", "hitsPerPage": 200}))
        .send()
        .await
        .unwrap();
    let body: Value = resp.json().await.unwrap();
    let hits = body["hits"].as_array().unwrap();

    // Verify suggestions are actually present (so the exclude assertions aren't vacuous)
    assert!(
        !hits.is_empty(),
        "expected suggestions from products analytics data"
    );

    for hit in hits {
        let q = hit["query"].as_str().unwrap_or("").to_lowercase();
        assert_ne!(q, "laptop", "'laptop' should be excluded");
        // "samsung" is in PRODUCT_QUERIES (count 45) so it would normally appear —
        // this properly exercises the exclude filter (unlike "shoes", which is in
        // DEFAULT_QUERIES but not PRODUCT_QUERIES and would never appear here).
        assert_ne!(q, "samsung", "'samsung' should be excluded");
    }
}

#[tokio::test]
async fn empty_analytics_builds_empty_index_no_crash() {
    // Use a source index name that has no analytics data — build should succeed
    // with 0 suggestions and not crash.
    let (addr, _tmp) = spawn_server_with_qs_analytics("products").await;
    let base = format!("http://{}", addr);

    let config = json!({
        "indexName": "empty_build_test",
        "sourceIndices": [{
            "indexName": "nonexistent_source_index_xyz",
            "minHits": 5,
            "minLetters": 4
        }]
    });
    post_config(&base, config).await;
    wait_for_build(&base, "empty_build_test").await;

    // Status should show a completed build (not stuck running)
    let status = get_status(&base, "empty_build_test").await;
    assert_eq!(status["isRunning"], false);
    assert!(status["lastBuiltAt"].is_string());
}

// ── delete config does not delete index ───────────────────────────────────────

#[tokio::test]
async fn delete_config_does_not_delete_suggestions_index() {
    let (addr, _tmp) = spawn_server_with_qs_analytics("products").await;
    let base = format!("http://{}", addr);

    post_config(&base, basic_config("del_no_index_test", "products")).await;
    wait_for_build(&base, "del_no_index_test").await;

    // Delete the config
    let resp = auth(client().delete(format!("{}/1/configs/del_no_index_test", base)))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // The config is gone
    assert_eq!(get_config(&base, "del_no_index_test").await.status(), 404);

    // But the suggestions index is still searchable
    let resp = auth(client().post(format!("{}/1/indexes/del_no_index_test/query", base)))
        .json(&json!({"query": ""}))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        200,
        "suggestions index should still be searchable after config delete"
    );
}

// ── logs ──────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn logs_returned_after_build() {
    let (addr, _tmp) = spawn_server_with_qs_analytics("products").await;
    let base = format!("http://{}", addr);

    post_config(&base, basic_config("logs_test", "products")).await;
    wait_for_build(&base, "logs_test").await;

    let resp = auth(client().get(format!("{}/1/logs/logs_test", base)))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let logs: Value = resp.json().await.unwrap();
    let entries = logs.as_array().unwrap();
    assert!(!entries.is_empty(), "should have log entries after build");
    // Each entry should have timestamp, level, message, contextLevel
    let first = &entries[0];
    assert!(first["timestamp"].is_string());
    assert!(first["level"].is_string());
    assert!(first["message"].is_string());
    assert!(first["contextLevel"].is_number());
}

// ── trigger build endpoint ────────────────────────────────────────────────────

#[tokio::test]
async fn trigger_build_returns_404_for_unknown_config() {
    let (addr, _tmp) = spawn_server().await;
    let base = format!("http://{}", addr);

    let resp = auth(client().post(format!("{}/1/configs/ghost/build", base)))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn trigger_build_succeeds_for_known_config() {
    let (addr, _tmp) = spawn_server_with_qs_analytics("products").await;
    let base = format!("http://{}", addr);

    // Create config (auto-triggers build)
    post_config(&base, basic_config("trigger_test", "products")).await;
    wait_for_build(&base, "trigger_test").await;

    // Trigger another explicit build
    let resp = auth(client().post(format!("{}/1/configs/trigger_test/build", base)))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    wait_for_build(&base, "trigger_test").await;

    // Status should reflect latest build
    let status = get_status(&base, "trigger_test").await;
    assert_eq!(status["isRunning"], false);
    assert!(status["lastBuiltAt"].is_string());
}

// ── objectID parity ───────────────────────────────────────────────────────────

/// Algolia SDK relies on objectID being present in every search hit.
/// For suggestions, objectID must equal the query string (it's the natural primary key).
#[tokio::test]
async fn suggestion_objectid_equals_query() {
    let (addr, _tmp) = spawn_server_with_qs_analytics("products").await;
    let base = format!("http://{}", addr);

    post_config(&base, basic_config("objectid_test", "products")).await;
    wait_for_build(&base, "objectid_test").await;

    let resp = auth(client().post(format!("{}/1/indexes/objectid_test/query", base)))
        .json(&json!({"query": "", "hitsPerPage": 50}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.unwrap();
    let hits = body["hits"].as_array().unwrap();

    assert!(!hits.is_empty(), "need suggestions to check objectID");
    for hit in hits {
        let object_id = hit["objectID"].as_str();
        let query = hit["query"].as_str();
        assert!(
            object_id.is_some(),
            "every suggestion hit must have objectID, got: {:?}",
            hit
        );
        assert!(
            query.is_some(),
            "every suggestion hit must have query field, got: {:?}",
            hit
        );
        assert_eq!(
            object_id, query,
            "objectID must equal query: objectID={:?}, query={:?}",
            object_id, query
        );
    }
}

// ── minHits boundary ─────────────────────────────────────────────────────────

/// minHits=999_999 is far above any 30-day count in seed data.
/// The suggestions index must be empty — this is an explicit boundary test for
/// the minHits filter (nb_words_correct and build_from_analytics only test it
/// implicitly with the default minHits=5).
#[tokio::test]
async fn min_hits_very_high_produces_empty_suggestions() {
    let (addr, _tmp) = spawn_server_with_qs_analytics("products").await;
    let base = format!("http://{}", addr);

    let config = json!({
        "indexName": "high_minhits_test",
        "sourceIndices": [{
            "indexName": "products",
            "minHits": 999999,
            "minLetters": 1
        }]
    });
    post_config(&base, config).await;
    wait_for_build(&base, "high_minhits_test").await;

    let resp = auth(client().post(format!("{}/1/indexes/high_minhits_test/query", base)))
        .json(&json!({"query": "", "hitsPerPage": 200}))
        .send()
        .await
        .unwrap();
    let body: Value = resp.json().await.unwrap();
    let hits = body["hits"].as_array().unwrap();
    assert!(
        hits.is_empty(),
        "minHits=999999 should exclude all queries, but got {} suggestions",
        hits.len()
    );
}

// ── update config triggers rebuild ───────────────────────────────────────────

/// PUT /1/configs/:name must trigger a new build that respects the updated config.
/// Strategy: build with minHits=5 (gets suggestions), then update to minHits=999999
/// (filters everything) and verify the suggestions index is now empty.
#[tokio::test]
async fn update_config_triggers_rebuild_with_new_params() {
    let (addr, _tmp) = spawn_server_with_qs_analytics("products").await;
    let base = format!("http://{}", addr);

    // Initial config: minHits=5 → should produce suggestions
    post_config(&base, basic_config("update_rebuild_test", "products")).await;
    wait_for_build(&base, "update_rebuild_test").await;

    let resp = auth(client().post(format!("{}/1/indexes/update_rebuild_test/query", base)))
        .json(&json!({"query": "", "hitsPerPage": 200}))
        .send()
        .await
        .unwrap();
    let body: Value = resp.json().await.unwrap();
    let initial_hits = body["hits"].as_array().unwrap().len();
    assert!(initial_hits > 0, "initial build should produce suggestions");

    // Update config: minHits=999999 → build should produce 0 suggestions
    let updated = json!({
        "indexName": "update_rebuild_test",
        "sourceIndices": [{
            "indexName": "products",
            "minHits": 999999,
            "minLetters": 1
        }]
    });
    let resp = auth(client().put(format!("{}/1/configs/update_rebuild_test", base)))
        .json(&updated)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200, "update should return 200");

    wait_for_build(&base, "update_rebuild_test").await;

    let resp = auth(client().post(format!("{}/1/indexes/update_rebuild_test/query", base)))
        .json(&json!({"query": "", "hitsPerPage": 200}))
        .send()
        .await
        .unwrap();
    let body: Value = resp.json().await.unwrap();
    let updated_hits = body["hits"].as_array().unwrap().len();
    assert_eq!(
        updated_hits, 0,
        "after update to minHits=999999, suggestions index must be empty (was {})",
        initial_hits
    );
}

// ── zero-result queries excluded ─────────────────────────────────────────────

/// Algolia parity: minHits filters on result count (nbHits), not search frequency.
/// Queries that return 0 results must NEVER appear in suggestions, even if users
/// searched for them many times.
///
/// The "products" seed data contains zero-result queries ("free shipping",
/// "coupon code", "refurbished xyz123", "wholesale bulk") that are searched many
/// times over 30 days but always return 0 hits. With minHits=1, only queries with
/// at least 1 result should appear.
#[tokio::test]
async fn zero_result_queries_excluded_from_suggestions() {
    let (addr, _tmp) = spawn_server_with_qs_analytics("products").await;
    let base = format!("http://{}", addr);

    let config = json!({
        "indexName": "zero_result_test",
        "sourceIndices": [{
            "indexName": "products",
            "minHits": 1,
            "minLetters": 1
        }]
    });
    post_config(&base, config).await;
    wait_for_build(&base, "zero_result_test").await;

    let resp = auth(client().post(format!("{}/1/indexes/zero_result_test/query", base)))
        .json(&json!({"query": "", "hitsPerPage": 200}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.unwrap();
    let hits = body["hits"].as_array().unwrap();

    // Must have some suggestions (queries with results should appear)
    assert!(
        !hits.is_empty(),
        "expected suggestions from products analytics data"
    );

    // Zero-result queries must NOT appear
    let zero_result_queries = [
        "free shipping",
        "coupon code",
        "refurbished xyz123",
        "wholesale bulk",
    ];
    for hit in hits {
        let q = hit["query"].as_str().unwrap_or("").to_lowercase();
        for bad in &zero_result_queries {
            assert_ne!(
                q, *bad,
                "zero-result query '{}' must not appear in suggestions (minHits filters on result count)",
                bad
            );
        }
    }
}

// ── update_config concurrent build guard ────────────────────────────────────

/// PUT /1/configs/:name while a build is running must return 409 Conflict.
/// Two simultaneous builds on the same staging index would corrupt each other.
#[tokio::test]
async fn update_config_while_building_returns_409() {
    let (addr, _tmp) = spawn_server_with_qs_analytics("products").await;
    let base = format!("http://{}", addr);

    // Create config — triggers auto-build
    post_config(&base, basic_config("update_conflict_test", "products")).await;

    // Immediately attempt an update before the build has time to finish.
    // The build was just triggered so isRunning should be true.
    let updated = json!({
        "indexName": "update_conflict_test",
        "sourceIndices": [{"indexName": "products", "minHits": 10, "minLetters": 4}]
    });
    let resp = auth(client().put(format!("{}/1/configs/update_conflict_test", base)))
        .json(&updated)
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        409,
        "update while build running must return 409 Conflict"
    );

    // Wait for the initial build to finish, then update should succeed
    wait_for_build(&base, "update_conflict_test").await;
    let resp = auth(client().put(format!("{}/1/configs/update_conflict_test", base)))
        .json(&updated)
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        200,
        "update after build completes must succeed"
    );
}

// ── no analytics engine ───────────────────────────────────────────────────────

/// When the server has no analytics engine, builds must gracefully skip
/// (not crash, not get stuck with isRunning=true).
/// lastBuiltAt stays null — a skipped build is NOT a successful build.
#[tokio::test]
async fn no_analytics_engine_build_gracefully_skips() {
    let (addr, _tmp) = spawn_server().await; // analytics_engine = None
    let base = format!("http://{}", addr);

    // Create triggers an async build; spawn_build detects None engine and returns early
    let resp = post_config(&base, basic_config("no_engine_test", "products")).await;
    assert_eq!(resp.status(), 200);

    // Must settle to isRunning=false within the timeout — NOT stuck
    wait_for_build(&base, "no_engine_test").await;

    let status = get_status(&base, "no_engine_test").await;
    assert_eq!(
        status["isRunning"], false,
        "build should not be stuck running when analytics engine is absent"
    );
    assert!(
        status["lastBuiltAt"].is_null(),
        "lastBuiltAt must be null when build was skipped (no analytics engine): {:?}",
        status["lastBuiltAt"]
    );
}
