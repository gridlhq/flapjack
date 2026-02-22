use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use flapjack::query_suggestions::{build_suggestions_index, QsConfig, QsConfigStore};
use serde_json::json;
use std::sync::Arc;

use super::AppState;

fn store(state: &AppState) -> QsConfigStore {
    QsConfigStore::new(&state.manager.base_path)
}

/// GET /1/configs — list all Query Suggestions configurations
pub async fn list_configs(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match store(&state).list_configs() {
        Ok(configs) => Json(json!(configs)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"message": e.to_string()})),
        )
            .into_response(),
    }
}

/// POST /1/configs — create a new configuration and schedule a build
pub async fn create_config(
    State(state): State<Arc<AppState>>,
    Json(config): Json<QsConfig>,
) -> impl IntoResponse {
    let s = store(&state);

    if s.config_exists(&config.index_name) {
        return (
            StatusCode::CONFLICT,
            Json(json!({
                "status": 409,
                "message": format!("A configuration for '{}' already exists.", config.index_name)
            })),
        )
            .into_response();
    }

    if let Err(e) = s.save_config(&config) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"message": e.to_string()})),
        )
            .into_response();
    }

    // Mark as running and fire off async build
    let mut status = s.load_status(&config.index_name);
    status.is_running = true;
    s.save_status(&status).ok();

    spawn_build(Arc::clone(&state), config.clone());

    (
        StatusCode::OK,
        Json(json!({
            "status": 200,
            "message": "Configuration was created, and a new indexing job has been scheduled."
        })),
    )
        .into_response()
}

/// GET /1/configs/:indexName — get a single configuration
pub async fn get_config(
    State(state): State<Arc<AppState>>,
    Path(index_name): Path<String>,
) -> impl IntoResponse {
    match store(&state).load_config(&index_name) {
        Ok(Some(config)) => Json(json!(config)).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({"message": format!("No configuration found for '{}'.", index_name)})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"message": e.to_string()})),
        )
            .into_response(),
    }
}

/// PUT /1/configs/:indexName — update an existing configuration and rebuild
pub async fn update_config(
    State(state): State<Arc<AppState>>,
    Path(index_name): Path<String>,
    Json(mut config): Json<QsConfig>,
) -> impl IntoResponse {
    let s = store(&state);

    if !s.config_exists(&index_name) {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({"message": format!("No configuration found for '{}'.", index_name)})),
        )
            .into_response();
    }

    // Ensure the indexName in the body matches the path
    config.index_name = index_name;

    if let Err(e) = s.save_config(&config) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"message": e.to_string()})),
        )
            .into_response();
    }

    // Guard against concurrent builds: two simultaneous builds on the same staging
    // index would corrupt each other (both writing to {indexName}__building).
    let status = s.load_status(&config.index_name);
    if status.is_running {
        return (
            StatusCode::CONFLICT,
            Json(json!({"message": "A build is already in progress. Wait for it to finish before updating."})),
        )
            .into_response();
    }

    let mut new_status = status;
    new_status.is_running = true;
    s.save_status(&new_status).ok();

    spawn_build(Arc::clone(&state), config);

    (
        StatusCode::OK,
        Json(json!({
            "status": 200,
            "message": "Configuration was updated, and a new indexing job has been scheduled."
        })),
    )
        .into_response()
}

/// DELETE /1/configs/:indexName — delete configuration (does NOT delete the suggestions index)
pub async fn delete_config(
    State(state): State<Arc<AppState>>,
    Path(index_name): Path<String>,
) -> impl IntoResponse {
    match store(&state).delete_config(&index_name) {
        Ok(true) => (
            StatusCode::OK,
            Json(json!({
                "status": 200,
                "message": "Configuration was deleted with success."
            })),
        )
            .into_response(),
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(json!({"message": format!("No configuration found for '{}'.", index_name)})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"message": e.to_string()})),
        )
            .into_response(),
    }
}

/// GET /1/configs/:indexName/status — build status
pub async fn get_status(
    State(state): State<Arc<AppState>>,
    Path(index_name): Path<String>,
) -> impl IntoResponse {
    if !store(&state).config_exists(&index_name) {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({"message": format!("No configuration found for '{}'.", index_name)})),
        )
            .into_response();
    }
    let status = store(&state).load_status(&index_name);
    Json(json!(status)).into_response()
}

/// GET /1/logs/:indexName — build logs
pub async fn get_logs(
    State(state): State<Arc<AppState>>,
    Path(index_name): Path<String>,
) -> impl IntoResponse {
    let logs = store(&state).read_logs(&index_name);
    Json(json!(logs)).into_response()
}

/// POST /1/configs/:indexName/build — trigger an immediate rebuild (Flapjack extension)
pub async fn trigger_build(
    State(state): State<Arc<AppState>>,
    Path(index_name): Path<String>,
) -> impl IntoResponse {
    let s = store(&state);

    let config = match s.load_config(&index_name) {
        Ok(Some(c)) => c,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({"message": format!("No configuration found for '{}'.", index_name)})),
            )
                .into_response()
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"message": e.to_string()})),
            )
                .into_response()
        }
    };

    let status = s.load_status(&index_name);
    if status.is_running {
        return (
            StatusCode::CONFLICT,
            Json(json!({"message": "A build is already in progress for this configuration."})),
        )
            .into_response();
    }

    let mut new_status = status;
    new_status.is_running = true;
    s.save_status(&new_status).ok();

    spawn_build(Arc::clone(&state), config);

    (
        StatusCode::OK,
        Json(json!({"status": 200, "message": "Build triggered."})),
    )
        .into_response()
}

/// Spawn a background build task.
fn spawn_build(state: Arc<AppState>, config: QsConfig) {
    let manager = Arc::clone(&state.manager);
    let analytics_engine = state.analytics_engine.clone();
    let base_path = state.manager.base_path.clone();

    tokio::spawn(async move {
        let store = QsConfigStore::new(&base_path);

        let engine = match analytics_engine {
            Some(e) => e,
            None => {
                tracing::warn!(
                    "[query-suggestions] Build skipped for '{}': analytics engine not initialized",
                    config.index_name
                );
                let mut status = store.load_status(&config.index_name);
                status.is_running = false;
                store.save_status(&status).ok();
                return;
            }
        };

        match build_suggestions_index(&config, &store, &manager, &engine).await {
            Ok(count) => tracing::info!(
                "[query-suggestions] Build complete for '{}': {} suggestions",
                config.index_name,
                count
            ),
            Err(e) => {
                tracing::error!(
                    "[query-suggestions] Build failed for '{}': {}",
                    config.index_name,
                    e
                );
                let mut status = store.load_status(&config.index_name);
                status.is_running = false;
                store.save_status(&status).ok();
            }
        }
    });
}
