use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use super::AppState;
use flapjack::index::rules::{Rule, RuleStore};
use flapjack::index::settings::IndexSettings;
use flapjack::index::synonyms::{Synonym, SynonymStore};
use flapjack::types::Document;

#[derive(Debug, Deserialize)]
pub struct MigrateFromAlgoliaRequest {
    #[serde(rename = "appId")]
    pub app_id: String,

    #[serde(rename = "apiKey")]
    pub api_key: String,

    #[serde(rename = "sourceIndex")]
    pub source_index: String,

    #[serde(rename = "targetIndex")]
    pub target_index: Option<String>,

    /// If true, delete any existing target index before migrating.
    /// Without this, migrating to an existing index returns 409.
    #[serde(default)]
    pub overwrite: bool,
}

#[derive(Debug, Serialize)]
pub struct MigrateFromAlgoliaResponse {
    pub status: String,
    pub settings: bool,
    pub synonyms: MigrateCount,
    pub rules: MigrateCount,
    pub objects: MigrateCount,
    #[serde(rename = "taskID")]
    pub task_id: i64,
}

#[derive(Debug, Serialize)]
pub struct MigrateCount {
    pub imported: usize,
}

fn algolia_host(app_id: &str) -> String {
    format!("{}-dsn.algolia.net", app_id)
}

fn algolia_url(app_id: &str, path: &str) -> String {
    format!("https://{}{}", algolia_host(app_id), path)
}

fn algolia_headers(app_id: &str, api_key: &str) -> Vec<(&'static str, String)> {
    vec![
        ("x-algolia-application-id", app_id.to_string()),
        ("x-algolia-api-key", api_key.to_string()),
        ("content-type", "application/json".to_string()),
    ]
}

async fn algolia_get(
    client: &reqwest::Client,
    app_id: &str,
    api_key: &str,
    path: &str,
) -> Result<serde_json::Value, String> {
    let url = algolia_url(app_id, path);
    let mut req = client.get(&url);
    for (k, v) in algolia_headers(app_id, api_key) {
        req = req.header(k, v);
    }
    let resp = req
        .send()
        .await
        .map_err(|e| format!("Algolia request failed: {}", e))?;
    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("Algolia returned {}: {}", status, body));
    }
    resp.json()
        .await
        .map_err(|e| format!("Failed to parse Algolia response: {}", e))
}

async fn algolia_post(
    client: &reqwest::Client,
    app_id: &str,
    api_key: &str,
    path: &str,
    body: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let url = algolia_url(app_id, path);
    let mut req = client.post(&url).json(body);
    for (k, v) in algolia_headers(app_id, api_key) {
        req = req.header(k, v);
    }
    let resp = req
        .send()
        .await
        .map_err(|e| format!("Algolia request failed: {}", e))?;
    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("Algolia returned {}: {}", status, body));
    }
    resp.json()
        .await
        .map_err(|e| format!("Failed to parse Algolia response: {}", e))
}

/// One-click migration from Algolia to Flapjack.
///
/// Fetches settings, synonyms, rules, and all objects from the source Algolia
/// index and imports them into the target Flapjack index in a single call.
pub async fn migrate_from_algolia(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<MigrateFromAlgoliaRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let app_id = &payload.app_id;
    let api_key = &payload.api_key;
    let source_index = &payload.source_index;
    let target_index = payload.target_index.as_deref().unwrap_or(source_index);

    // Validate
    if app_id.is_empty() || api_key.is_empty() || source_index.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "message": "appId, apiKey, and sourceIndex are required"
            })),
        ));
    }

    let client = reqwest::Client::new();

    // Check if the target index already exists
    let target_path = state.manager.base_path.join(target_index);
    if target_path.exists() {
        if !payload.overwrite {
            return Err((
                StatusCode::CONFLICT,
                Json(serde_json::json!({
                    "message": format!(
                        "Target index '{}' already exists. Use \"overwrite\": true to replace it.",
                        target_index
                    )
                })),
            ));
        }
        // Delete the existing index first
        tracing::info!("[migrate] Overwriting existing index '{}'", target_index);
        state
            .manager
            .delete_tenant(&target_index.to_string())
            .await
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({
                        "message": format!("Failed to delete existing index: {}", e)
                    })),
                )
            })?;
    }

    // Create the target index in Flapjack
    state.manager.create_tenant(target_index).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"message": format!("Failed to create index: {}", e)})),
        )
    })?;

    // ── 1. Settings ─────────────────────────────────────────────────────
    tracing::info!(
        "[migrate] Fetching settings from Algolia {}/{}",
        app_id,
        source_index
    );
    let settings_json = algolia_get(
        &client,
        app_id,
        api_key,
        &format!("/1/indexes/{}/settings", urlencoding::encode(source_index)),
    )
    .await
    .map_err(|e| algolia_error(&e))?;

    // Apply settings to Flapjack
    let settings_path = state
        .manager
        .base_path
        .join(target_index)
        .join("settings.json");

    let mut settings = if settings_path.exists() {
        IndexSettings::load(&settings_path).unwrap_or_default()
    } else {
        IndexSettings::default()
    };

    if let Some(arr) = settings_json
        .get("searchableAttributes")
        .and_then(|v| v.as_array())
    {
        settings.searchable_attributes = Some(
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect(),
        );
    }
    if let Some(arr) = settings_json
        .get("attributesForFaceting")
        .and_then(|v| v.as_array())
    {
        settings.attributes_for_faceting = arr
            .iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect();
    }
    if let Some(arr) = settings_json
        .get("customRanking")
        .and_then(|v| v.as_array())
    {
        settings.custom_ranking = Some(
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect(),
        );
    }
    if let Some(arr) = settings_json
        .get("attributesToRetrieve")
        .and_then(|v| v.as_array())
    {
        settings.attributes_to_retrieve = Some(
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect(),
        );
    }
    if let Some(arr) = settings_json
        .get("unretrievableAttributes")
        .and_then(|v| v.as_array())
    {
        settings.unretrievable_attributes = Some(
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect(),
        );
    }
    if let Some(s) = settings_json
        .get("attributeForDistinct")
        .and_then(|v| v.as_str())
    {
        settings.attribute_for_distinct = Some(s.to_string());
    }

    if let Some(parent) = settings_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"message": format!("Failed to create dir: {}", e)})),
            )
        })?;
    }
    settings.save(&settings_path).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"message": format!("Failed to save settings: {}", e)})),
        )
    })?;
    state.manager.invalidate_settings_cache(target_index);
    state.manager.invalidate_facet_cache(target_index);
    tracing::info!("[migrate] Settings imported");

    // ── 2. Synonyms ────────────────────────────────────────────────────
    tracing::info!("[migrate] Fetching synonyms from Algolia");
    let mut all_synonyms: Vec<Synonym> = Vec::new();
    let mut page = 0;
    loop {
        let resp = algolia_post(
            &client,
            app_id,
            api_key,
            &format!(
                "/1/indexes/{}/synonyms/search",
                urlencoding::encode(source_index)
            ),
            &serde_json::json!({"query": "", "hitsPerPage": 1000, "page": page}),
        )
        .await
        .map_err(|e| algolia_error(&e))?;

        let hits = resp.get("hits").and_then(|v| v.as_array());
        let nb_hits = resp.get("nbHits").and_then(|v| v.as_u64()).unwrap_or(0);

        if let Some(hits) = hits {
            for hit in hits {
                // Strip _highlightResult before parsing
                let mut clean = hit.clone();
                if let Some(obj) = clean.as_object_mut() {
                    obj.remove("_highlightResult");
                }
                if let Ok(syn) = serde_json::from_value::<Synonym>(clean) {
                    all_synonyms.push(syn);
                }
            }
        }

        let fetched = (page + 1) * 1000;
        if fetched >= nb_hits as usize || hits.map(|h| h.len()).unwrap_or(0) < 1000 {
            break;
        }
        page += 1;
    }

    // Save synonyms
    let synonyms_path = state
        .manager
        .base_path
        .join(target_index)
        .join("synonyms.json");
    let mut syn_store = SynonymStore::new();
    for syn in &all_synonyms {
        syn_store.insert(syn.clone());
    }
    syn_store.save(&synonyms_path).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"message": format!("Failed to save synonyms: {}", e)})),
        )
    })?;
    state.manager.invalidate_synonyms_cache(target_index);
    let synonyms_count = all_synonyms.len();
    tracing::info!("[migrate] Imported {} synonyms", synonyms_count);

    // ── 3. Rules ───────────────────────────────────────────────────────
    tracing::info!("[migrate] Fetching rules from Algolia");
    let mut all_rules: Vec<Rule> = Vec::new();
    let mut page = 0;
    loop {
        let resp = algolia_post(
            &client,
            app_id,
            api_key,
            &format!(
                "/1/indexes/{}/rules/search",
                urlencoding::encode(source_index)
            ),
            &serde_json::json!({"query": "", "hitsPerPage": 1000, "page": page}),
        )
        .await
        .map_err(|e| algolia_error(&e))?;

        let hits = resp.get("hits").and_then(|v| v.as_array());
        let nb_hits = resp.get("nbHits").and_then(|v| v.as_u64()).unwrap_or(0);

        if let Some(hits) = hits {
            for hit in hits {
                let mut clean = hit.clone();
                if let Some(obj) = clean.as_object_mut() {
                    obj.remove("_highlightResult");
                }
                if let Ok(rule) = serde_json::from_value::<Rule>(clean) {
                    all_rules.push(rule);
                }
            }
        }

        let fetched = (page + 1) * 1000;
        if fetched >= nb_hits as usize || hits.map(|h| h.len()).unwrap_or(0) < 1000 {
            break;
        }
        page += 1;
    }

    // Save rules (only if non-empty — an empty rules.json triggers the rules
    // branch in search which currently skips synonym expansion)
    let rules_count = all_rules.len();
    if !all_rules.is_empty() {
        let rules_path = state
            .manager
            .base_path
            .join(target_index)
            .join("rules.json");
        let mut rule_store = RuleStore::new();
        for rule in &all_rules {
            rule_store.insert(rule.clone());
        }
        rule_store.save(&rules_path).map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"message": format!("Failed to save rules: {}", e)})),
            )
        })?;
        state.manager.invalidate_rules_cache(target_index);
    }
    tracing::info!("[migrate] Imported {} rules", rules_count);

    // ── 4. Objects (browse) ────────────────────────────────────────────
    tracing::info!("[migrate] Browsing objects from Algolia");
    let mut total_objects = 0usize;
    let mut cursor: Option<String> = None;
    let mut last_task_id: i64 = 0;

    loop {
        let body = if let Some(ref c) = cursor {
            serde_json::json!({"cursor": c})
        } else {
            serde_json::json!({"hitsPerPage": 1000})
        };

        let resp = algolia_post(
            &client,
            app_id,
            api_key,
            &format!("/1/indexes/{}/browse", urlencoding::encode(source_index)),
            &body,
        )
        .await
        .map_err(|e| algolia_error(&e))?;

        let hits = resp
            .get("hits")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        if hits.is_empty() {
            break;
        }

        // Clean and convert hits to Documents
        let mut documents = Vec::with_capacity(hits.len());
        for hit in &hits {
            let mut clean = hit.clone();
            if let Some(obj) = clean.as_object_mut() {
                obj.remove("_highlightResult");
                obj.remove("_snippetResult");
                obj.remove("_rankingInfo");
            }
            match Document::from_json(&clean) {
                Ok(doc) => documents.push(doc),
                Err(e) => {
                    tracing::warn!("[migrate] Skipping doc: {}", e);
                }
            }
        }

        let batch_size = documents.len();
        if !documents.is_empty() {
            let task = state
                .manager
                .add_documents(target_index, documents)
                .map_err(|e| {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(serde_json::json!({
                            "message": format!("Failed to import objects: {}", e),
                            "objects_imported_before_error": total_objects
                        })),
                    )
                })?;
            last_task_id = task.numeric_id;
        }
        total_objects += batch_size;

        cursor = resp
            .get("cursor")
            .and_then(|v| v.as_str())
            .map(String::from);

        if cursor.is_none() {
            break;
        }
    }

    tracing::info!("[migrate] Imported {} objects total", total_objects);

    // ── 5. Wait for indexing to complete ────────────────────────────────
    let max_wait = std::time::Duration::from_secs(60);
    let start = std::time::Instant::now();
    loop {
        let pending = state.manager.pending_task_count(target_index);
        if pending == 0 {
            break;
        }
        if start.elapsed() > max_wait {
            tracing::warn!(
                "[migrate] Timed out waiting for indexing ({} tasks pending)",
                pending
            );
            break;
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    }

    tracing::info!(
        "[migrate] Migration complete: settings=true, synonyms={}, rules={}, objects={}",
        synonyms_count,
        rules_count,
        total_objects
    );

    Ok(Json(MigrateFromAlgoliaResponse {
        status: "complete".to_string(),
        settings: true,
        synonyms: MigrateCount {
            imported: synonyms_count,
        },
        rules: MigrateCount {
            imported: rules_count,
        },
        objects: MigrateCount {
            imported: total_objects,
        },
        task_id: last_task_id,
    }))
}

fn algolia_error(msg: &str) -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::BAD_GATEWAY,
        Json(serde_json::json!({"message": msg})),
    )
}
