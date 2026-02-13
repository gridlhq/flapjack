use super::AppState;
use axum::{
    body::Bytes,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use flapjack::index::s3::S3Config;
use flapjack::index::snapshot::{export_to_bytes, import_from_bytes};
use std::sync::Arc;

/// Export index as downloadable snapshot
#[utoipa::path(
    get,
    path = "/1/indexes/{indexName}/export",
    tag = "snapshots",
    params(
        ("indexName" = String, Path, description = "Index name")
    ),
    responses(
        (status = 200, description = "Snapshot file", body = Vec<u8>),
        (status = 404, description = "Index not found")
    ),
    security(
        ("api_key" = [])
    )
)]
pub async fn export_snapshot(
    State(state): State<Arc<AppState>>,
    Path(index_name): Path<String>,
) -> impl IntoResponse {
    let index_path = state.manager.base_path.join(&index_name);
    if !index_path.exists() {
        return (StatusCode::NOT_FOUND, "Index not found").into_response();
    }

    match export_to_bytes(&index_path) {
        Ok(bytes) => {
            let headers = [
                ("Content-Type", "application/gzip"),
                (
                    "Content-Disposition",
                    &format!("attachment; filename=\"{}.tar.gz\"", index_name),
                ),
            ];
            (headers, bytes).into_response()
        }
        Err(e) => {
            tracing::error!("Export failed: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Export failed: {}", e),
            )
                .into_response()
        }
    }
}

/// Import index from uploaded snapshot
#[utoipa::path(
    post,
    path = "/1/indexes/{indexName}/import",
    tag = "snapshots",
    params(
        ("indexName" = String, Path, description = "Index name")
    ),
    request_body(content = Vec<u8>, description = "Snapshot tar.gz file"),
    responses(
        (status = 200, description = "Import successful", body = serde_json::Value),
        (status = 500, description = "Import failed")
    ),
    security(
        ("api_key" = [])
    )
)]
pub async fn import_snapshot(
    State(state): State<Arc<AppState>>,
    Path(index_name): Path<String>,
    body: Bytes,
) -> impl IntoResponse {
    let index_path = state.manager.base_path.join(&index_name);

    if index_path.exists() {
        if let Err(e) = std::fs::remove_dir_all(&index_path) {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to clear existing index: {}", e),
            )
                .into_response();
        }
    }

    match import_from_bytes(&body, &index_path) {
        Ok(()) => {
            state.manager.unload_tenant(&index_name);
            (StatusCode::OK, r#"{"status":"imported"}"#).into_response()
        }
        Err(e) => {
            tracing::error!("Import failed: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Import failed: {}", e),
            )
                .into_response()
        }
    }
}

/// Upload index snapshot to S3
#[utoipa::path(
    post,
    path = "/1/indexes/{indexName}/snapshot",
    tag = "snapshots",
    params(
        ("indexName" = String, Path, description = "Index name")
    ),
    responses(
        (status = 200, description = "Snapshot uploaded to S3", body = serde_json::Value),
        (status = 400, description = "S3 not configured"),
        (status = 404, description = "Index not found")
    ),
    security(
        ("api_key" = [])
    )
)]
pub async fn snapshot_to_s3(
    State(state): State<Arc<AppState>>,
    Path(index_name): Path<String>,
) -> impl IntoResponse {
    let s3_config =
        match S3Config::from_env() {
            Some(c) => c,
            None => return (
                StatusCode::SERVICE_UNAVAILABLE,
                r#"{"error":"S3 not configured. Set FLAPJACK_S3_BUCKET and FLAPJACK_S3_REGION."}"#,
            )
                .into_response(),
        };

    let index_path = state.manager.base_path.join(&index_name);
    if !index_path.exists() {
        return (StatusCode::NOT_FOUND, r#"{"error":"Index not found"}"#).into_response();
    }

    let bytes = match export_to_bytes(&index_path) {
        Ok(b) => b,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!(r#"{{"error":"Export failed: {}"}}"#, e),
            )
                .into_response()
        }
    };

    match flapjack::index::s3::upload_snapshot(&s3_config, &index_name, &bytes).await {
        Ok(key) => {
            let retention = std::env::var("FLAPJACK_SNAPSHOT_RETENTION")
                .ok()
                .and_then(|v| v.parse::<usize>().ok())
                .unwrap_or(24);
            let _ =
                flapjack::index::s3::enforce_retention(&s3_config, &index_name, retention).await;

            Json(serde_json::json!({
                "status": "uploaded",
                "key": key,
                "size_bytes": bytes.len(),
            }))
            .into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!(r#"{{"error":"S3 upload failed: {}"}}"#, e),
        )
            .into_response(),
    }
}

/// Restore index from S3 snapshot
#[utoipa::path(
    post,
    path = "/1/indexes/{indexName}/restore",
    tag = "snapshots",
    params(
        ("indexName" = String, Path, description = "Index name")
    ),
    request_body(content = serde_json::Value, description = "Restore options with snapshot ID"),
    responses(
        (status = 200, description = "Restore successful", body = serde_json::Value),
        (status = 400, description = "S3 not configured"),
        (status = 404, description = "Snapshot not found")
    ),
    security(
        ("api_key" = [])
    )
)]
pub async fn restore_from_s3(
    State(state): State<Arc<AppState>>,
    Path(index_name): Path<String>,
    body: Option<Json<serde_json::Value>>,
) -> impl IntoResponse {
    let s3_config = match S3Config::from_env() {
        Some(c) => c,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                r#"{"error":"S3 not configured"}"#,
            )
                .into_response()
        }
    };

    let key_override = body.and_then(|b| b.get("key").and_then(|v| v.as_str()).map(String::from));

    let (key, data) = if let Some(key) = key_override {
        let data = match flapjack::index::s3::download_snapshot(&s3_config, &key).await {
            Ok(d) => d,
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!(r#"{{"error":"S3 download failed: {}"}}"#, e),
                )
                    .into_response()
            }
        };
        (key, data)
    } else {
        match flapjack::index::s3::download_latest_snapshot(&s3_config, &index_name).await {
            Ok(r) => r,
            Err(e) => {
                return (StatusCode::NOT_FOUND, format!(r#"{{"error":"{}"}}"#, e)).into_response()
            }
        }
    };

    let index_path = state.manager.base_path.join(&index_name);
    if index_path.exists() {
        let _ = std::fs::remove_dir_all(&index_path);
    }

    match import_from_bytes(&data, &index_path) {
        Ok(()) => {
            state.manager.unload_tenant(&index_name);
            Json(serde_json::json!({
                "status": "restored",
                "key": key,
                "size_bytes": data.len(),
            }))
            .into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!(r#"{{"error":"Restore failed: {}"}}"#, e),
        )
            .into_response(),
    }
}

/// List available S3 snapshots for an index
#[utoipa::path(
    get,
    path = "/1/indexes/{indexName}/snapshots",
    tag = "snapshots",
    params(
        ("indexName" = String, Path, description = "Index name")
    ),
    responses(
        (status = 200, description = "List of snapshots", body = serde_json::Value),
        (status = 400, description = "S3 not configured")
    ),
    security(
        ("api_key" = [])
    )
)]
pub async fn list_s3_snapshots(Path(index_name): Path<String>) -> impl IntoResponse {
    let s3_config = match S3Config::from_env() {
        Some(c) => c,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                r#"{"error":"S3 not configured"}"#,
            )
                .into_response()
        }
    };

    match flapjack::index::s3::list_snapshots(&s3_config, &index_name).await {
        Ok(keys) => Json(serde_json::json!({ "snapshots": keys })).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!(r#"{{"error":"{}"}}"#, e),
        )
            .into_response(),
    }
}
