use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

use super::AppState;
use flapjack::index::settings::{
    detect_embedder_changes, DistinctValue, EmbedderChange, IndexMode, IndexSettings,
    SemanticSearchSettings,
};

#[derive(Debug, Serialize, Deserialize)]
pub struct SetSettingsRequest {
    #[serde(rename = "attributesForFaceting")]
    pub attributes_for_faceting: Option<Vec<String>>,

    #[serde(rename = "searchableAttributes")]
    pub searchable_attributes: Option<Vec<String>>,

    #[serde(rename = "ranking")]
    pub ranking: Option<Vec<String>>,

    #[serde(rename = "customRanking")]
    pub custom_ranking: Option<Vec<String>>,

    #[serde(rename = "attributesToRetrieve")]
    pub attributes_to_retrieve: Option<Vec<String>>,

    #[serde(rename = "unretrievableAttributes")]
    pub unretrievable_attributes: Option<Vec<String>>,

    #[serde(rename = "attributeForDistinct")]
    pub attribute_for_distinct: Option<String>,

    pub distinct: Option<serde_json::Value>,

    #[serde(rename = "removeStopWords")]
    pub remove_stop_words: Option<flapjack::query::stopwords::RemoveStopWordsValue>,

    #[serde(rename = "ignorePlurals")]
    pub ignore_plurals: Option<flapjack::query::plurals::IgnorePluralsValue>,

    #[serde(rename = "queryLanguages")]
    pub query_languages: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub embedders: Option<HashMap<String, serde_json::Value>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<IndexMode>,

    #[serde(rename = "semanticSearch", skip_serializing_if = "Option::is_none")]
    pub semantic_search: Option<SemanticSearchSettings>,

    #[serde(flatten)]
    pub other: Option<serde_json::Map<String, serde_json::Value>>,
}

#[derive(Debug, Serialize)]
pub struct SetSettingsResponse {
    #[serde(rename = "updatedAt")]
    pub updated_at: String,

    #[serde(rename = "taskID")]
    pub task_id: i64,

    #[serde(rename = "unsupportedParams", skip_serializing_if = "Option::is_none")]
    pub unsupported_params: Option<Vec<String>>,
}

/// Update index settings
#[utoipa::path(
    post,
    path = "/1/indexes/{indexName}/settings",
    tag = "settings",
    params(
        ("indexName" = String, Path, description = "Index name")
    ),
    request_body(content = serde_json::Value, description = "Settings to update"),
    responses(
        (status = 200, description = "Settings updated successfully", body = serde_json::Value),
        (status = 400, description = "Invalid settings")
    ),
    security(
        ("api_key" = [])
    )
)]
pub async fn set_settings(
    State(state): State<Arc<AppState>>,
    Path(index_name): Path<String>,
    Json(payload): Json<SetSettingsRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    state
        .manager
        .create_tenant(&index_name)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let settings_path = state
        .manager
        .base_path
        .join(&index_name)
        .join("settings.json");

    let mut unsupported = Vec::new();

    if payload.ranking.is_some() {
        unsupported.push("ranking".to_string());
    }

    if let Some(other) = &payload.other {
        for key in other.keys() {
            unsupported.push(key.clone());
        }
    }

    let distinct_value = payload.distinct.and_then(|v| match v {
        serde_json::Value::Bool(b) => Some(DistinctValue::Bool(b)),
        serde_json::Value::Number(n) => n.as_u64().map(|u| DistinctValue::Integer(u as u32)),
        _ => None,
    });

    let mut settings = if settings_path.exists() {
        IndexSettings::load(&settings_path)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    } else {
        IndexSettings::default()
    };

    if let Some(facets) = payload.attributes_for_faceting {
        settings.attributes_for_faceting = facets;
    }
    if let Some(searchable) = payload.searchable_attributes {
        settings.searchable_attributes = Some(searchable);
    }
    if let Some(custom) = payload.custom_ranking {
        settings.custom_ranking = Some(custom);
    }
    if let Some(retrieve) = payload.attributes_to_retrieve {
        settings.attributes_to_retrieve = Some(retrieve);
    }
    if let Some(unretrievable) = payload.unretrievable_attributes {
        settings.unretrievable_attributes = Some(unretrievable);
    }
    if let Some(distinct_attr) = payload.attribute_for_distinct {
        settings.attribute_for_distinct = Some(distinct_attr);
    }
    if let Some(dv) = distinct_value {
        settings.distinct = Some(dv);
    }
    if let Some(rsw) = payload.remove_stop_words {
        settings.remove_stop_words = rsw;
    }
    if let Some(ip) = payload.ignore_plurals {
        settings.ignore_plurals = ip;
    }
    if let Some(ql) = payload.query_languages {
        settings.query_languages = ql;
    }

    // Capture old embedders for stale detection before merge
    let old_embedders = settings.embedders.clone();
    #[cfg(feature = "vector-search")]
    let embedders_updated = payload.embedders.is_some();

    if let Some(map) = payload.embedders {
        let filtered: HashMap<String, serde_json::Value> =
            map.into_iter().filter(|(_, v)| !v.is_null()).collect();
        if filtered.is_empty() {
            settings.embedders = None;
        } else {
            settings.embedders = Some(filtered);
        }
    }

    if let Some(mode) = payload.mode {
        settings.mode = Some(mode);
    }
    if let Some(ss) = payload.semantic_search {
        settings.semantic_search = Some(ss);
    }

    // Warn if neuralSearch mode is set without embedders configured
    if settings.mode == Some(IndexMode::NeuralSearch) && settings.embedders.is_none() {
        tracing::warn!(
            "mode set to neuralSearch but no embedders configured; hybrid search will fall back to keyword-only until embedders are added"
        );
    }

    // Validate embedders before saving
    settings
        .validate_embedders()
        .map_err(|msg| (StatusCode::BAD_REQUEST, msg))?;

    // Stale vector detection
    for change in detect_embedder_changes(&old_embedders, &settings.embedders) {
        match change {
            EmbedderChange::Modified(name) => {
                tracing::warn!(
                    "embedder '{}' configuration changed; existing vectors may be stale",
                    name
                );
            }
            EmbedderChange::Removed(name) => {
                tracing::warn!(
                    "embedder '{}' removed; associated vectors will be orphaned",
                    name
                );
            }
            EmbedderChange::Added(name) => {
                tracing::info!("embedder '{}' configured", name);
            }
        }
    }

    if let Some(parent) = settings_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to create settings directory: {}", e),
            )
        })?;
    }

    settings
        .save(&settings_path)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    state.manager.invalidate_settings_cache(&index_name);
    state.manager.invalidate_facet_cache(&index_name);

    // Invalidate cached embedders when embedder config changes
    #[cfg(feature = "vector-search")]
    if embedders_updated {
        state.embedder_store.invalidate(&index_name);
    }

    state.manager.append_oplog(
        &index_name,
        "settings",
        serde_json::to_value(&settings).unwrap_or_default(),
    );

    let noop_task = state
        .manager
        .make_noop_task(&index_name)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let response = SetSettingsResponse {
        updated_at: chrono::Utc::now().to_rfc3339(),
        task_id: noop_task.numeric_id,
        unsupported_params: if unsupported.is_empty() {
            None
        } else {
            Some(unsupported)
        },
    };

    let status = if response.unsupported_params.is_some() {
        StatusCode::MULTI_STATUS
    } else {
        StatusCode::OK
    };

    Ok((status, Json(response)))
}

/// Get index settings
#[utoipa::path(
    get,
    path = "/1/indexes/{indexName}/settings",
    tag = "settings",
    params(
        ("indexName" = String, Path, description = "Index name")
    ),
    responses(
        (status = 200, description = "Index settings", body = serde_json::Value),
        (status = 404, description = "Index not found")
    ),
    security(
        ("api_key" = [])
    )
)]
pub async fn get_settings(
    State(state): State<Arc<AppState>>,
    Path(index_name): Path<String>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let settings_path = state
        .manager
        .base_path
        .join(&index_name)
        .join("settings.json");

    let settings = if settings_path.exists() {
        IndexSettings::load(&settings_path)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    } else {
        IndexSettings::default()
    };

    Ok(Json(settings))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handlers::metrics::MetricsState;
    use axum::body::Body;
    use axum::http::Request;
    use axum::routing::get;
    use axum::Router;
    use flapjack::IndexManager;
    use tempfile::TempDir;
    use tower::ServiceExt;

    fn make_settings_state(tmp: &TempDir) -> Arc<AppState> {
        Arc::new(AppState {
            manager: IndexManager::new(tmp.path()),
            key_store: None,
            replication_manager: None,
            ssl_manager: None,
            analytics_engine: None,
            experiment_store: None,
            metrics_state: Some(MetricsState::new()),
            usage_counters: Arc::new(dashmap::DashMap::new()),
            paused_indexes: crate::pause_registry::PausedIndexes::new(),
            start_time: std::time::Instant::now(),
            #[cfg(feature = "vector-search")]
            embedder_store: Arc::new(crate::embedder_store::EmbedderStore::new()),
        })
    }

    fn settings_router(state: Arc<AppState>) -> Router {
        Router::new()
            .route(
                "/1/indexes/:indexName/settings",
                get(get_settings).post(set_settings),
            )
            .with_state(state)
    }

    async fn post_settings(app: &Router, body: &str) -> axum::http::Response<Body> {
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/1/indexes/test_idx/settings")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap()
    }

    async fn get_settings_json(app: &Router) -> serde_json::Value {
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/1/indexes/test_idx/settings")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        serde_json::from_slice(&body).unwrap()
    }

    #[tokio::test]
    async fn test_set_settings_with_embedders() {
        let tmp = TempDir::new().unwrap();
        let state = make_settings_state(&tmp);
        let app = settings_router(state);

        let resp = post_settings(
            &app,
            r#"{"embedders": {"default": {"source": "userProvided", "dimensions": 384}}}"#,
        )
        .await;
        assert_eq!(resp.status(), StatusCode::OK);

        let json = get_settings_json(&app).await;
        assert_eq!(json["embedders"]["default"]["source"], "userProvided");
        assert_eq!(json["embedders"]["default"]["dimensions"], 384);
    }

    #[cfg(feature = "vector-search")]
    #[tokio::test]
    async fn test_set_settings_invalid_embedder_returns_400() {
        let tmp = TempDir::new().unwrap();
        let state = make_settings_state(&tmp);
        let app = settings_router(state);

        let resp = post_settings(&app, r#"{"embedders": {"myEmb": {"source": "openAi"}}}"#).await;
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let body_str = String::from_utf8_lossy(&body);
        assert!(
            body_str.contains("myEmb"),
            "error should mention embedder name: {}",
            body_str
        );
    }

    #[tokio::test]
    async fn test_set_settings_without_embedders_no_embedders_in_response() {
        let tmp = TempDir::new().unwrap();
        let state = make_settings_state(&tmp);
        let app = settings_router(state);

        let resp = post_settings(&app, r#"{"searchableAttributes": ["title"]}"#).await;
        assert_eq!(resp.status(), StatusCode::OK);

        let json = get_settings_json(&app).await;
        assert!(
            json.get("embedders").is_none(),
            "response should not contain 'embedders' key"
        );
    }

    #[tokio::test]
    async fn test_set_settings_embedders_persist_to_disk() {
        let tmp = TempDir::new().unwrap();
        let state = make_settings_state(&tmp);
        let app = settings_router(state);

        post_settings(
            &app,
            r#"{"embedders": {"default": {"source": "userProvided", "dimensions": 256}}}"#,
        )
        .await;

        // Load directly from disk
        let settings_path = tmp.path().join("test_idx").join("settings.json");
        let loaded = IndexSettings::load(&settings_path).unwrap();
        let emb = loaded
            .embedders
            .as_ref()
            .expect("embedders should be persisted");
        assert_eq!(emb["default"]["dimensions"], 256);
    }

    #[tokio::test]
    async fn test_set_settings_preserves_embedders_on_other_update() {
        let tmp = TempDir::new().unwrap();
        let state = make_settings_state(&tmp);
        let app = settings_router(state);

        // First: set embedders
        post_settings(
            &app,
            r#"{"embedders": {"default": {"source": "userProvided", "dimensions": 384}}}"#,
        )
        .await;

        // Second: update a different field (no embedders in payload)
        post_settings(&app, r#"{"attributesForFaceting": ["category"]}"#).await;

        let json = get_settings_json(&app).await;
        assert_eq!(
            json["embedders"]["default"]["dimensions"], 384,
            "embedders should be preserved when not in payload"
        );
    }

    // ── Mode and SemanticSearch handler tests (5.8) ──

    #[tokio::test]
    async fn test_set_settings_mode_neural_search() {
        let tmp = TempDir::new().unwrap();
        let state = make_settings_state(&tmp);
        let app = settings_router(state);

        let resp = post_settings(&app, r#"{"mode": "neuralSearch"}"#).await;
        assert_eq!(resp.status(), StatusCode::OK);

        let json = get_settings_json(&app).await;
        assert_eq!(json["mode"], "neuralSearch");
    }

    #[tokio::test]
    async fn test_set_settings_mode_keyword_search() {
        let tmp = TempDir::new().unwrap();
        let state = make_settings_state(&tmp);
        let app = settings_router(state);

        let resp = post_settings(&app, r#"{"mode": "keywordSearch"}"#).await;
        assert_eq!(resp.status(), StatusCode::OK);

        let json = get_settings_json(&app).await;
        assert_eq!(json["mode"], "keywordSearch");
    }

    #[tokio::test]
    async fn test_set_settings_mode_default_not_in_response() {
        let tmp = TempDir::new().unwrap();
        let state = make_settings_state(&tmp);
        let app = settings_router(state);

        let json = get_settings_json(&app).await;
        assert!(
            json.get("mode").is_none(),
            "fresh index should not have 'mode' key in response"
        );
        assert!(
            json.get("semanticSearch").is_none(),
            "fresh index should not have 'semanticSearch' key in response"
        );
    }

    #[tokio::test]
    async fn test_set_settings_mode_preserves_on_other_update() {
        let tmp = TempDir::new().unwrap();
        let state = make_settings_state(&tmp);
        let app = settings_router(state);

        // Set mode
        post_settings(&app, r#"{"mode": "neuralSearch"}"#).await;

        // Update a different field
        post_settings(&app, r#"{"searchableAttributes": ["title"]}"#).await;

        let json = get_settings_json(&app).await;
        assert_eq!(json["mode"], "neuralSearch", "mode should be preserved");
    }

    #[tokio::test]
    async fn test_set_settings_mode_revert_to_keyword() {
        let tmp = TempDir::new().unwrap();
        let state = make_settings_state(&tmp);
        let app = settings_router(state);

        post_settings(&app, r#"{"mode": "neuralSearch"}"#).await;
        post_settings(&app, r#"{"mode": "keywordSearch"}"#).await;

        let json = get_settings_json(&app).await;
        assert_eq!(json["mode"], "keywordSearch");
    }

    #[tokio::test]
    async fn test_set_settings_semantic_search() {
        let tmp = TempDir::new().unwrap();
        let state = make_settings_state(&tmp);
        let app = settings_router(state);

        let resp = post_settings(
            &app,
            r#"{"semanticSearch": {"eventSources": ["idx1", "idx2"]}}"#,
        )
        .await;
        assert_eq!(resp.status(), StatusCode::OK);

        let json = get_settings_json(&app).await;
        let event_sources = &json["semanticSearch"]["eventSources"];
        assert_eq!(event_sources[0], "idx1");
        assert_eq!(event_sources[1], "idx2");
    }

    #[tokio::test]
    async fn test_set_settings_mode_and_embedders_together() {
        let tmp = TempDir::new().unwrap();
        let state = make_settings_state(&tmp);
        let app = settings_router(state);

        let resp = post_settings(
            &app,
            r#"{"mode": "neuralSearch", "embedders": {"default": {"source": "userProvided", "dimensions": 384}}}"#,
        )
        .await;
        assert_eq!(resp.status(), StatusCode::OK);

        let json = get_settings_json(&app).await;
        assert_eq!(json["mode"], "neuralSearch");
        assert_eq!(json["embedders"]["default"]["source"], "userProvided");
    }

    #[tokio::test]
    async fn test_set_settings_neural_mode_no_embedders_warns() {
        let tmp = TempDir::new().unwrap();
        let state = make_settings_state(&tmp);
        let app = settings_router(state);

        // Should succeed (200), not error — even though no embedders configured
        let resp = post_settings(&app, r#"{"mode": "neuralSearch"}"#).await;
        assert_eq!(resp.status(), StatusCode::OK);
    }

    /// 6.22: Verify that updating embedder settings invalidates the embedder cache.
    #[cfg(feature = "vector-search")]
    #[tokio::test]
    async fn test_set_settings_embedders_invalidate_cache() {
        let tmp = TempDir::new().unwrap();
        let state = make_settings_state(&tmp);
        let app = settings_router(state.clone());

        // Set initial embedder config
        post_settings(
            &app,
            r#"{"embedders": {"default": {"source": "userProvided", "dimensions": 384}}}"#,
        )
        .await;

        // Create and cache an embedder via get_or_create
        let settings = flapjack::index::settings::IndexSettings::load(
            tmp.path().join("test_idx").join("settings.json"),
        )
        .unwrap();
        let e1 = state
            .embedder_store
            .get_or_create("test_idx", "default", &settings)
            .unwrap();

        // Now update embedder settings (different dimensions)
        post_settings(
            &app,
            r#"{"embedders": {"default": {"source": "userProvided", "dimensions": 768}}}"#,
        )
        .await;

        // Get the embedder again — should be a fresh instance (different Arc)
        let updated_settings = flapjack::index::settings::IndexSettings::load(
            tmp.path().join("test_idx").join("settings.json"),
        )
        .unwrap();
        let e2 = state
            .embedder_store
            .get_or_create("test_idx", "default", &updated_settings)
            .unwrap();
        assert!(
            !std::sync::Arc::ptr_eq(&e1, &e2),
            "Embedder should be re-created after settings update (cache invalidated)"
        );
        assert_eq!(
            e2.dimensions(),
            768,
            "New embedder should have updated dimensions"
        );
    }

    #[tokio::test]
    async fn test_set_settings_clear_embedders() {
        let tmp = TempDir::new().unwrap();
        let state = make_settings_state(&tmp);
        let app = settings_router(state);

        // First: set embedders
        post_settings(
            &app,
            r#"{"embedders": {"default": {"source": "userProvided", "dimensions": 384}}}"#,
        )
        .await;

        // Second: clear with empty map
        post_settings(&app, r#"{"embedders": {}}"#).await;

        let json = get_settings_json(&app).await;
        assert!(
            json.get("embedders").is_none(),
            "embedders should be cleared after empty map"
        );
    }
}
