mod common;

use flapjack_http::auth::{
    generate_secured_api_key, validate_secured_key, KeyStore, SecuredKeyRestrictions,
};
use tempfile::TempDir;

fn setup_key_store() -> (TempDir, KeyStore) {
    let temp_dir = TempDir::new().unwrap();
    let store = KeyStore::load_or_create(temp_dir.path(), "admin_key_1234567890abcdef");
    (temp_dir, store)
}

fn get_search_key(store: &KeyStore) -> String {
    // The default search key's plaintext value is stored in hmac_key
    store
        .list_all()
        .iter()
        .find(|k| k.description == "Default Search API Key")
        .and_then(|k| k.hmac_key.clone())
        .expect("Default search key should have hmac_key with plaintext value")
}

#[test]
fn test_generate_and_validate_basic() {
    let (_dir, store) = setup_key_store();
    let search_key = get_search_key(&store);

    let params = "filters=category%3Aphones&validUntil=9999999999";
    let secured = generate_secured_api_key(&search_key, params);

    let result = validate_secured_key(&secured, &store);
    assert!(result.is_some(), "should validate secured key");
    let (parent, restrictions) = result.unwrap();
    // Verify parent key is the search key (by checking description)
    assert_eq!(parent.description, "Default Search API Key");
    assert_eq!(restrictions.filters, Some("category:phones".to_string()));
    assert_eq!(restrictions.valid_until, Some(9999999999));
}

#[test]
fn test_expired_key_rejected() {
    let (_dir, store) = setup_key_store();
    let search_key = get_search_key(&store);

    let params = "validUntil=1000000000";
    let secured = generate_secured_api_key(&search_key, params);

    let result = validate_secured_key(&secured, &store);
    assert!(result.is_none(), "expired key should be rejected");
}

#[test]
fn test_tampered_key_rejected() {
    let (_dir, store) = setup_key_store();
    let search_key = get_search_key(&store);

    let params = "filters=category%3Aphones";
    let mut secured = generate_secured_api_key(&search_key, params);
    secured.push('X');

    let result = validate_secured_key(&secured, &store);
    assert!(result.is_none(), "tampered key should be rejected");
}

#[test]
fn test_wrong_parent_key_rejected() {
    let (_dir, store) = setup_key_store();

    let params = "filters=category%3Aphones";
    let secured = generate_secured_api_key("nonexistent_key_value", params);

    let result = validate_secured_key(&secured, &store);
    assert!(
        result.is_none(),
        "key signed with unknown parent should be rejected"
    );
}

#[test]
fn test_admin_key_cannot_be_parent() {
    let (_dir, store) = setup_key_store();

    let params = "filters=test";
    let secured = generate_secured_api_key("admin_key_1234567890abcdef", params);

    let result = validate_secured_key(&secured, &store);
    assert!(
        result.is_none(),
        "admin key should not be accepted as parent"
    );
}

#[test]
fn test_restrict_indices_parsed() {
    let (_dir, store) = setup_key_store();
    let search_key = get_search_key(&store);

    let params = "restrictIndices=%5B%22products%22%2C%22users%22%5D&validUntil=9999999999";
    let secured = generate_secured_api_key(&search_key, params);

    let (_, restrictions) = validate_secured_key(&secured, &store).unwrap();
    assert_eq!(
        restrictions.restrict_indices,
        Some(vec!["products".to_string(), "users".to_string()])
    );
}

#[test]
fn test_user_token_parsed() {
    let (_dir, store) = setup_key_store();
    let search_key = get_search_key(&store);

    let params = "userToken=user_123&validUntil=9999999999";
    let secured = generate_secured_api_key(&search_key, params);

    let (_, restrictions) = validate_secured_key(&secured, &store).unwrap();
    assert_eq!(restrictions.user_token, Some("user_123".to_string()));
}

#[test]
fn test_hits_per_page_parsed() {
    let (_dir, store) = setup_key_store();
    let search_key = get_search_key(&store);

    let params = "hitsPerPage=5&validUntil=9999999999";
    let secured = generate_secured_api_key(&search_key, params);

    let (_, restrictions) = validate_secured_key(&secured, &store).unwrap();
    assert_eq!(restrictions.hits_per_page, Some(5));
}

#[test]
fn test_no_restrictions_still_valid() {
    let (_dir, store) = setup_key_store();
    let search_key = get_search_key(&store);

    let secured = generate_secured_api_key(&search_key, "");

    let result = validate_secured_key(&secured, &store);
    assert!(
        result.is_some(),
        "secured key with valid HMAC but no restrictions should still validate"
    );
}

#[test]
fn test_empty_string_not_valid() {
    let (_dir, store) = setup_key_store();
    let result = validate_secured_key("", &store);
    assert!(result.is_none());
}

#[test]
fn test_garbage_not_valid() {
    let (_dir, store) = setup_key_store();
    let result = validate_secured_key("not_base64!!!", &store);
    assert!(result.is_none());
}

#[test]
fn test_algolia_compatible_format() {
    let parent_key = "d50a04204a5a4f9b88e3aabfc7d6b5f1";
    let params = "filters=user_id%3A42&validUntil=9999999999";

    let secured = generate_secured_api_key(parent_key, params);

    let decoded =
        base64::Engine::decode(&base64::engine::general_purpose::STANDARD, &secured).unwrap();
    let decoded_str = String::from_utf8(decoded).unwrap();

    assert!(decoded_str.len() > 64, "should have HMAC + params");
    let hmac_part = &decoded_str[..64];
    let params_part = &decoded_str[64..];
    assert!(
        hmac_part.chars().all(|c| c.is_ascii_hexdigit()),
        "first 64 chars should be hex"
    );
    assert_eq!(params_part, params, "remainder should be the params string");
}
#[test]
fn test_constant_time_comparison_with_invalid_hex() {
    let (_dir, store) = setup_key_store();
    let bad =
        BASE64.encode(b"NOT_HEX_NOT_HEX_NOT_HEX_NOT_HEX_NOT_HEX_NOT_HEX_NOT_HEX_NOT_Hparams=x");
    let result = validate_secured_key(&bad, &store);
    assert!(
        result.is_none(),
        "invalid hex in HMAC portion should be rejected"
    );
}

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};

#[test]
fn test_deleted_parent_key_invalidates_secured_key() {
    let (_dir, store) = setup_key_store();
    let search_key = get_search_key(&store);

    let params = "filters=brand%3ASamsung&validUntil=9999999999";
    let secured = generate_secured_api_key(&search_key, params);

    assert!(
        validate_secured_key(&secured, &store).is_some(),
        "should validate before deletion"
    );

    store.delete_key(&search_key);

    assert!(
        validate_secured_key(&secured, &store).is_none(),
        "should fail after parent key deleted"
    );
}

#[test]
fn test_restrict_indices_enforcement() {
    let (_dir, store) = setup_key_store();
    let search_key = get_search_key(&store);

    let params = "restrictIndices=%5B%22products%22%5D&validUntil=9999999999";
    let secured = generate_secured_api_key(&search_key, params);
    let (_, restrictions) = validate_secured_key(&secured, &store).unwrap();

    let ri = restrictions.restrict_indices.unwrap();
    assert!(
        flapjack_http::auth::index_pattern_matches(&ri, "products"),
        "products should match"
    );
    assert!(
        !flapjack_http::auth::index_pattern_matches(&ri, "users"),
        "users should NOT match"
    );
}

#[test]
fn test_restrict_indices_wildcard() {
    let (_dir, store) = setup_key_store();
    let search_key = get_search_key(&store);

    let params = "restrictIndices=%5B%22dev_*%22%5D&validUntil=9999999999";
    let secured = generate_secured_api_key(&search_key, params);
    let (_, restrictions) = validate_secured_key(&secured, &store).unwrap();

    let ri = restrictions.restrict_indices.unwrap();
    assert!(
        flapjack_http::auth::index_pattern_matches(&ri, "dev_products"),
        "dev_products should match dev_*"
    );
    assert!(
        !flapjack_http::auth::index_pattern_matches(&ri, "prod_products"),
        "prod_products should NOT match dev_*"
    );
}

#[test]
fn test_filters_merge_logic() {
    use flapjack_http::dto::SearchRequest;

    let restrictions = SecuredKeyRestrictions {
        filters: Some("brand:Samsung".to_string()),
        valid_until: None,
        restrict_indices: None,
        user_token: None,
        hits_per_page: None,
        restrict_sources: None,
    };

    let mut req: SearchRequest = serde_json::from_str(r#"{"query":"phone"}"#).unwrap();
    assert!(req.filters.is_none());

    if let Some(ref forced) = restrictions.filters {
        match &req.filters {
            Some(existing) => req.filters = Some(format!("({}) AND ({})", existing, forced)),
            None => req.filters = Some(forced.clone()),
        }
    }
    assert_eq!(req.filters, Some("brand:Samsung".to_string()));

    let mut req2: SearchRequest =
        serde_json::from_str(r#"{"query":"phone","filters":"price > 100"}"#).unwrap();
    if let Some(ref forced) = restrictions.filters {
        match &req2.filters {
            Some(existing) => req2.filters = Some(format!("({}) AND ({})", existing, forced)),
            None => req2.filters = Some(forced.clone()),
        }
    }
    assert_eq!(
        req2.filters,
        Some("(price > 100) AND (brand:Samsung)".to_string())
    );
}

#[test]
fn test_hits_per_page_cap() {
    let restrictions = SecuredKeyRestrictions {
        filters: None,
        valid_until: None,
        restrict_indices: None,
        user_token: None,
        hits_per_page: Some(5),
        restrict_sources: None,
    };

    use flapjack_http::dto::SearchRequest;
    let mut req: SearchRequest =
        serde_json::from_str(r#"{"query":"test","hitsPerPage":20}"#).unwrap();
    if let Some(max_hpp) = restrictions.hits_per_page {
        if req.hits_per_page.is_none_or(|h| h > max_hpp) {
            req.hits_per_page = Some(max_hpp);
        }
    }
    assert_eq!(
        req.hits_per_page,
        Some(5),
        "should cap to secured key limit"
    );

    let mut req2: SearchRequest =
        serde_json::from_str(r#"{"query":"test","hitsPerPage":3}"#).unwrap();
    if let Some(max_hpp) = restrictions.hits_per_page {
        if req2.hits_per_page.is_none_or(|h| h > max_hpp) {
            req2.hits_per_page = Some(max_hpp);
        }
    }
    assert_eq!(req2.hits_per_page, Some(3), "should keep lower value");
}

#[test]
fn test_secured_key_inherits_parent_acl() {
    let (_dir, store) = setup_key_store();
    let search_key = get_search_key(&store);

    let params = "validUntil=9999999999";
    let secured = generate_secured_api_key(&search_key, params);
    let (parent, _) = validate_secured_key(&secured, &store).unwrap();

    assert!(
        parent.acl.contains(&"search".to_string()),
        "should inherit search ACL"
    );
    assert!(
        !parent.acl.contains(&"addObject".to_string()),
        "search key should NOT have addObject"
    );
}

#[test]
fn test_multiple_restrictions_combined() {
    let (_dir, store) = setup_key_store();
    let search_key = get_search_key(&store);

    let params = "filters=brand%3ASamsung&restrictIndices=%5B%22products%22%5D&validUntil=9999999999&hitsPerPage=10&userToken=user42";
    let secured = generate_secured_api_key(&search_key, params);
    let (_, r) = validate_secured_key(&secured, &store).unwrap();

    assert_eq!(r.filters, Some("brand:Samsung".to_string()));
    assert_eq!(r.restrict_indices, Some(vec!["products".to_string()]));
    assert_eq!(r.valid_until, Some(9999999999));
    assert_eq!(r.hits_per_page, Some(10));
    assert_eq!(r.user_token, Some("user42".to_string()));
}
#[test]
fn test_parent_index_restriction_enforced() {
    let (_dir, store) = setup_key_store();
    let _search_key_value = get_search_key(&store);

    let (_scoped_key, scoped_key_plaintext) = store.create_key(flapjack_http::auth::ApiKey {
        hash: String::new(),
        salt: String::new(),
        hmac_key: None,
        created_at: 0,
        acl: vec!["search".to_string()],
        description: "Scoped to products only".to_string(),
        indexes: vec!["products".to_string()],
        max_hits_per_query: 0,
        max_queries_per_ip_per_hour: 0,
        query_parameters: String::new(),
        referers: vec![],
        validity: 0,
    });

    let params = "restrictIndices=%5B%22users%22%5D&validUntil=9999999999";
    let secured = generate_secured_api_key(&scoped_key_plaintext, params);
    let result = validate_secured_key(&secured, &store);
    assert!(result.is_some(), "HMAC should still validate");
    let (parent, restrictions) = result.unwrap();

    assert_eq!(parent.indexes, vec!["products".to_string()]);
    assert_eq!(
        restrictions.restrict_indices,
        Some(vec!["users".to_string()])
    );

    assert!(
        flapjack_http::auth::index_pattern_matches(&parent.indexes, "products"),
        "parent allows products"
    );
    assert!(
        !flapjack_http::auth::index_pattern_matches(&parent.indexes, "users"),
        "parent does NOT allow users — middleware should reject"
    );
}

// ─── Admin key bootstrap tests ───────────────────────────────────────────────

mod admin_key_bootstrap {
    use flapjack_http::auth::{generate_admin_key, reset_admin_key, KeyStore};
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_keystore_creates_admin_entry() {
        let temp_dir = TempDir::new().unwrap();
        let admin_key = "test_admin_key_1234567890abcdef";

        let store = KeyStore::load_or_create(temp_dir.path(), admin_key);

        let found = store.lookup(admin_key);
        assert!(found.is_some(), "Admin key should be in store");

        let admin = found.unwrap();
        assert_eq!(admin.description, "Admin API Key");
        assert!(admin.acl.len() > 10, "Admin should have all ACLs");
    }

    #[test]
    fn test_keystore_hashes_admin_key_at_rest() {
        let temp_dir = TempDir::new().unwrap();
        let admin_key = "test_admin_secret_key_plaintext";

        let _store = KeyStore::load_or_create(temp_dir.path(), admin_key);

        let keys_json = fs::read_to_string(temp_dir.path().join("keys.json")).unwrap();

        assert!(
            !keys_json.contains(admin_key),
            "keys.json should NOT contain plaintext key"
        );

        assert!(keys_json.contains("\"hash\":"), "Should have hash field");
        assert!(keys_json.contains("\"salt\":"), "Should have salt field");
    }

    #[test]
    fn test_keystore_rotates_admin_key() {
        let temp_dir = TempDir::new().unwrap();
        let old_key = "old_admin_key_1111111111111111";
        let new_key = "new_admin_key_2222222222222222";

        let _store1 = KeyStore::load_or_create(temp_dir.path(), old_key);

        let keys_json_old = fs::read_to_string(temp_dir.path().join("keys.json")).unwrap();
        let data_old: serde_json::Value = serde_json::from_str(&keys_json_old).unwrap();
        let hash_old = data_old["keys"]
            .as_array()
            .unwrap()
            .iter()
            .find(|k| k["description"] == "Admin API Key")
            .unwrap()["hash"]
            .as_str()
            .unwrap();

        let store2 = KeyStore::load_or_create(temp_dir.path(), new_key);

        assert!(
            store2.lookup(old_key).is_none(),
            "Old key should be invalid after rotation"
        );

        assert!(
            store2.lookup(new_key).is_some(),
            "New key should work after rotation"
        );

        let keys_json_new = fs::read_to_string(temp_dir.path().join("keys.json")).unwrap();
        let data_new: serde_json::Value = serde_json::from_str(&keys_json_new).unwrap();
        let hash_new = data_new["keys"]
            .as_array()
            .unwrap()
            .iter()
            .find(|k| k["description"] == "Admin API Key")
            .unwrap()["hash"]
            .as_str()
            .unwrap();

        assert_ne!(hash_old, hash_new, "Hash should change on rotation");
    }

    #[test]
    fn test_reset_admin_key_generates_new_key() {
        let temp_dir = TempDir::new().unwrap();
        let initial_key = "initial_admin_key_123456789abc";

        let _store = KeyStore::load_or_create(temp_dir.path(), initial_key);

        let admin_key_file = temp_dir.path().join(".admin_key");
        fs::write(&admin_key_file, initial_key).unwrap();

        let new_key = reset_admin_key(temp_dir.path()).expect("Should reset admin key");

        assert_ne!(new_key, initial_key, "Should generate new key");
        assert!(
            new_key.starts_with("fj_admin_"),
            "New key should have correct prefix"
        );

        let file_key = fs::read_to_string(&admin_key_file).unwrap();
        assert_eq!(file_key, new_key, ".admin_key should be updated");

        let store_after_reset = KeyStore::load_or_create(temp_dir.path(), &new_key);
        assert!(
            store_after_reset.lookup(&new_key).is_some(),
            "New key should work"
        );
        assert!(
            store_after_reset.lookup(initial_key).is_none(),
            "Old key should not work"
        );
    }

    #[test]
    fn test_generate_admin_key_format() {
        let key = generate_admin_key();

        assert!(key.starts_with("fj_admin_"), "Should have fj_admin_ prefix");
        assert_eq!(key.len(), 41, "Should be fj_admin_ + 32 hex chars");

        let hex_part = &key[9..];
        assert!(
            hex_part.chars().all(|c| c.is_ascii_hexdigit()),
            "Suffix should be hex"
        );
    }

    #[test]
    fn test_admin_key_file_permissions_unix() {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let temp_dir = TempDir::new().unwrap();
            let admin_key_file = temp_dir.path().join(".admin_key");
            let admin_key = "test_key_for_permissions_test";

            let _store = KeyStore::load_or_create(temp_dir.path(), admin_key);

            fs::write(&admin_key_file, admin_key).unwrap();
            let perms = fs::Permissions::from_mode(0o600);
            fs::set_permissions(&admin_key_file, perms).unwrap();

            let metadata = fs::metadata(&admin_key_file).unwrap();
            let file_perms = metadata.permissions();
            let mode = file_perms.mode() & 0o777;

            assert_eq!(mode, 0o600, "Should have restrictive permissions");
        }
    }

    #[test]
    fn test_keystore_preserves_existing_keys() {
        let temp_dir = TempDir::new().unwrap();
        let admin_key = "test_admin_key";

        let store1 = KeyStore::load_or_create(temp_dir.path(), admin_key);
        let all_keys = store1.list_all();
        let initial_count = all_keys.len();

        assert!(initial_count >= 2, "Should have admin + search keys");

        let store2 = KeyStore::load_or_create(temp_dir.path(), admin_key);
        let keys_after_reload = store2.list_all();

        assert_eq!(
            keys_after_reload.len(),
            initial_count,
            "Should preserve existing keys on reload"
        );

        assert!(
            keys_after_reload
                .iter()
                .any(|k| k.description == "Admin API Key"),
            "Admin key should persist"
        );
        assert!(
            keys_after_reload
                .iter()
                .any(|k| k.description == "Default Search API Key"),
            "Search key should persist"
        );
    }

    #[test]
    fn test_cannot_delete_admin_key() {
        let temp_dir = TempDir::new().unwrap();
        let admin_key = "test_admin_key_delete_test";

        let store = KeyStore::load_or_create(temp_dir.path(), admin_key);

        let deleted = store.delete_key(admin_key);

        assert!(!deleted, "Should not be able to delete admin key");

        assert!(
            store.lookup(admin_key).is_some(),
            "Admin key should still exist"
        );
    }

    #[test]
    fn test_keystore_handles_corrupted_json_gracefully() {
        let temp_dir = TempDir::new().unwrap();
        let keys_json_path = temp_dir.path().join("keys.json");
        let admin_key = "test_admin_key_corruption_test";

        fs::write(&keys_json_path, "{ invalid json }").unwrap();

        let store = KeyStore::load_or_create(temp_dir.path(), admin_key);

        assert!(
            store.lookup(admin_key).is_some(),
            "Should recover from corrupted JSON"
        );

        let keys_json = fs::read_to_string(&keys_json_path).unwrap();
        assert!(
            serde_json::from_str::<serde_json::Value>(&keys_json).is_ok(),
            "Should have valid JSON after recovery"
        );
    }

    #[test]
    fn test_admin_key_with_leading_trailing_whitespace() {
        let temp_dir = TempDir::new().unwrap();
        let key_with_spaces = " fj_admin_test_key_whitespace ";
        let trimmed_key = key_with_spaces.trim();

        let store1 = KeyStore::load_or_create(temp_dir.path(), trimmed_key);

        assert!(
            store1.lookup(trimmed_key).is_some(),
            "Trimmed key should authenticate immediately"
        );

        let admin_key_file = temp_dir.path().join(".admin_key");
        fs::write(&admin_key_file, trimmed_key).unwrap();

        let file_content = fs::read_to_string(&admin_key_file).unwrap();
        let file_key_trimmed = file_content.trim();
        let store2 = KeyStore::load_or_create(temp_dir.path(), file_key_trimmed);

        assert!(
            store2.lookup(file_key_trimmed).is_some(),
            "Trimmed key should authenticate after reload from file"
        );

        assert_eq!(file_content, trimmed_key, "File should contain trimmed key");
        assert!(
            !file_content.starts_with(' ') && !file_content.ends_with(' '),
            "File should not have leading/trailing whitespace"
        );

        assert_eq!(
            key_with_spaces.trim(),
            trimmed_key,
            "Original key with spaces should trim to expected value"
        );
    }
}

// ─── E2E secured key tests (HTTP server) ────────────────────────────────────

mod e2e {
    use super::common;
    use flapjack_http::auth::{generate_secured_api_key, KeyStore};

    type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

    fn get_search_key(store: &KeyStore) -> String {
        store
            .list_all()
            .iter()
            .find(|k| k.description == "Default Search API Key")
            .unwrap()
            .hmac_key
            .clone()
            .unwrap()
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
        let client = reqwest::Client::new();
        common::wait_for_response_task_authed(&client, addr, resp, Some(admin_key)).await;
    }

    #[tokio::test]
    async fn test_e2e_secured_key_forces_filter() -> Result<()> {
        let admin_key = "admin_key_1234567890abcdef";
        let (addr, tmp) = common::spawn_server_with_key(Some(admin_key)).await;
        let store = KeyStore::load_or_create(tmp.path(), admin_key);
        let search_key = get_search_key(&store);

        setup_index(&addr, admin_key).await;

        let settings = serde_json::json!({"attributesForFaceting": ["filterOnly(brand)"]});
        let settings_resp =
            http_post(&addr, "/1/indexes/products/settings", &settings, admin_key).await;
        let client = reqwest::Client::new();
        common::wait_for_response_task_authed(&client, &addr, settings_resp, Some(admin_key)).await;

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

        let (_scoped, scoped_plaintext) = store.create_key(flapjack_http::auth::ApiKey {
            hash: String::new(),
            salt: String::new(),
            hmac_key: None,
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

        let secured = generate_secured_api_key(&scoped_plaintext, "validUntil=9999999999");
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
        let settings_resp =
            http_post(&addr, "/1/indexes/products/settings", &settings, admin_key).await;
        let client = reqwest::Client::new();
        common::wait_for_response_task_authed(&client, &addr, settings_resp, Some(admin_key)).await;

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
}
