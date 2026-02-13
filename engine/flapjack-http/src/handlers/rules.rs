use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;

use super::AppState;
use flapjack::index::rules::{Rule, RuleStore};

/// Get a rule by ID
#[utoipa::path(
    get,
    path = "/1/indexes/{indexName}/rules/{objectID}",
    tag = "rules",
    params(
        ("indexName" = String, Path, description = "Index name"),
        ("objectID" = String, Path, description = "Rule ID")
    ),
    responses(
        (status = 200, description = "Rule retrieved", body = serde_json::Value),
        (status = 404, description = "Rule not found")
    ),
    security(
        ("api_key" = [])
    )
)]
pub async fn get_rule(
    State(state): State<Arc<AppState>>,
    Path((index_name, object_id)): Path<(String, String)>,
) -> Result<Json<Rule>, (StatusCode, String)> {
    let rules_path = state.manager.base_path.join(&index_name).join("rules.json");

    if !rules_path.exists() {
        return Err((
            StatusCode::NOT_FOUND,
            format!("Rule {} not found", object_id),
        ));
    }

    let store = RuleStore::load(&rules_path)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    store
        .get(&object_id)
        .cloned()
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                format!("Rule {} not found", object_id),
            )
        })
        .map(Json)
}

/// Create or update a rule
#[utoipa::path(
    put,
    path = "/1/indexes/{indexName}/rules/{objectID}",
    tag = "rules",
    params(
        ("indexName" = String, Path, description = "Index name"),
        ("objectID" = String, Path, description = "Rule ID")
    ),
    request_body(content = serde_json::Value, description = "Rule data"),
    responses(
        (status = 200, description = "Rule saved", body = serde_json::Value)
    ),
    security(
        ("api_key" = [])
    )
)]
pub async fn save_rule(
    State(state): State<Arc<AppState>>,
    Path((index_name, _object_id)): Path<(String, String)>,
    Json(rule): Json<Rule>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    state
        .manager
        .create_tenant(&index_name)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let rules_path = state.manager.base_path.join(&index_name).join("rules.json");

    let mut store = if rules_path.exists() {
        RuleStore::load(&rules_path)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    } else {
        RuleStore::new()
    };

    store.insert(rule.clone());

    store
        .save(&rules_path)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    state.manager.invalidate_rules_cache(&index_name);

    state.manager.append_oplog(
        &index_name,
        "save_rule",
        serde_json::to_value(&rule).unwrap_or_default(),
    );

    let task = state
        .manager
        .make_noop_task(&index_name)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(serde_json::json!({
        "taskID": task.numeric_id,
        "updatedAt": chrono::Utc::now().to_rfc3339(),
        "id": rule.object_id
    })))
}

/// Delete a rule by ID
#[utoipa::path(
    delete,
    path = "/1/indexes/{indexName}/rules/{objectID}",
    tag = "rules",
    params(
        ("indexName" = String, Path, description = "Index name"),
        ("objectID" = String, Path, description = "Rule ID")
    ),
    responses(
        (status = 200, description = "Rule deleted", body = serde_json::Value),
        (status = 404, description = "Rule not found")
    ),
    security(
        ("api_key" = [])
    )
)]
pub async fn delete_rule(
    State(state): State<Arc<AppState>>,
    Path((index_name, object_id)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let rules_path = state.manager.base_path.join(&index_name).join("rules.json");

    if !rules_path.exists() {
        return Err((
            StatusCode::NOT_FOUND,
            format!("Rule {} not found", object_id),
        ));
    }

    let mut store = RuleStore::load(&rules_path)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    store.remove(&object_id).ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            format!("Rule {} not found", object_id),
        )
    })?;

    store
        .save(&rules_path)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    state.manager.invalidate_rules_cache(&index_name);

    state.manager.append_oplog(
        &index_name,
        "delete_rule",
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

/// Batch save multiple rules
#[utoipa::path(
    post,
    path = "/1/indexes/{indexName}/rules/batch",
    tag = "rules",
    params(
        ("indexName" = String, Path, description = "Index name")
    ),
    request_body(content = serde_json::Value, description = "Array of rules"),
    responses(
        (status = 200, description = "Rules saved", body = serde_json::Value)
    ),
    security(
        ("api_key" = [])
    )
)]
pub async fn save_rules(
    State(state): State<Arc<AppState>>,
    Path(index_name): Path<String>,
    Query(params): Query<HashMap<String, String>>,
    Json(rules): Json<Vec<Rule>>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    state
        .manager
        .create_tenant(&index_name)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let rules_path = state.manager.base_path.join(&index_name).join("rules.json");

    let clear_existing = params
        .get("clearExistingRules")
        .and_then(|v| v.parse::<bool>().ok())
        .unwrap_or(false);

    let mut store = if clear_existing || !rules_path.exists() {
        RuleStore::new()
    } else {
        RuleStore::load(&rules_path)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    };

    let rules_json: Vec<serde_json::Value> = rules
        .iter()
        .map(|r| serde_json::to_value(r).unwrap_or_default())
        .collect();

    for rule in rules {
        store.insert(rule);
    }

    store
        .save(&rules_path)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    state.manager.invalidate_rules_cache(&index_name);

    state.manager.append_oplog(
        &index_name,
        "save_rules",
        serde_json::json!({"rules": rules_json, "clearExisting": clear_existing}),
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

/// Clear all rules for an index
#[utoipa::path(
    post,
    path = "/1/indexes/{indexName}/rules/clear",
    tag = "rules",
    params(
        ("indexName" = String, Path, description = "Index name")
    ),
    responses(
        (status = 200, description = "Rules cleared", body = serde_json::Value)
    ),
    security(
        ("api_key" = [])
    )
)]
pub async fn clear_rules(
    State(state): State<Arc<AppState>>,
    Path(index_name): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let rules_path = state.manager.base_path.join(&index_name).join("rules.json");

    if rules_path.exists() {
        std::fs::remove_file(&rules_path)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    }

    state.manager.invalidate_rules_cache(&index_name);
    state
        .manager
        .append_oplog(&index_name, "clear_rules", serde_json::json!({}));

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
pub struct SearchRulesRequest {
    #[serde(default)]
    pub query: String,

    #[serde(default)]
    pub page: usize,

    #[serde(rename = "hitsPerPage", default = "default_hits_per_page")]
    pub hits_per_page: usize,
}

fn default_hits_per_page() -> usize {
    20
}

/// Search for rules
#[utoipa::path(
    post,
    path = "/1/indexes/{indexName}/rules/search",
    tag = "rules",
    params(
        ("indexName" = String, Path, description = "Index name")
    ),
    request_body(content = serde_json::Value, description = "Search parameters"),
    responses(
        (status = 200, description = "Matching rules", body = serde_json::Value)
    ),
    security(
        ("api_key" = [])
    )
)]
pub async fn search_rules(
    State(state): State<Arc<AppState>>,
    Path(index_name): Path<String>,
    Json(req): Json<SearchRulesRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let rules_path = state.manager.base_path.join(&index_name).join("rules.json");

    let store = if rules_path.exists() {
        RuleStore::load(&rules_path)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    } else {
        RuleStore::new()
    };

    let (hits, total) = store.search(&req.query, req.page, req.hits_per_page);
    let nb_pages = total.div_ceil(req.hits_per_page);

    Ok(Json(serde_json::json!({
        "hits": hits,
        "nbHits": total,
        "page": req.page,
        "nbPages": nb_pages
    })))
}
