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
    /// SHA-256 hash of the key value (for authentication)
    pub hash: String,
    /// Unique salt for this key (hex-encoded)
    pub salt: String,
    /// HMAC verification key (for secured API key validation)
    /// NOTE: Stored in plaintext to enable HMAC verification of secured keys.
    /// This is a security tradeoff - secured keys require the parent key for HMAC validation.
    /// Admin keys should not be used as parents for secured keys and won't have this field.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hmac_key: Option<String>,
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
        let mut data = if file_path.exists() {
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
            Self::create_default_keys(admin_key)
        };

        // Ensure the admin entry in keys.json matches the provided admin_key.
        // This handles key rotation via FLAPJACK_ADMIN_KEY env var.
        if let Some(admin_entry) = data
            .keys
            .iter_mut()
            .find(|k| k.description == "Admin API Key")
        {
            // Rehash the admin key if it changed
            if !verify_key(admin_key, &admin_entry.hash, &admin_entry.salt) {
                let new_salt = generate_salt();
                let new_hash = hash_key(admin_key, &new_salt);
                admin_entry.hash = new_hash;
                admin_entry.salt = new_salt;
                tracing::info!("Admin key rotated");
            }
        }

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

        let admin_salt = generate_salt();
        let admin_hash = hash_key(admin_key, &admin_salt);

        let admin = ApiKey {
            hash: admin_hash,
            salt: admin_salt,
            hmac_key: None, // Admin keys should not be used for secured key generation
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

        let search_key_value = format!("fj_search_{}", generate_hex_key());
        let search_salt = generate_salt();
        let search_hash = hash_key(&search_key_value, &search_salt);

        let search_key = ApiKey {
            hash: search_hash,
            salt: search_salt,
            hmac_key: Some(search_key_value.clone()), // Store for HMAC verification of secured keys
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
        data.keys
            .iter()
            .find(|k| verify_key(key_value, &k.hash, &k.salt))
            .cloned()
    }

    pub fn list_all(&self) -> Vec<ApiKey> {
        let data = self.data.read().unwrap();
        data.keys.clone()
    }

    /// Creates a new key and returns the plaintext value (only time it's visible)
    /// The key is hashed before storage
    pub fn create_key(&self, mut key: ApiKey) -> (ApiKey, String) {
        let plaintext_value = format!("fj_search_{}", generate_hex_key());
        let salt = generate_salt();
        let hash = hash_key(&plaintext_value, &salt);

        key.hash = hash;
        key.salt = salt;
        key.created_at = Utc::now().timestamp_millis();
        // Store hmac_key for secured key support (except for admin-like keys)
        key.hmac_key = Some(plaintext_value.clone());

        let mut data = self.data.write().unwrap();
        data.keys.push(key.clone());
        drop(data);
        self.save();

        (key, plaintext_value)
    }

    pub fn update_key(&self, key_value: &str, mut updated: ApiKey) -> Option<ApiKey> {
        let mut data = self.data.write().unwrap();
        if let Some(existing) = data
            .keys
            .iter_mut()
            .find(|k| verify_key(key_value, &k.hash, &k.salt))
        {
            // Preserve hash, salt, and creation time
            updated.hash = existing.hash.clone();
            updated.salt = existing.salt.clone();
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
        let mut data = self.data.write().unwrap();

        // Check if this is the admin key and prevent deletion
        if let Some(admin) = data.keys.iter().find(|k| k.description == "Admin API Key") {
            if verify_key(key_value, &admin.hash, &admin.salt) {
                return false;
            }
        }

        // Find and delete the key
        if let Some(pos) = data
            .keys
            .iter()
            .position(|k| verify_key(key_value, &k.hash, &k.salt))
        {
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
        if let Some(pos) = data
            .deleted_keys
            .iter()
            .position(|k| verify_key(key_value, &k.hash, &k.salt))
        {
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

#[derive(Debug, Clone, Default)]
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
        // Skip keys without hmac_key (admin keys, or keys that don't support secured key generation)
        let hmac_key_value = match &key.hmac_key {
            Some(v) => v,
            None => continue,
        };

        type HmacSha256 = Hmac<Sha256>;
        let hmac_bytes = match hex::decode(hmac_hex) {
            Ok(b) => b,
            Err(_) => return None,
        };
        let mut mac = HmacSha256::new_from_slice(hmac_key_value.as_bytes())
            .expect("HMAC accepts any key length");
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

/// Generate a random salt for key hashing
fn generate_salt() -> String {
    let mut rng = rand::thread_rng();
    let bytes: [u8; 32] = rng.gen();
    hex::encode(bytes)
}

/// Hash a key value with a salt using SHA-256
fn hash_key(key_value: &str, salt: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(salt.as_bytes());
    hasher.update(key_value.as_bytes());
    hex::encode(hasher.finalize())
}

/// Verify a key value against a stored hash and salt using constant-time comparison
fn verify_key(key_value: &str, stored_hash: &str, salt: &str) -> bool {
    let computed_hash = hash_key(key_value, salt);
    // Constant-time comparison to prevent timing attacks
    if computed_hash.len() != stored_hash.len() {
        return false;
    }
    let mut result = 0u8;
    for (a, b) in computed_hash.bytes().zip(stored_hash.bytes()) {
        result |= a ^ b;
    }
    result == 0
}

/// Generate a prefixed admin key (fj_admin_ + 32 hex chars).
pub fn generate_admin_key() -> String {
    format!("fj_admin_{}", generate_hex_key())
}

/// Read the admin key from an existing keys.json, if one exists.
/// NOTE: With hashed keys, this can no longer return the plaintext value.
/// This function is deprecated and always returns None.
/// The admin key must be provided via FLAPJACK_ADMIN_KEY env var.
#[deprecated(note = "Admin keys are now hashed at rest. Use FLAPJACK_ADMIN_KEY env var.")]
pub fn read_existing_admin_key(_data_dir: &Path) -> Option<String> {
    None
}

/// Generate a new admin key and update both .admin_key file and keys.json. Returns the new key.
pub fn reset_admin_key(data_dir: &Path) -> Result<String, String> {
    let file_path = data_dir.join("keys.json");
    if !file_path.exists() {
        return Err("No keys.json found. Start the server first to initialize.".into());
    }

    let contents = std::fs::read_to_string(&file_path)
        .map_err(|e| format!("Failed to read keys.json: {}", e))?;
    let mut data: KeyStoreData =
        serde_json::from_str(&contents).map_err(|e| format!("Failed to parse keys.json: {}", e))?;

    let new_key = generate_admin_key();
    let new_salt = generate_salt();
    let new_hash = hash_key(&new_key, &new_salt);

    if let Some(admin) = data
        .keys
        .iter_mut()
        .find(|k| k.description == "Admin API Key")
    {
        admin.hash = new_hash;
        admin.salt = new_salt;
    } else {
        return Err("No admin key found in keys.json.".into());
    }

    let json = serde_json::to_string_pretty(&data)
        .map_err(|e| format!("Failed to serialize keys.json: {}", e))?;
    std::fs::write(&file_path, json).map_err(|e| format!("Failed to write keys.json: {}", e))?;

    // Update the .admin_key file with the new plaintext key
    let admin_key_file = data_dir.join(".admin_key");
    std::fs::write(&admin_key_file, &new_key)
        .map_err(|e| format!("Failed to write .admin_key: {}", e))?;

    // Set restrictive permissions (Unix only)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Err(e) =
            std::fs::set_permissions(&admin_key_file, std::fs::Permissions::from_mode(0o600))
        {
            tracing::warn!("Failed to set .admin_key permissions: {}", e);
        }
    }

    Ok(new_key)
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

    // GET /1/indexes → listIndexes, POST /1/indexes → addObject (create index)
    if parts.len() == 2 && parts[0] == "1" && parts[1] == "indexes" {
        return match *method {
            Method::GET => Some("listIndexes"),
            Method::POST => Some("addObject"),
            _ => None,
        };
    }

    if parts.len() >= 3 && parts[0] == "1" && parts[1] == "indexes" {
        if parts.len() == 3 && !parts[2].is_empty() {
            // POST /1/indexes/:indexName → addObject (Algolia save-object endpoint)
            // DELETE /1/indexes/:indexName → deleteIndex
            return match *method {
                Method::DELETE => Some("deleteIndex"),
                Method::POST => Some("addObject"),
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

    // Skip auth for public endpoints: health check, metrics, dashboard UI, API docs
    if path == "/health"
        || path == "/metrics"
        || path.starts_with("/dashboard")
        || path.starts_with("/swagger-ui")
        || path.starts_with("/api-docs")
    {
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

#[cfg(test)]
mod tests {
    use super::*;

    // ── hash_key / verify_key ──

    #[test]
    fn hash_and_verify_roundtrip() {
        let salt = "test_salt_123";
        let key = "my_secret_key";
        let hash = hash_key(key, salt);
        assert!(verify_key(key, &hash, salt));
    }

    #[test]
    fn verify_wrong_key_fails() {
        let salt = "salt";
        let hash = hash_key("correct_key", salt);
        assert!(!verify_key("wrong_key", &hash, salt));
    }

    #[test]
    fn verify_wrong_salt_fails() {
        let hash = hash_key("key", "salt1");
        assert!(!verify_key("key", &hash, "salt2"));
    }

    #[test]
    fn hash_is_hex_64_chars() {
        let hash = hash_key("key", "salt");
        assert_eq!(hash.len(), 64); // SHA-256 = 32 bytes = 64 hex chars
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn hash_deterministic() {
        let h1 = hash_key("key", "salt");
        let h2 = hash_key("key", "salt");
        assert_eq!(h1, h2);
    }

    // ── generate_hex_key ──

    #[test]
    fn generate_hex_key_format() {
        let key = generate_hex_key();
        assert_eq!(key.len(), 32); // 16 bytes = 32 hex chars
        assert!(key.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn generate_hex_key_unique() {
        let k1 = generate_hex_key();
        let k2 = generate_hex_key();
        assert_ne!(k1, k2);
    }

    // ── generate_admin_key ──

    #[test]
    fn generate_admin_key_prefix() {
        let key = generate_admin_key();
        assert!(key.starts_with("fj_admin_"));
        assert_eq!(key.len(), 9 + 32); // prefix + 32 hex chars
    }

    // ── index_pattern_matches ──

    #[test]
    fn index_pattern_empty_matches_all() {
        assert!(index_pattern_matches(&[], "anything"));
    }

    #[test]
    fn index_pattern_exact_match() {
        let patterns = vec!["products".to_string()];
        assert!(index_pattern_matches(&patterns, "products"));
        assert!(!index_pattern_matches(&patterns, "users"));
    }

    #[test]
    fn index_pattern_star_matches_all() {
        let patterns = vec!["*".to_string()];
        assert!(index_pattern_matches(&patterns, "anything"));
    }

    #[test]
    fn index_pattern_prefix_wildcard() {
        let patterns = vec!["prod_*".to_string()];
        assert!(index_pattern_matches(&patterns, "prod_us"));
        assert!(index_pattern_matches(&patterns, "prod_eu"));
        assert!(!index_pattern_matches(&patterns, "dev_us"));
    }

    #[test]
    fn index_pattern_suffix_wildcard() {
        let patterns = vec!["*_prod".to_string()];
        assert!(index_pattern_matches(&patterns, "us_prod"));
        assert!(!index_pattern_matches(&patterns, "us_dev"));
    }

    #[test]
    fn index_pattern_contains_wildcard() {
        let patterns = vec!["*prod*".to_string()];
        assert!(index_pattern_matches(&patterns, "my_prod_index"));
        assert!(index_pattern_matches(&patterns, "production"));
        assert!(!index_pattern_matches(&patterns, "development"));
    }

    #[test]
    fn index_pattern_multiple_any_match() {
        let patterns = vec!["products".to_string(), "users".to_string()];
        assert!(index_pattern_matches(&patterns, "products"));
        assert!(index_pattern_matches(&patterns, "users"));
        assert!(!index_pattern_matches(&patterns, "orders"));
    }

    // ── extract_index_name ──

    #[test]
    fn extract_index_name_valid() {
        assert_eq!(
            extract_index_name("/1/indexes/products/query"),
            Some("products".to_string())
        );
    }

    #[test]
    fn extract_index_name_just_index() {
        assert_eq!(
            extract_index_name("/1/indexes/myindex"),
            Some("myindex".to_string())
        );
    }

    #[test]
    fn extract_index_name_queries_excluded() {
        assert_eq!(extract_index_name("/1/indexes/queries"), None);
    }

    #[test]
    fn extract_index_name_objects_excluded() {
        assert_eq!(extract_index_name("/1/indexes/objects"), None);
    }

    #[test]
    fn extract_index_name_too_short() {
        assert_eq!(extract_index_name("/1/indexes"), None);
    }

    #[test]
    fn extract_index_name_wrong_prefix() {
        assert_eq!(extract_index_name("/2/indexes/foo"), None);
    }

    // ── required_acl_for_route ──

    #[test]
    fn acl_keys_admin() {
        assert_eq!(
            required_acl_for_route(&Method::GET, "/1/keys"),
            Some("admin")
        );
        assert_eq!(
            required_acl_for_route(&Method::POST, "/1/keys"),
            Some("admin")
        );
    }

    #[test]
    fn acl_analytics_endpoint() {
        assert_eq!(
            required_acl_for_route(&Method::GET, "/2/searches"),
            Some("analytics")
        );
    }

    #[test]
    fn acl_events_search() {
        assert_eq!(
            required_acl_for_route(&Method::POST, "/1/events"),
            Some("search")
        );
    }

    #[test]
    fn acl_list_indexes() {
        assert_eq!(
            required_acl_for_route(&Method::GET, "/1/indexes"),
            Some("listIndexes")
        );
    }

    #[test]
    fn acl_search_query() {
        assert_eq!(
            required_acl_for_route(&Method::POST, "/1/indexes/products/query"),
            Some("search")
        );
    }

    #[test]
    fn acl_browse() {
        assert_eq!(
            required_acl_for_route(&Method::POST, "/1/indexes/products/browse"),
            Some("browse")
        );
    }

    #[test]
    fn acl_batch_add_object() {
        assert_eq!(
            required_acl_for_route(&Method::POST, "/1/indexes/products/batch"),
            Some("addObject")
        );
    }

    #[test]
    fn acl_settings_get() {
        assert_eq!(
            required_acl_for_route(&Method::GET, "/1/indexes/products/settings"),
            Some("settings")
        );
    }

    #[test]
    fn acl_settings_put() {
        assert_eq!(
            required_acl_for_route(&Method::PUT, "/1/indexes/products/settings"),
            Some("editSettings")
        );
    }

    #[test]
    fn acl_delete_index() {
        assert_eq!(
            required_acl_for_route(&Method::DELETE, "/1/indexes/products"),
            Some("deleteIndex")
        );
    }

    #[test]
    fn acl_clear_delete_object() {
        assert_eq!(
            required_acl_for_route(&Method::POST, "/1/indexes/products/clear"),
            Some("deleteObject")
        );
    }

    #[test]
    fn acl_tasks() {
        assert_eq!(
            required_acl_for_route(&Method::GET, "/1/tasks/123"),
            Some("search")
        );
    }

    // ── SecuredKeyRestrictions::from_params ──

    #[test]
    fn secured_key_restrictions_filters() {
        let r = SecuredKeyRestrictions::from_params("filters=brand%3ANike");
        assert_eq!(r.filters, Some("brand:Nike".to_string()));
    }

    #[test]
    fn secured_key_restrictions_valid_until() {
        let r = SecuredKeyRestrictions::from_params("validUntil=1700000000");
        assert_eq!(r.valid_until, Some(1700000000));
    }

    #[test]
    fn secured_key_restrictions_restrict_indices_csv() {
        let r = SecuredKeyRestrictions::from_params("restrictIndices=prod,staging");
        let indices = r.restrict_indices.unwrap();
        assert_eq!(indices, vec!["prod", "staging"]);
    }

    #[test]
    fn secured_key_restrictions_restrict_indices_json() {
        let r =
            SecuredKeyRestrictions::from_params("restrictIndices=%5B%22prod%22%2C%22staging%22%5D");
        let indices = r.restrict_indices.unwrap();
        assert_eq!(indices, vec!["prod", "staging"]);
    }

    #[test]
    fn secured_key_restrictions_user_token() {
        let r = SecuredKeyRestrictions::from_params("userToken=user123");
        assert_eq!(r.user_token, Some("user123".to_string()));
    }

    #[test]
    fn secured_key_restrictions_hits_per_page() {
        let r = SecuredKeyRestrictions::from_params("hitsPerPage=5");
        assert_eq!(r.hits_per_page, Some(5));
    }

    #[test]
    fn secured_key_restrictions_empty() {
        let r = SecuredKeyRestrictions::from_params("");
        assert!(r.filters.is_none());
        assert!(r.valid_until.is_none());
        assert!(r.restrict_indices.is_none());
        assert!(r.user_token.is_none());
        assert!(r.hits_per_page.is_none());
    }

    // ── generate_secured_api_key ──

    #[test]
    fn generate_secured_api_key_produces_base64() {
        let key = generate_secured_api_key("parent_key", "filters=brand:Nike");
        // Should be valid base64
        assert!(BASE64.decode(key.as_bytes()).is_ok());
    }

    #[test]
    fn generate_secured_api_key_deterministic() {
        let k1 = generate_secured_api_key("key", "params");
        let k2 = generate_secured_api_key("key", "params");
        assert_eq!(k1, k2);
    }

    #[test]
    fn generate_secured_api_key_different_params_differ() {
        let k1 = generate_secured_api_key("key", "filters=a");
        let k2 = generate_secured_api_key("key", "filters=b");
        assert_ne!(k1, k2);
    }
}
