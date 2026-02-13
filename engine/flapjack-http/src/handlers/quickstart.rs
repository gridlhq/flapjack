//! Quickstart API: simple, no-auth convenience endpoints for local development.
//!
//! These endpoints follow industry-standard REST patterns (similar to Meilisearch)
//! and require no authentication or Content-Type headers. They are thin wrappers
//! that delegate to the same internal logic as the Algolia-compatible API.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use std::sync::Arc;

use super::settings::SetSettingsRequest;
use super::AppState;
use crate::dto::{AddDocumentsRequest, AddDocumentsResponse, BatchOperation, SearchRequest};
use flapjack::error::FlapjackError;

// ---------------------------------------------------------------------------
// Search
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct QsSearchParams {
    pub q: Option<String>,
    #[serde(rename = "hitsPerPage")]
    pub hits_per_page: Option<usize>,
    pub page: Option<usize>,
    pub filters: Option<String>,
    /// Comma-separated list of facet names, or a JSON array string.
    pub facets: Option<String>,
    /// Comma-separated sort specs, e.g. "price:asc,name:desc".
    pub sort: Option<String>,
}

/// `GET /indexes/:indexName/search?q=...`
pub async fn qs_search_get(
    State(state): State<Arc<AppState>>,
    Path(index_name): Path<String>,
    Query(params): Query<QsSearchParams>,
) -> Result<Json<serde_json::Value>, FlapjackError> {
    let facets = params.facets.map(|f| {
        serde_json::from_str::<Vec<String>>(&f)
            .unwrap_or_else(|_| f.split(',').map(|s| s.trim().to_string()).collect())
    });

    let sort = params.sort.map(|s| {
        s.split(',')
            .map(|s| s.trim().to_string())
            .collect::<Vec<_>>()
    });

    let req = SearchRequest {
        query: params.q.unwrap_or_default(),
        hits_per_page: params.hits_per_page,
        page: params.page.unwrap_or(0),
        filters: params.filters,
        facets,
        sort,
        ..Default::default()
    };

    super::search::search_single(State(state), index_name, req).await
}

/// `POST /indexes/:indexName/search` — complex search via JSON body.
pub async fn qs_search_post(
    State(state): State<Arc<AppState>>,
    Path(index_name): Path<String>,
    Json(mut req): Json<SearchRequest>,
) -> Result<Json<serde_json::Value>, FlapjackError> {
    req.apply_params_string();
    super::search::search_single(State(state), index_name, req).await
}

// ---------------------------------------------------------------------------
// Documents
// ---------------------------------------------------------------------------

/// `POST /indexes/:indexName/documents` — add documents.
///
/// Accepts a JSON array `[{...}, {...}]` or a single JSON object `{...}`.
/// Auto-generates `objectID` (UUID v4) if not present.
pub async fn qs_add_documents(
    State(state): State<Arc<AppState>>,
    Path(index_name): Path<String>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<AddDocumentsResponse>, FlapjackError> {
    let docs: Vec<serde_json::Map<String, serde_json::Value>> = match body {
        serde_json::Value::Array(arr) => arr
            .into_iter()
            .map(|v| match v {
                serde_json::Value::Object(obj) => Ok(obj),
                _ => Err(FlapjackError::InvalidQuery(
                    "Array elements must be JSON objects".to_string(),
                )),
            })
            .collect::<Result<Vec<_>, _>>()?,
        serde_json::Value::Object(obj) => vec![obj],
        _ => {
            return Err(FlapjackError::InvalidQuery(
                "Expected JSON array or object".to_string(),
            ));
        }
    };

    let operations: Vec<BatchOperation> = docs
        .into_iter()
        .map(|obj| {
            let mut body: std::collections::HashMap<String, serde_json::Value> =
                obj.into_iter().collect();
            if !body.contains_key("objectID") {
                body.insert(
                    "objectID".to_string(),
                    serde_json::Value::String(uuid::Uuid::new_v4().to_string()),
                );
            }
            BatchOperation {
                action: "addObject".to_string(),
                body,
                create_if_not_exists: None,
            }
        })
        .collect();

    let req = AddDocumentsRequest::Batch {
        requests: operations,
    };
    super::objects::add_documents_batch_impl(State(state), index_name, req).await
}

/// `GET /indexes/:indexName/documents/:docId` — get a document by ID.
pub async fn qs_get_document(
    State(state): State<Arc<AppState>>,
    Path((index_name, doc_id)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    super::objects::get_object(State(state), Path((index_name, doc_id))).await
}

/// `DELETE /indexes/:indexName/documents/:docId` — delete a document.
pub async fn qs_delete_document(
    State(state): State<Arc<AppState>>,
    Path((index_name, doc_id)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, FlapjackError> {
    super::objects::delete_object(State(state), Path((index_name, doc_id))).await
}

// ---------------------------------------------------------------------------
// Indexes
// ---------------------------------------------------------------------------

/// `GET /indexes` — list all indexes.
pub async fn qs_list_indexes(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, FlapjackError> {
    super::indices::list_indices(State(state)).await
}

/// `DELETE /indexes/:indexName` — delete an index.
pub async fn qs_delete_index(
    State(state): State<Arc<AppState>>,
    Path(index_name): Path<String>,
) -> Result<Json<serde_json::Value>, FlapjackError> {
    super::indices::delete_index(State(state), Path(index_name)).await
}

// ---------------------------------------------------------------------------
// Tasks
// ---------------------------------------------------------------------------

/// `GET /tasks/:taskId` — get task status.
pub async fn qs_get_task(
    State(state): State<Arc<AppState>>,
    Path(task_id): Path<String>,
) -> Result<Json<super::tasks::TaskResponse>, FlapjackError> {
    super::tasks::get_task(State(state), Path(task_id)).await
}

// ---------------------------------------------------------------------------
// Migration
// ---------------------------------------------------------------------------

/// `POST /migrate` — migrate an index from Algolia.
pub async fn qs_migrate(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<super::migration::MigrateFromAlgoliaRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    super::migration::migrate_from_algolia(State(state), Json(payload)).await
}

// ---------------------------------------------------------------------------
// Bulk Delete
// ---------------------------------------------------------------------------

/// `POST /indexes/:indexName/documents/delete` — delete multiple documents by ID.
///
/// Accepts a JSON array of document IDs: `["id1", "id2", "id3"]`.
pub async fn qs_delete_documents(
    State(state): State<Arc<AppState>>,
    Path(index_name): Path<String>,
    Json(ids): Json<Vec<String>>,
) -> Result<Json<serde_json::Value>, FlapjackError> {
    if ids.is_empty() {
        let task = state.manager.make_noop_task(&index_name)?;
        return Ok(Json(serde_json::json!({
            "taskID": task.numeric_id,
            "deletedAt": chrono::Utc::now().to_rfc3339()
        })));
    }

    state
        .manager
        .delete_documents_sync(&index_name, ids)
        .await?;

    let task = state.manager.make_noop_task(&index_name)?;
    Ok(Json(serde_json::json!({
        "taskID": task.numeric_id,
        "deletedAt": chrono::Utc::now().to_rfc3339()
    })))
}

// ---------------------------------------------------------------------------
// Settings
// ---------------------------------------------------------------------------

/// `GET /indexes/:indexName/settings` — get index settings.
pub async fn qs_get_settings(
    State(state): State<Arc<AppState>>,
    Path(index_name): Path<String>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    super::settings::get_settings(State(state), Path(index_name)).await
}

/// `PUT /indexes/:indexName/settings` — update index settings.
pub async fn qs_set_settings(
    State(state): State<Arc<AppState>>,
    Path(index_name): Path<String>,
    Json(payload): Json<SetSettingsRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    super::settings::set_settings(State(state), Path(index_name), Json(payload)).await
}
