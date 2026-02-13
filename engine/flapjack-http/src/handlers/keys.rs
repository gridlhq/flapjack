use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use chrono::Utc;
use serde::Deserialize;
use std::sync::Arc;

use crate::auth::KeyStore;

#[derive(Debug, Deserialize)]
pub struct CreateKeyRequest {
    pub acl: Vec<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub indexes: Option<Vec<String>>,
    #[serde(default, rename = "maxHitsPerQuery")]
    pub max_hits_per_query: Option<i64>,
    #[serde(default, rename = "maxQueriesPerIPPerHour")]
    pub max_queries_per_ip_per_hour: Option<i64>,
    #[serde(default, rename = "queryParameters")]
    pub query_parameters: Option<String>,
    #[serde(default)]
    pub referers: Option<Vec<String>>,
    #[serde(default)]
    pub validity: Option<i64>,
}

/// Create a new API key
#[utoipa::path(
    post,
    path = "/1/keys",
    tag = "keys",
    request_body(content = serde_json::Value, description = "Key configuration"),
    responses(
        (status = 201, description = "Key created", body = serde_json::Value)
    ),
    security(
        ("api_key" = [])
    )
)]
pub async fn create_key(
    State(key_store): State<Arc<KeyStore>>,
    Json(body): Json<CreateKeyRequest>,
) -> impl IntoResponse {
    let key = crate::auth::ApiKey {
        value: String::new(),
        created_at: 0,
        acl: body.acl,
        description: body.description.unwrap_or_default(),
        indexes: body.indexes.unwrap_or_default(),
        max_hits_per_query: body.max_hits_per_query.unwrap_or(0),
        max_queries_per_ip_per_hour: body.max_queries_per_ip_per_hour.unwrap_or(0),
        query_parameters: body.query_parameters.unwrap_or_default(),
        referers: body.referers.unwrap_or_default(),
        validity: body.validity.unwrap_or(0),
    };

    let created = key_store.create_key(key);
    let response = serde_json::json!({
        "key": created.value,
        "createdAt": Utc::now().to_rfc3339(),
    });

    (StatusCode::CREATED, Json(response))
}

/// List all API keys
#[utoipa::path(
    get,
    path = "/1/keys",
    tag = "keys",
    responses(
        (status = 200, description = "List of keys", body = serde_json::Value)
    ),
    security(
        ("api_key" = [])
    )
)]
pub async fn list_keys(State(key_store): State<Arc<KeyStore>>) -> impl IntoResponse {
    let keys = key_store.list_all();
    Json(serde_json::json!({ "keys": keys }))
}

/// Get an API key by value
#[utoipa::path(
    get,
    path = "/1/keys/{key}",
    tag = "keys",
    params(
        ("key" = String, Path, description = "API key value")
    ),
    responses(
        (status = 200, description = "Key details", body = serde_json::Value),
        (status = 404, description = "Key not found")
    ),
    security(
        ("api_key" = [])
    )
)]
pub async fn get_key(
    State(key_store): State<Arc<KeyStore>>,
    Path(key_value): Path<String>,
) -> impl IntoResponse {
    match key_store.lookup(&key_value) {
        Some(key) => Json(serde_json::to_value(key).unwrap()).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"message": "Key not found", "status": 404})),
        )
            .into_response(),
    }
}

/// Update an API key
#[utoipa::path(
    put,
    path = "/1/keys/{key}",
    tag = "keys",
    params(
        ("key" = String, Path, description = "API key value")
    ),
    request_body(content = serde_json::Value, description = "Key updates"),
    responses(
        (status = 200, description = "Key updated", body = serde_json::Value),
        (status = 404, description = "Key not found")
    ),
    security(
        ("api_key" = [])
    )
)]
pub async fn update_key(
    State(key_store): State<Arc<KeyStore>>,
    Path(key_value): Path<String>,
    Json(body): Json<CreateKeyRequest>,
) -> impl IntoResponse {
    let updated = crate::auth::ApiKey {
        value: String::new(),
        created_at: 0,
        acl: body.acl,
        description: body.description.unwrap_or_default(),
        indexes: body.indexes.unwrap_or_default(),
        max_hits_per_query: body.max_hits_per_query.unwrap_or(0),
        max_queries_per_ip_per_hour: body.max_queries_per_ip_per_hour.unwrap_or(0),
        query_parameters: body.query_parameters.unwrap_or_default(),
        referers: body.referers.unwrap_or_default(),
        validity: body.validity.unwrap_or(0),
    };

    match key_store.update_key(&key_value, updated) {
        Some(_) => Json(serde_json::json!({
            "key": key_value,
            "updatedAt": Utc::now().to_rfc3339(),
        }))
        .into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"message": "Key not found", "status": 404})),
        )
            .into_response(),
    }
}

/// Delete an API key
#[utoipa::path(
    delete,
    path = "/1/keys/{key}",
    tag = "keys",
    params(
        ("key" = String, Path, description = "API key value")
    ),
    responses(
        (status = 200, description = "Key deleted", body = serde_json::Value),
        (status = 404, description = "Key not found")
    ),
    security(
        ("api_key" = [])
    )
)]
pub async fn delete_key(
    State(key_store): State<Arc<KeyStore>>,
    Path(key_value): Path<String>,
) -> impl IntoResponse {
    if key_store.is_admin(&key_value) {
        return (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({"message": "Cannot delete admin key", "status": 403})),
        )
            .into_response();
    }

    if key_store.delete_key(&key_value) {
        Json(serde_json::json!({
            "deletedAt": Utc::now().to_rfc3339(),
        }))
        .into_response()
    } else {
        (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"message": "Key not found", "status": 404})),
        )
            .into_response()
    }
}

/// Restore a deleted API key
#[utoipa::path(
    post,
    path = "/1/keys/{key}/restore",
    tag = "keys",
    params(
        ("key" = String, Path, description = "API key value")
    ),
    responses(
        (status = 200, description = "Key restored", body = serde_json::Value),
        (status = 404, description = "Key not found")
    ),
    security(
        ("api_key" = [])
    )
)]
pub async fn restore_key(
    State(key_store): State<Arc<KeyStore>>,
    Path(key_value): Path<String>,
) -> impl IntoResponse {
    match key_store.restore_key(&key_value) {
        Some(_) => Json(serde_json::json!({
            "createdAt": Utc::now().to_rfc3339(),
        }))
        .into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"message": "Key not found", "status": 404})),
        )
            .into_response(),
    }
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateSecuredKeyRequest {
    pub parent_api_key: String,
    #[serde(default)]
    pub restrictions: SecuredKeyRestrictions,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SecuredKeyRestrictions {
    #[serde(default)]
    pub filters: Option<String>,
    #[serde(default)]
    pub valid_until: Option<i64>,
    #[serde(default)]
    pub restrict_indices: Option<Vec<String>>,
    #[serde(default)]
    pub user_token: Option<String>,
    #[serde(default)]
    pub hits_per_page: Option<usize>,
}

/// Generate a secured API key with restrictions
#[utoipa::path(
    post,
    path = "/1/keys/generateSecuredApiKey",
    tag = "keys",
    request_body(content = serde_json::Value, description = "Secured key restrictions"),
    responses(
        (status = 200, description = "Secured key generated", body = serde_json::Value),
        (status = 400, description = "Invalid request")
    ),
    security(
        ("api_key" = [])
    )
)]
pub async fn generate_secured_key(
    State(key_store): State<Arc<KeyStore>>,
    Json(body): Json<GenerateSecuredKeyRequest>,
) -> impl IntoResponse {
    if key_store.is_admin(&body.parent_api_key) {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"message": "Cannot generate secured key from admin key", "status": 400})),
        ).into_response();
    }

    if key_store.lookup(&body.parent_api_key).is_none() {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"message": "Parent key not found", "status": 404})),
        )
            .into_response();
    }

    let mut params = Vec::new();
    if let Some(ref f) = body.restrictions.filters {
        params.push(format!("filters={}", urlencoding::encode(f)));
    }
    if let Some(vu) = body.restrictions.valid_until {
        params.push(format!("validUntil={}", vu));
    }
    if let Some(ref ri) = body.restrictions.restrict_indices {
        let json_arr = serde_json::to_string(ri).unwrap_or_default();
        params.push(format!(
            "restrictIndices={}",
            urlencoding::encode(&json_arr)
        ));
    }
    if let Some(ref ut) = body.restrictions.user_token {
        params.push(format!("userToken={}", urlencoding::encode(ut)));
    }
    if let Some(hpp) = body.restrictions.hits_per_page {
        params.push(format!("hitsPerPage={}", hpp));
    }
    let params_str = params.join("&");
    let secured_key = crate::auth::generate_secured_api_key(&body.parent_api_key, &params_str);

    Json(serde_json::json!({
        "securedApiKey": secured_key,
    }))
    .into_response()
}
