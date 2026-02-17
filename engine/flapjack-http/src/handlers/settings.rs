use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use super::AppState;
use flapjack::index::settings::{DistinctValue, IndexSettings};

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
