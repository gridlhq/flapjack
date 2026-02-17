use axum::{
    extract::{Path, State},
    Json,
};
use std::sync::Arc;

use super::AppState;
use crate::dto::CreateIndexRequest;
use flapjack::error::FlapjackError;

/// Recursively compute total size of all files in a directory.
fn dir_size(path: &std::path::Path) -> u64 {
    let mut total = 0u64;
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            let ft = match entry.file_type() {
                Ok(ft) => ft,
                Err(_) => continue,
            };
            if ft.is_file() {
                total += entry.metadata().map(|m| m.len()).unwrap_or(0);
            } else if ft.is_dir() {
                total += dir_size(&entry.path());
            }
        }
    }
    total
}

#[derive(serde::Serialize, utoipa::ToSchema)]
pub struct CreateIndexResponse {
    pub uid: String,
    pub created_at: String,
}

/// Create a new index
#[utoipa::path(
    post,
    path = "/1/indexes",
    tag = "indices",
    request_body = CreateIndexRequest,
    responses(
        (status = 200, description = "Index created successfully", body = CreateIndexResponse),
        (status = 400, description = "Invalid request"),
        (status = 409, description = "Index already exists")
    ),
    security(
        ("api_key" = [])
    )
)]
pub async fn create_index(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateIndexRequest>,
) -> Result<Json<CreateIndexResponse>, FlapjackError> {
    state.manager.create_tenant(&req.uid)?;

    Ok(Json(CreateIndexResponse {
        uid: req.uid,
        created_at: chrono::Utc::now().to_rfc3339(),
    }))
}

/// Delete an index
#[utoipa::path(
    delete,
    path = "/1/indexes/{indexName}",
    tag = "indices",
    params(
        ("indexName" = String, Path, description = "Index name to delete")
    ),
    responses(
        (status = 200, description = "Index deleted successfully", body = serde_json::Value),
        (status = 404, description = "Index not found")
    ),
    security(
        ("api_key" = [])
    )
)]
pub async fn delete_index(
    State(state): State<Arc<AppState>>,
    Path(index_name): Path<String>,
) -> Result<Json<serde_json::Value>, FlapjackError> {
    state.manager.delete_tenant(&index_name).await?;
    let task = state.manager.make_noop_task(&index_name)?;
    Ok(Json(serde_json::json!({
        "taskID": task.numeric_id,
        "deletedAt": chrono::Utc::now().to_rfc3339()
    })))
}

/// List all indices
#[utoipa::path(
    get,
    path = "/1/indexes",
    tag = "indices",
    responses(
        (status = 200, description = "List of all indices", body = serde_json::Value)
    ),
    security(
        ("api_key" = [])
    )
)]
pub async fn list_indices(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, FlapjackError> {
    let mut items = Vec::new();

    for entry in std::fs::read_dir(&state.manager.base_path)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }

        let name = entry.file_name().to_string_lossy().to_string();
        let index_path = entry.path();
        let size = dir_size(&index_path);
        tracing::debug!(index = %name, path = ?index_path, bytes = size, "Index directory size");

        let entries = match state.manager.get_or_load(&name) {
            Ok(index) => {
                let reader = index.reader();
                let searcher = reader.searcher();
                searcher.num_docs()
            }
            Err(e) => {
                tracing::warn!("Failed to load index {}: {}", name, e);
                continue;
            }
        };

        let pending = state.manager.pending_task_count(&name);

        items.push(serde_json::json!({
            "name": name,
            "createdAt": "2024-01-01T00:00:00Z",
            "updatedAt": chrono::Utc::now().to_rfc3339(),
            "entries": entries,
            "dataSize": size,
            "fileSize": size,
            "numberOfPendingTasks": pending,
            "pendingTask": pending > 0
        }));
    }

    Ok(Json(serde_json::json!({
        "items": items,
        "nbPages": 1
    })))
}

/// Clear all documents from an index
#[utoipa::path(
    post,
    path = "/1/indexes/{indexName}/clear",
    tag = "indices",
    params(
        ("indexName" = String, Path, description = "Index name to clear")
    ),
    responses(
        (status = 200, description = "Index cleared successfully", body = serde_json::Value),
        (status = 404, description = "Index not found")
    ),
    security(
        ("api_key" = [])
    )
)]
pub async fn clear_index(
    State(state): State<Arc<AppState>>,
    Path(index_name): Path<String>,
) -> Result<Json<serde_json::Value>, FlapjackError> {
    let index_path = state.manager.base_path.join(&index_name);
    let settings_path = index_path.join("settings.json");
    let relevance_path = index_path.join("relevance.json");

    // Preserve settings and relevance config before clearing
    let settings = if settings_path.exists() {
        Some(std::fs::read(&settings_path)?)
    } else {
        None
    };
    let relevance = if relevance_path.exists() {
        Some(std::fs::read(&relevance_path)?)
    } else {
        None
    };

    // Use delete_tenant (which awaits the write queue) instead of
    // unload + remove_dir_all to avoid race conditions.
    state.manager.delete_tenant(&index_name).await?;
    state.manager.create_tenant(&index_name)?;

    // Restore settings and relevance config
    if let Some(data) = settings {
        std::fs::write(&settings_path, data)?;
    }
    if let Some(data) = relevance {
        std::fs::write(&relevance_path, data)?;
    }

    let task = state.manager.make_noop_task(&index_name)?;
    Ok(Json(serde_json::json!({
        "taskID": task.numeric_id,
        "updatedAt": chrono::Utc::now().to_rfc3339()
    })))
}

/// Compact an index (merge segments and reclaim disk space)
#[utoipa::path(
    post,
    path = "/1/indexes/{indexName}/compact",
    tag = "indices",
    params(
        ("indexName" = String, Path, description = "Index name to compact")
    ),
    responses(
        (status = 200, description = "Compaction started", body = serde_json::Value),
        (status = 404, description = "Index not found")
    ),
    security(
        ("api_key" = [])
    )
)]
pub async fn compact_index(
    State(state): State<Arc<AppState>>,
    Path(index_name): Path<String>,
) -> Result<Json<serde_json::Value>, FlapjackError> {
    let task = state.manager.compact_index(&index_name)?;
    Ok(Json(serde_json::json!({
        "taskID": task.numeric_id,
        "updatedAt": chrono::Utc::now().to_rfc3339()
    })))
}

#[derive(serde::Deserialize, utoipa::ToSchema)]
pub struct OperationIndexRequest {
    pub operation: String,
    pub destination: String,
    pub scope: Option<Vec<String>>,
}

/// Move or copy an index
#[utoipa::path(
    post,
    path = "/1/indexes/{indexName}/operation",
    tag = "indices",
    params(
        ("indexName" = String, Path, description = "Source index name")
    ),
    request_body = OperationIndexRequest,
    responses(
        (status = 200, description = "Operation completed successfully", body = serde_json::Value),
        (status = 400, description = "Invalid operation"),
        (status = 404, description = "Index not found")
    ),
    security(
        ("api_key" = [])
    )
)]
pub async fn operation_index(
    State(state): State<Arc<AppState>>,
    Path(index_name): Path<String>,
    Json(req): Json<OperationIndexRequest>,
) -> Result<Json<serde_json::Value>, FlapjackError> {
    let task = match req.operation.as_str() {
        "move" => {
            state
                .manager
                .move_index(&index_name, &req.destination)
                .await?
        }
        "copy" => {
            state
                .manager
                .copy_index(&index_name, &req.destination, req.scope.as_deref())
                .await?
        }
        _ => {
            return Err(FlapjackError::InvalidQuery(format!(
                "Unknown operation: {}",
                req.operation
            )))
        }
    };

    Ok(Json(serde_json::json!({
        "taskID": task.numeric_id,
        "updatedAt": chrono::Utc::now().to_rfc3339()
    })))
}
