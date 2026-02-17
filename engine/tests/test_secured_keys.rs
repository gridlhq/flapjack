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
    store
        .list_all()
        .iter()
        .find(|k| k.description == "Default Search API Key")
        .unwrap()
        .value
        .clone()
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
    assert_eq!(parent.value, search_key);
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

    let scoped_key = store.create_key(flapjack_http::auth::ApiKey {
        value: String::new(),
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
    let secured = generate_secured_api_key(&scoped_key.value, params);
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
