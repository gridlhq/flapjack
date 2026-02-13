use axum::{
    extract::{Path, State},
    Json,
};
use serde::Serialize;
use std::sync::Arc;
use utoipa::ToSchema;

use super::AppState;
use flapjack::error::FlapjackError;
use flapjack::types::TaskStatus;

#[derive(Debug, Serialize, ToSchema)]
pub struct TaskResponse {
    pub task_uid: String,
    pub status: String,
    pub received_documents: usize,
    pub indexed_documents: usize,
    pub rejected_documents: Vec<DocFailureDto>,
    pub rejected_count: usize,
    pub error: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct DocFailureDto {
    pub doc_id: String,
    pub error: String,
    pub message: String,
}

/// Get task status by ID
#[utoipa::path(
    get,
    path = "/1/tasks/{task_id}",
    tag = "tasks",
    params(
        ("task_id" = String, Path, description = "Task ID")
    ),
    responses(
        (status = 200, description = "Task status and results", body = TaskResponse),
        (status = 404, description = "Task not found")
    ),
    security(
        ("api_key" = [])
    )
)]
pub async fn get_task(
    State(state): State<Arc<AppState>>,
    Path(task_id): Path<String>,
) -> Result<Json<TaskResponse>, FlapjackError> {
    let task = state.manager.get_task(&task_id)?;

    let status_str = match &task.status {
        TaskStatus::Enqueued | TaskStatus::Processing => "notPublished",
        TaskStatus::Succeeded => "published",
        TaskStatus::Failed(_) => "error",
    };

    let error = match &task.status {
        TaskStatus::Failed(e) => Some(e.clone()),
        _ => None,
    };

    let rejected_docs = task
        .rejected_documents
        .iter()
        .map(|df| DocFailureDto {
            doc_id: df.doc_id.clone(),
            error: df.error.clone(),
            message: df.message.clone(),
        })
        .collect();

    Ok(Json(TaskResponse {
        task_uid: task.id,
        status: status_str.to_string(),
        received_documents: task.received_documents,
        indexed_documents: task.indexed_documents,
        rejected_documents: rejected_docs,
        rejected_count: task.rejected_count,
        error,
    }))
}

/// Get task status for a specific index
#[utoipa::path(
    get,
    path = "/1/indexes/{indexName}/task/{task_id}",
    tag = "tasks",
    params(
        ("indexName" = String, Path, description = "Index name"),
        ("task_id" = String, Path, description = "Task ID")
    ),
    responses(
        (status = 200, description = "Task status and results", body = TaskResponse),
        (status = 404, description = "Task not found")
    ),
    security(
        ("api_key" = [])
    )
)]
pub async fn get_task_for_index(
    State(state): State<Arc<AppState>>,
    Path((index_name, task_id)): Path<(String, String)>,
) -> Result<Json<TaskResponse>, FlapjackError> {
    let full_task_id = if task_id.starts_with("task_") {
        task_id.clone()
    } else {
        format!("task_{}_{}", index_name, task_id)
    };

    let task = state
        .manager
        .get_task(&full_task_id)
        .or_else(|_| state.manager.get_task(&task_id))?;

    if let Some(extracted_index) = task
        .id
        .strip_prefix("task_")
        .and_then(|s| s.split('_').next())
    {
        if extracted_index != index_name {
            return Err(FlapjackError::TaskNotFound(task_id));
        }
    }

    let status_str = match &task.status {
        TaskStatus::Enqueued | TaskStatus::Processing => "notPublished",
        TaskStatus::Succeeded => "published",
        TaskStatus::Failed(_) => "error",
    };

    let error = match &task.status {
        TaskStatus::Failed(e) => Some(e.clone()),
        _ => None,
    };

    let rejected_docs = task
        .rejected_documents
        .iter()
        .map(|df| DocFailureDto {
            doc_id: df.doc_id.clone(),
            error: df.error.clone(),
            message: df.message.clone(),
        })
        .collect();

    Ok(Json(TaskResponse {
        task_uid: task.id,
        status: status_str.to_string(),
        received_documents: task.received_documents,
        indexed_documents: task.indexed_documents,
        rejected_documents: rejected_docs,
        rejected_count: task.rejected_count,
        error,
    }))
}
