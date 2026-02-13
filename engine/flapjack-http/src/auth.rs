use axum::{
    extract::Request,
    http::{Method, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use chrono::Utc;
use hmac::{Hmac, Mac};
use rand::Rng;
use sha2::Sha256;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKey {
    pub value: String,
    #[serde(rename = "createdAt")]
    pub created_at: i64,
    pub acl: Vec<String>,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub indexes: Vec<String>,
    #[serde(default, rename = "maxHitsPerQuery")]
    pub max_hits_per_query: i64,
    #[serde(default, rename = "maxQueriesPerIPPerHour")]
    pub max_queries_per_ip_per_hour: i64,
    #[serde(default, rename = "queryParameters")]
    pub query_parameters: String,
    #[serde(default)]
    pub referers: Vec<String>,
    #[serde(default)]
    pub validity: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct KeyStoreData {
    pub keys: Vec<ApiKey>,
    #[serde(default)]
    pub deleted_keys: Vec<ApiKey>,
}

pub struct KeyStore {
    data: RwLock<KeyStoreData>,
    file_path: PathBuf,
    admin_key_value: String,
}

impl KeyStore {
    pub fn load_or_create(data_dir: &Path, admin_key: &str) -> Self {
        let file_path = data_dir.join("keys.json");
        let data = if file_path.exists() {
            match std::fs::read_to_string(&file_path) {
                Ok(contents) => match serde_json::from_str::<KeyStoreData>(&contents) {
                    Ok(d) => d,
                    Err(e) => {
                        tracing::warn!("Failed to parse keys.json, recreating: {}", e);
                        Self::create_default_keys(admin_key)
                    }
                },
                Err(e) => {
                    tracing::warn!("Failed to read keys.json, recreating: {}", e);
                    Self::create_default_keys(admin_key)
                }
            }
        } else {
            let data = Self::create_default_keys(admin_key);
            let search_key = data
                .keys
                .iter()
                .find(|k| k.description == "Default Search API Key");
            tracing::info!("=== Flapjack API Keys ===");
            tracing::info!("Admin API Key: {}", admin_key);
            if let Some(sk) = search_key {
                tracing::info!("Default Search API Key: {}", sk.value);
            }
            tracing::info!("=========================");
            data
        };

        let store = Self {
            data: RwLock::new(data),
            file_path,
            admin_key_value: admin_key.to_string(),
        };
        store.save();
        store
    }

    fn create_default_keys(admin_key: &str) -> KeyStoreData {
        let now = Utc::now().timestamp_millis();
        let all_acls = vec![
            "search".into(),
            "browse".into(),
            "addObject".into(),
            "deleteObject".into(),
            "deleteIndex".into(),
            "settings".into(),
            "editSettings".into(),
            "listIndexes".into(),
            "logs".into(),
            "seeUnretrievableAttributes".into(),
            "analytics".into(),
            "recommendation".into(),
            "usage".into(),
            "inference".into(),
            "personalization".into(),
        ];

        let admin = ApiKey {
            value: admin_key.to_string(),
            created_at: now,
            acl: all_acls,
            description: "Admin API Key".into(),
            indexes: vec![],
            max_hits_per_query: 0,
            max_queries_per_ip_per_hour: 0,
            query_parameters: String::new(),
            referers: vec![],
            validity: 0,
        };

        let search_key = ApiKey {
            value: generate_hex_key(),
            created_at: now,
            acl: vec!["search".into()],
            description: "Default Search API Key".into(),
            indexes: vec![],
            max_hits_per_query: 0,
            max_queries_per_ip_per_hour: 0,
            query_parameters: String::new(),
            referers: vec![],
            validity: 0,
        };

        KeyStoreData {
            keys: vec![admin, search_key],
            deleted_keys: vec![],
        }
    }

    fn save(&self) {
        let data = self.data.read().unwrap();
        if let Ok(json) = serde_json::to_string_pretty(&*data) {
            if let Some(parent) = self.file_path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if let Err(e) = std::fs::write(&self.file_path, json) {
                tracing::warn!("Failed to save keys.json: {}", e);
            }
        }
    }

    pub fn is_admin(&self, key_value: &str) -> bool {
        key_value == self.admin_key_value
    }

    pub fn lookup(&self, key_value: &str) -> Option<ApiKey> {
        let data = self.data.read().unwrap();
        data.keys.iter().find(|k| k.value == key_value).cloned()
    }

    pub fn list_all(&self) -> Vec<ApiKey> {
        let data = self.data.read().unwrap();
        data.keys.clone()
    }

    pub fn create_key(&self, mut key: ApiKey) -> ApiKey {
        key.value = generate_hex_key();
        key.created_at = Utc::now().timestamp_millis();
        let mut data = self.data.write().unwrap();
        data.keys.push(key.clone());
        drop(data);
        self.save();
        key
    }

    pub fn update_key(&self, key_value: &str, mut updated: ApiKey) -> Option<ApiKey> {
        let mut data = self.data.write().unwrap();
        if let Some(existing) = data.keys.iter_mut().find(|k| k.value == key_value) {
            updated.value = existing.value.clone();
            updated.created_at = existing.created_at;
            *existing = updated.clone();
            drop(data);
            self.save();
            Some(updated)
        } else {
            None
        }
    }

    pub fn delete_key(&self, key_value: &str) -> bool {
        if key_value == self.admin_key_value {
            return false;
        }
        let mut data = self.data.write().unwrap();
        if let Some(pos) = data.keys.iter().position(|k| k.value == key_value) {
            let removed = data.keys.remove(pos);
            data.deleted_keys.push(removed);
            drop(data);
            self.save();
            true
        } else {
            false
        }
    }

    pub fn restore_key(&self, key_value: &str) -> Option<ApiKey> {
        let mut data = self.data.write().unwrap();
        if let Some(pos) = data.deleted_keys.iter().position(|k| k.value == key_value) {
            let restored = data.deleted_keys.remove(pos);
            data.keys.push(restored.clone());
            drop(data);
            self.save();
            Some(restored)
        } else {
            None
        }
    }

    pub fn admin_key_value(&self) -> &str {
        &self.admin_key_value
    }
}

#[derive(Debug, Clone)]
pub struct SecuredKeyRestrictions {
    pub filters: Option<String>,
    pub valid_until: Option<i64>,
    pub restrict_indices: Option<Vec<String>>,
    pub user_token: Option<String>,
    pub hits_per_page: Option<usize>,
    pub restrict_sources: Option<String>,
}

impl SecuredKeyRestrictions {
    fn from_params(params: &str) -> Self {
        let mut filters = None;
        let mut valid_until = None;
        let mut restrict_indices = None;
        let mut user_token = None;
        let mut hits_per_page = None;
        let mut restrict_sources = None;

        for (key, value) in url::form_urlencoded::parse(params.as_bytes()) {
            match key.as_ref() {
                "filters" => filters = Some(value.into_owned()),
                "validUntil" => valid_until = value.parse().ok(),
                "restrictIndices" => {
                    if let Ok(v) = serde_json::from_str::<Vec<String>>(&value) {
                        restrict_indices = Some(v);
                    } else {
                        restrict_indices =
                            Some(value.split(',').map(|s| s.trim().to_string()).collect());
                    }
                }
                "userToken" => user_token = Some(value.into_owned()),
                "hitsPerPage" => hits_per_page = value.parse().ok(),
                "restrictSources" => restrict_sources = Some(value.into_owned()),
                _ => {}
            }
        }

        SecuredKeyRestrictions {
            filters,
            valid_until,
            restrict_indices,
            user_token,
            hits_per_page,
            restrict_sources,
        }
    }
}

pub fn generate_secured_api_key(parent_key: &str, params: &str) -> String {
    type HmacSha256 = Hmac<Sha256>;
    let mut mac =
        HmacSha256::new_from_slice(parent_key.as_bytes()).expect("HMAC accepts any key length");
    mac.update(params.as_bytes());
    let hmac_hex = hex::encode(mac.finalize().into_bytes());
    let combined = format!("{}{}", hmac_hex, params);
    BASE64.encode(combined.as_bytes())
}

pub fn validate_secured_key(
    encoded: &str,
    key_store: &KeyStore,
) -> Option<(ApiKey, SecuredKeyRestrictions)> {
    let decoded = BASE64.decode(encoded.as_bytes()).ok()?;
    let decoded_str = String::from_utf8(decoded).ok()?;

    if decoded_str.len() < 64 {
        return None;
    }

    let hmac_hex = &decoded_str[..64];
    let params = &decoded_str[64..];

    let data = key_store.data.read().unwrap();
    for key in &data.keys {
        if key_store.is_admin(&key.value) {
            continue;
        }
        type HmacSha256 = Hmac<Sha256>;
        let hmac_bytes = match hex::decode(hmac_hex) {
            Ok(b) => b,
            Err(_) => return None,
        };
        let mut mac =
            HmacSha256::new_from_slice(key.value.as_bytes()).expect("HMAC accepts any key length");
        mac.update(params.as_bytes());
        if mac.verify_slice(&hmac_bytes).is_ok() {
            let restrictions = SecuredKeyRestrictions::from_params(params);

            if let Some(valid_until) = restrictions.valid_until {
                if Utc::now().timestamp() > valid_until {
                    return None;
                }
            }

            return Some((key.clone(), restrictions));
        }
    }
    None
}

pub fn generate_hex_key() -> String {
    let mut rng = rand::thread_rng();
    let bytes: [u8; 16] = rng.gen();
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

pub fn required_acl_for_route(method: &Method, path: &str) -> Option<&'static str> {
    if path.starts_with("/1/keys") {
        return Some("admin");
    }

    // Analytics API endpoints (/2/*) require "analytics" ACL
    if path.starts_with("/2/") {
        return Some("analytics");
    }

    // Insights API (/1/events) — uses "search" ACL (client-facing, matches Algolia behavior)
    if path == "/1/events" {
        return Some("search");
    }

    let parts: Vec<&str> = path.trim_start_matches('/').split('/').collect();

    if parts.len() >= 3 && parts[0] == "1" && parts[1] == "indexes" {
        if parts.len() == 3 && !parts[2].is_empty() {
            return match *method {
                Method::DELETE => Some("deleteIndex"),
                _ => None,
            };
        }

        if parts.len() >= 4 {
            let segment = parts[3];
            return match segment {
                "query" => Some("search"),
                "queries" => Some("search"),
                "browse" => Some("browse"),
                "batch" => Some("addObject"),
                "clear" => Some("deleteObject"),
                "deleteByQuery" => Some("deleteObject"),
                "operation" => Some("addObject"),
                "objects" => Some("search"),
                "settings" => match *method {
                    Method::GET => Some("settings"),
                    _ => Some("editSettings"),
                },
                "facets" => Some("search"),
                "synonyms" => match *method {
                    Method::GET => Some("settings"),
                    _ => Some("editSettings"),
                },
                "rules" => match *method {
                    Method::GET => Some("settings"),
                    _ => Some("editSettings"),
                },
                "task" => Some("search"),
                _ => match *method {
                    Method::GET => Some("search"),
                    Method::PUT => Some("addObject"),
                    Method::DELETE => Some("deleteObject"),
                    _ => Some("search"),
                },
            };
        }

        if parts.len() == 3 {
            return match *method {
                Method::GET => Some("listIndexes"),
                Method::POST => Some("addObject"),
                _ => None,
            };
        }
    }

    if parts.len() >= 2 && parts[0] == "1" && parts[1] == "tasks" {
        return Some("search");
    }

    None
}

pub fn index_pattern_matches(patterns: &[String], index_name: &str) -> bool {
    if patterns.is_empty() {
        return true;
    }
    patterns.iter().any(|pattern| {
        if pattern == "*" {
            true
        } else if pattern.starts_with('*') && pattern.ends_with('*') {
            let inner = &pattern[1..pattern.len() - 1];
            index_name.contains(inner)
        } else if let Some(suffix) = pattern.strip_prefix('*') {
            index_name.ends_with(suffix)
        } else if pattern.ends_with('*') {
            index_name.starts_with(&pattern[..pattern.len() - 1])
        } else {
            pattern == index_name
        }
    })
}

fn extract_index_name(path: &str) -> Option<String> {
    let parts: Vec<&str> = path.trim_start_matches('/').split('/').collect();
    if parts.len() >= 3 && parts[0] == "1" && parts[1] == "indexes" {
        let name = parts[2];
        if name != "queries" && name != "objects" {
            return Some(name.to_string());
        }
    }
    None
}

fn extract_api_key(request: &Request) -> Option<String> {
    if let Some(val) = request.headers().get("x-algolia-api-key") {
        return val.to_str().ok().map(|s| s.to_string());
    }
    if let Some(query) = request.uri().query() {
        for pair in query.split('&') {
            if let Some(val) = pair.strip_prefix("x-algolia-api-key=") {
                return Some(val.to_string());
            }
        }
    }
    None
}

fn has_header_or_param(request: &Request, key: &str) -> bool {
    request.headers().contains_key(key)
        || request
            .uri()
            .query()
            .is_some_and(|q| q.contains(&format!("{}=", key)))
}

fn error_json(message: &str, status: u16) -> Response {
    let body = serde_json::json!({ "message": message, "status": status });
    (
        StatusCode::from_u16(status).unwrap_or(StatusCode::FORBIDDEN),
        [("content-type", "application/json")],
        body.to_string(),
    )
        .into_response()
}

pub async fn authenticate_and_authorize(
    request: Request,
    next: Next,
) -> Result<Response, Response> {
    if request.method() == Method::OPTIONS {
        return Ok(next.run(request).await);
    }

    let path = request.uri().path().to_string();

    if path == "/health" {
        return Ok(next.run(request).await);
    }

    let key_store = request.extensions().get::<std::sync::Arc<KeyStore>>();

    if key_store.is_none() {
        return Ok(next.run(request).await);
    }

    let key_store = key_store.unwrap().clone();

    if !has_header_or_param(&request, "x-algolia-application-id") {
        return Err(error_json("Invalid Application-ID or API key", 403));
    }

    let api_key_value = match extract_api_key(&request) {
        Some(k) => k,
        None => return Err(error_json("Invalid Application-ID or API key", 403)),
    };

    let (api_key, secured_restrictions) = match key_store.lookup(&api_key_value) {
        Some(k) => (k, None),
        None => match validate_secured_key(&api_key_value, &key_store) {
            Some((parent_key, restrictions)) => (parent_key, Some(restrictions)),
            None => return Err(error_json("Invalid Application-ID or API key", 403)),
        },
    };

    if api_key.validity > 0 {
        let expires_at = api_key.created_at + (api_key.validity * 1000);
        if Utc::now().timestamp_millis() > expires_at {
            return Err(error_json("Invalid Application-ID or API key", 403));
        }
    }

    let method = request.method().clone();
    let required = required_acl_for_route(&method, &path);

    if let Some(acl) = required {
        if acl == "admin" {
            if !key_store.is_admin(&api_key_value) {
                let is_get_own_key = method == Method::GET && {
                    let parts: Vec<&str> = path.trim_start_matches('/').split('/').collect();
                    parts.len() == 3
                        && parts[0] == "1"
                        && parts[1] == "keys"
                        && parts[2] == api_key_value
                };
                if !is_get_own_key {
                    return Err(error_json("Method not allowed with this API key", 403));
                }
            }
        } else if !api_key.acl.iter().any(|a| a == acl) {
            return Err(error_json("Method not allowed with this API key", 403));
        }
    }

    if let Some(ref restrictions) = secured_restrictions {
        if let Some(ref index_name) = extract_index_name(&path) {
            if !api_key.indexes.is_empty() && !index_pattern_matches(&api_key.indexes, index_name) {
                return Err(error_json("Invalid Application-ID or API key", 403));
            }
            if let Some(ref restrict_indices) = restrictions.restrict_indices {
                if !index_pattern_matches(restrict_indices, index_name) {
                    return Err(error_json("Invalid Application-ID or API key", 403));
                }
            }
        }
    } else if let Some(index_name) = extract_index_name(&path) {
        if !index_pattern_matches(&api_key.indexes, &index_name) {
            return Err(error_json("Invalid Application-ID or API key", 403));
        }
    }

    let mut request = request;
    if let Some(restrictions) = secured_restrictions {
        request.extensions_mut().insert(restrictions);
    }

    Ok(next.run(request).await)
}
