use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;

use super::AppState;
use flapjack::index::synonyms::{Synonym, SynonymStore};

/// Get a synonym by ID
#[utoipa::path(
    get,
    path = "/1/indexes/{indexName}/synonyms/{objectID}",
    tag = "synonyms",
    params(
        ("indexName" = String, Path, description = "Index name"),
        ("objectID" = String, Path, description = "Synonym ID")
    ),
    responses(
        (status = 200, description = "Synonym retrieved", body = serde_json::Value),
        (status = 404, description = "Synonym not found")
    ),
    security(
        ("api_key" = [])
    )
)]
pub async fn get_synonym(
    State(state): State<Arc<AppState>>,
    Path((index_name, object_id)): Path<(String, String)>,
) -> Result<Json<Synonym>, (StatusCode, String)> {
    let synonyms_path = state
        .manager
        .base_path
        .join(&index_name)
        .join("synonyms.json");

    if !synonyms_path.exists() {
        return Err((
            StatusCode::NOT_FOUND,
            format!("Synonym {} not found", object_id),
        ));
    }

    let store = SynonymStore::load(&synonyms_path)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    store
        .get(&object_id)
        .cloned()
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                format!("Synonym {} not found", object_id),
            )
        })
        .map(Json)
}

/// Create or update a synonym
#[utoipa::path(
    put,
    path = "/1/indexes/{indexName}/synonyms/{objectID}",
    tag = "synonyms",
    params(
        ("indexName" = String, Path, description = "Index name"),
        ("objectID" = String, Path, description = "Synonym ID")
    ),
    request_body(content = serde_json::Value, description = "Synonym data"),
    responses(
        (status = 200, description = "Synonym saved", body = serde_json::Value)
    ),
    security(
        ("api_key" = [])
    )
)]
pub async fn save_synonym(
    State(state): State<Arc<AppState>>,
    Path((index_name, _object_id)): Path<(String, String)>,
    Json(synonym): Json<Synonym>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    state
        .manager
        .create_tenant(&index_name)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let synonyms_path = state
        .manager
        .base_path
        .join(&index_name)
        .join("synonyms.json");

    let mut store = if synonyms_path.exists() {
        SynonymStore::load(&synonyms_path)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    } else {
        SynonymStore::new()
    };

    store.insert(synonym.clone());

    store
        .save(&synonyms_path)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    state.manager.invalidate_synonyms_cache(&index_name);

    state.manager.append_oplog(
        &index_name,
        "save_synonym",
        serde_json::to_value(&synonym).unwrap_or_default(),
    );

    let task = state
        .manager
        .make_noop_task(&index_name)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(serde_json::json!({
        "taskID": task.numeric_id,
        "updatedAt": chrono::Utc::now().to_rfc3339(),
        "id": synonym.object_id()
    })))
}

/// Delete a synonym by ID
#[utoipa::path(
    delete,
    path = "/1/indexes/{indexName}/synonyms/{objectID}",
    tag = "synonyms",
    params(
        ("indexName" = String, Path, description = "Index name"),
        ("objectID" = String, Path, description = "Synonym ID")
    ),
    responses(
        (status = 200, description = "Synonym deleted", body = serde_json::Value),
        (status = 404, description = "Synonym not found")
    ),
    security(
        ("api_key" = [])
    )
)]
pub async fn delete_synonym(
    State(state): State<Arc<AppState>>,
    Path((index_name, object_id)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let synonyms_path = state
        .manager
        .base_path
        .join(&index_name)
        .join("synonyms.json");

    if !synonyms_path.exists() {
        return Err((
            StatusCode::NOT_FOUND,
            format!("Synonym {} not found", object_id),
        ));
    }

    let mut store = SynonymStore::load(&synonyms_path)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    store.remove(&object_id).ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            format!("Synonym {} not found", object_id),
        )
    })?;

    store
        .save(&synonyms_path)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    state.manager.invalidate_synonyms_cache(&index_name);

    state.manager.append_oplog(
        &index_name,
        "delete_synonym",
        serde_json::json!({"objectID": object_id}),
    );

    let task = state
        .manager
        .make_noop_task(&index_name)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(serde_json::json!({
        "taskID": task.numeric_id,
        "deletedAt": chrono::Utc::now().to_rfc3339()
    })))
}

/// Batch save multiple synonyms
#[utoipa::path(
    post,
    path = "/1/indexes/{indexName}/synonyms/batch",
    tag = "synonyms",
    params(
        ("indexName" = String, Path, description = "Index name")
    ),
    request_body(content = serde_json::Value, description = "Array of synonyms"),
    responses(
        (status = 200, description = "Synonyms saved", body = serde_json::Value)
    ),
    security(
        ("api_key" = [])
    )
)]
pub async fn save_synonyms(
    State(state): State<Arc<AppState>>,
    Path(index_name): Path<String>,
    Query(params): Query<HashMap<String, String>>,
    Json(synonyms): Json<Vec<Synonym>>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    state
        .manager
        .create_tenant(&index_name)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let synonyms_path = state
        .manager
        .base_path
        .join(&index_name)
        .join("synonyms.json");

    let replace = params
        .get("replaceExistingSynonyms")
        .and_then(|v| v.parse::<bool>().ok())
        .unwrap_or(false);

    let mut store = if replace || !synonyms_path.exists() {
        SynonymStore::new()
    } else {
        SynonymStore::load(&synonyms_path)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    };

    let synonyms_json: Vec<serde_json::Value> = synonyms
        .iter()
        .map(|s| serde_json::to_value(s).unwrap_or_default())
        .collect();

    for syn in synonyms {
        store.insert(syn);
    }

    store
        .save(&synonyms_path)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    state.manager.invalidate_synonyms_cache(&index_name);

    state.manager.append_oplog(
        &index_name,
        "save_synonyms",
        serde_json::json!({"synonyms": synonyms_json, "replace": replace}),
    );

    let task = state
        .manager
        .make_noop_task(&index_name)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(serde_json::json!({
        "taskID": task.numeric_id,
        "updatedAt": chrono::Utc::now().to_rfc3339()
    })))
}

/// Clear all synonyms for an index
#[utoipa::path(
    post,
    path = "/1/indexes/{indexName}/synonyms/clear",
    tag = "synonyms",
    params(
        ("indexName" = String, Path, description = "Index name")
    ),
    responses(
        (status = 200, description = "Synonyms cleared", body = serde_json::Value)
    ),
    security(
        ("api_key" = [])
    )
)]
pub async fn clear_synonyms(
    State(state): State<Arc<AppState>>,
    Path(index_name): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let synonyms_path = state
        .manager
        .base_path
        .join(&index_name)
        .join("synonyms.json");

    if synonyms_path.exists() {
        std::fs::remove_file(&synonyms_path)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    }

    state.manager.invalidate_synonyms_cache(&index_name);
    state
        .manager
        .append_oplog(&index_name, "clear_synonyms", serde_json::json!({}));

    let task = state
        .manager
        .make_noop_task(&index_name)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(serde_json::json!({
        "taskID": task.numeric_id,
        "updatedAt": chrono::Utc::now().to_rfc3339()
    })))
}

#[derive(Deserialize)]
pub struct SearchSynonymsRequest {
    #[serde(default)]
    pub query: String,

    #[serde(rename = "type")]
    pub synonym_type: Option<String>,

    #[serde(default)]
    pub page: usize,

    #[serde(rename = "hitsPerPage", default = "default_hits_per_page")]
    pub hits_per_page: usize,
}

fn default_hits_per_page() -> usize {
    20
}

/// Search for synonyms
#[utoipa::path(
    post,
    path = "/1/indexes/{indexName}/synonyms/search",
    tag = "synonyms",
    params(
        ("indexName" = String, Path, description = "Index name")
    ),
    request_body(content = serde_json::Value, description = "Search parameters"),
    responses(
        (status = 200, description = "Matching synonyms", body = serde_json::Value)
    ),
    security(
        ("api_key" = [])
    )
)]
pub async fn search_synonyms(
    State(state): State<Arc<AppState>>,
    Path(index_name): Path<String>,
    Json(req): Json<SearchSynonymsRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let synonyms_path = state
        .manager
        .base_path
        .join(&index_name)
        .join("synonyms.json");

    let store = if synonyms_path.exists() {
        SynonymStore::load(&synonyms_path)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    } else {
        SynonymStore::new()
    };

    let (hits, total) = store.search(
        &req.query,
        req.synonym_type.as_deref(),
        req.page,
        req.hits_per_page,
    );

    Ok(Json(serde_json::json!({
        "hits": hits,
        "nbHits": total
    })))
}
