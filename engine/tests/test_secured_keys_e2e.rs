mod common;

use flapjack_http::auth::{generate_secured_api_key, KeyStore};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn get_search_key(store: &KeyStore) -> String {
    store
        .list_all()
        .iter()
        .find(|k| k.description == "Default Search API Key")
        .unwrap()
        .value
        .clone()
}

async fn http_post(
    addr: &str,
    path: &str,
    body: &serde_json::Value,
    api_key: &str,
) -> reqwest::Response {
    reqwest::Client::new()
        .post(format!("http://{}{}", addr, path))
        .header("x-algolia-application-id", "test")
        .header("x-algolia-api-key", api_key)
        .header("content-type", "application/json")
        .json(body)
        .send()
        .await
        .unwrap()
}

async fn setup_index(addr: &str, admin_key: &str) {
    let docs = serde_json::json!({
        "requests": [
            {"action": "addObject", "body": {"objectID": "1", "brand": "Samsung", "name": "Galaxy S24"}},
            {"action": "addObject", "body": {"objectID": "2", "brand": "Samsung", "name": "Galaxy Tab"}},
            {"action": "addObject", "body": {"objectID": "3", "brand": "Apple", "name": "iPhone 15"}},
            {"action": "addObject", "body": {"objectID": "4", "brand": "Apple", "name": "MacBook Pro"}},
        ]
    });
    let resp = http_post(addr, "/1/indexes/products/batch", &docs, admin_key).await;
    assert_eq!(resp.status(), 200);
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
}

#[tokio::test]
async fn test_e2e_secured_key_forces_filter() -> Result<()> {
    let admin_key = "admin_key_1234567890abcdef";
    let (addr, tmp) = common::spawn_server_with_key(Some(admin_key)).await;
    let store = KeyStore::load_or_create(tmp.path(), admin_key);
    let search_key = get_search_key(&store);

    setup_index(&addr, admin_key).await;

    let settings = serde_json::json!({"attributesForFaceting": ["filterOnly(brand)"]});
    http_post(&addr, "/1/indexes/products/settings", &settings, admin_key).await;
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    let unrestricted = http_post(
        &addr,
        "/1/indexes/products/query",
        &serde_json::json!({"query": ""}),
        &search_key,
    )
    .await;
    let unrestricted_body: serde_json::Value = unrestricted.json().await?;
    assert_eq!(
        unrestricted_body["nbHits"], 4,
        "unrestricted should see all 4 docs"
    );

    let secured =
        generate_secured_api_key(&search_key, "filters=brand%3ASamsung&validUntil=9999999999");
    let restricted = http_post(
        &addr,
        "/1/indexes/products/query",
        &serde_json::json!({"query": ""}),
        &secured,
    )
    .await;
    assert_eq!(restricted.status(), 200);
    let restricted_body: serde_json::Value = restricted.json().await?;
    assert_eq!(
        restricted_body["nbHits"], 2,
        "secured key should only see Samsung docs"
    );

    let hits = restricted_body["hits"].as_array().unwrap();
    for hit in hits {
        assert_eq!(hit["brand"], "Samsung", "all hits should be Samsung");
    }

    Ok(())
}

#[tokio::test]
async fn test_e2e_secured_key_restrict_indices_blocks_wrong_index() -> Result<()> {
    let admin_key = "admin_key_1234567890abcdef";
    let (addr, tmp) = common::spawn_server_with_key(Some(admin_key)).await;
    let store = KeyStore::load_or_create(tmp.path(), admin_key);
    let search_key = get_search_key(&store);

    setup_index(&addr, admin_key).await;

    let secured = generate_secured_api_key(
        &search_key,
        "restrictIndices=%5B%22other_index%22%5D&validUntil=9999999999",
    );
    let resp = http_post(
        &addr,
        "/1/indexes/products/query",
        &serde_json::json!({"query": ""}),
        &secured,
    )
    .await;
    assert_eq!(resp.status(), 403, "should be blocked from products index");

    Ok(())
}

#[tokio::test]
async fn test_e2e_secured_key_expired_rejected() -> Result<()> {
    let admin_key = "admin_key_1234567890abcdef";
    let (addr, tmp) = common::spawn_server_with_key(Some(admin_key)).await;
    let store = KeyStore::load_or_create(tmp.path(), admin_key);
    let search_key = get_search_key(&store);

    let secured = generate_secured_api_key(&search_key, "validUntil=1000000000");
    let resp = http_post(
        &addr,
        "/1/indexes/products/query",
        &serde_json::json!({"query": ""}),
        &secured,
    )
    .await;
    assert_eq!(resp.status(), 403, "expired secured key should get 403");

    Ok(())
}

#[tokio::test]
async fn test_e2e_secured_key_parent_index_scope_enforced() -> Result<()> {
    let admin_key = "admin_key_1234567890abcdef";
    let (addr, tmp) = common::spawn_server_with_key(Some(admin_key)).await;
    let store = KeyStore::load_or_create(tmp.path(), admin_key);

    let scoped = store.create_key(flapjack_http::auth::ApiKey {
        value: String::new(),
        created_at: 0,
        acl: vec!["search".to_string()],
        description: "scoped".to_string(),
        indexes: vec!["allowed_*".to_string()],
        max_hits_per_query: 0,
        max_queries_per_ip_per_hour: 0,
        query_parameters: String::new(),
        referers: vec![],
        validity: 0,
    });

    let secured = generate_secured_api_key(&scoped.value, "validUntil=9999999999");
    let resp = http_post(
        &addr,
        "/1/indexes/products/query",
        &serde_json::json!({"query": ""}),
        &secured,
    )
    .await;
    assert_eq!(
        resp.status(),
        403,
        "parent scoped to allowed_* should block products"
    );

    Ok(())
}

#[tokio::test]
async fn test_e2e_secured_key_filter_merges_with_user_filter() -> Result<()> {
    let admin_key = "admin_key_1234567890abcdef";
    let (addr, tmp) = common::spawn_server_with_key(Some(admin_key)).await;
    let store = KeyStore::load_or_create(tmp.path(), admin_key);
    let search_key = get_search_key(&store);

    setup_index(&addr, admin_key).await;

    let settings = serde_json::json!({"attributesForFaceting": ["filterOnly(brand)"]});
    http_post(&addr, "/1/indexes/products/settings", &settings, admin_key).await;
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    let secured =
        generate_secured_api_key(&search_key, "filters=brand%3ASamsung&validUntil=9999999999");

    let resp = http_post(
        &addr,
        "/1/indexes/products/query",
        &serde_json::json!({"query": "Galaxy"}),
        &secured,
    )
    .await;
    let body: serde_json::Value = resp.json().await?;
    let hits = body["hits"].as_array().unwrap();
    assert!(!hits.is_empty(), "should find Samsung Galaxy docs");
    for hit in hits {
        assert_eq!(hit["brand"], "Samsung");
    }

    Ok(())
}
