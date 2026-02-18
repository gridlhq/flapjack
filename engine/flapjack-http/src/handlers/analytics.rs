use axum::{
    extract::{Query, RawQuery, State},
    http::HeaderMap,
    Json,
};
use serde::Deserialize;
use std::collections::HashSet;
use std::sync::Arc;

use flapjack::analytics::AnalyticsQueryEngine;
use flapjack::error::FlapjackError;

use super::AppState;

/// Shared query parameters for all analytics endpoints.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyticsParams {
    pub index: String,
    #[serde(default = "default_start_date")]
    pub start_date: String,
    #[serde(default = "default_end_date")]
    pub end_date: String,
    #[serde(default)]
    pub tags: Option<String>,
    #[serde(default)]
    pub limit: Option<usize>,
    #[serde(default)]
    pub offset: Option<usize>,
    #[serde(default)]
    pub click_analytics: Option<bool>,
    #[serde(default)]
    pub country: Option<String>,
    #[serde(default)]
    pub order_by: Option<String>,
}

/// Query parameters for the overview endpoint (no index required).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OverviewParams {
    #[serde(default = "default_start_date")]
    pub start_date: String,
    #[serde(default = "default_end_date")]
    pub end_date: String,
}

fn default_start_date() -> String {
    (chrono::Utc::now() - chrono::Duration::days(8))
        .format("%Y-%m-%d")
        .to_string()
}

fn default_end_date() -> String {
    chrono::Utc::now().format("%Y-%m-%d").to_string()
}

/// If cluster mode and not local-only, fan out query to peers and merge results.
/// Returns local result unchanged in standalone mode or when X-Flapjack-Local-Only is set.
async fn maybe_fan_out(
    headers: &HeaderMap,
    endpoint: &str,
    path: &str,
    raw_query: &str,
    local_result: serde_json::Value,
    limit: usize,
) -> serde_json::Value {
    // Skip fan-out if local-only header present (peer-to-peer query)
    if headers.get("X-Flapjack-Local-Only").is_some() {
        return local_result;
    }
    // Skip if no cluster client configured
    let cluster = match crate::analytics_cluster::get_global_cluster() {
        Some(c) => c,
        None => return local_result,
    };
    // Fan out to peers, merge, and return
    cluster
        .fan_out_and_merge(endpoint, path, raw_query, local_result, limit)
        .await
}

/// GET /2/searches - Top searches ranked by frequency
pub async fn get_top_searches(
    headers: HeaderMap,
    State(engine): State<Arc<AnalyticsQueryEngine>>,
    RawQuery(raw_query): RawQuery,
    Query(params): Query<AnalyticsParams>,
) -> Result<Json<serde_json::Value>, FlapjackError> {
    let limit = params.limit.unwrap_or(10);
    let click_analytics = params.click_analytics.unwrap_or(false);
    let result = engine
        .top_searches(
            &params.index,
            &params.start_date,
            &params.end_date,
            limit,
            click_analytics,
            params.country.as_deref(),
            params.tags.as_deref(),
        )
        .await
        .map_err(|e| FlapjackError::InvalidQuery(format!("Analytics error: {}", e)))?;
    let result = maybe_fan_out(&headers, "searches", "/2/searches", &raw_query.unwrap_or_default(), result, limit).await;
    Ok(Json(result))
}

/// GET /2/searches/count - Total search count with daily breakdown
pub async fn get_search_count(
    headers: HeaderMap,
    State(engine): State<Arc<AnalyticsQueryEngine>>,
    RawQuery(raw_query): RawQuery,
    Query(params): Query<AnalyticsParams>,
) -> Result<Json<serde_json::Value>, FlapjackError> {
    let result = engine
        .search_count(&params.index, &params.start_date, &params.end_date)
        .await
        .map_err(|e| FlapjackError::InvalidQuery(format!("Analytics error: {}", e)))?;
    let result = maybe_fan_out(&headers, "searches/count", "/2/searches/count", &raw_query.unwrap_or_default(), result, 1000).await;
    Ok(Json(result))
}

/// GET /2/searches/noResults - Top queries with 0 results
pub async fn get_no_results(
    headers: HeaderMap,
    State(engine): State<Arc<AnalyticsQueryEngine>>,
    RawQuery(raw_query): RawQuery,
    Query(params): Query<AnalyticsParams>,
) -> Result<Json<serde_json::Value>, FlapjackError> {
    let limit = params.limit.unwrap_or(1000);
    let result = engine
        .no_results_searches(&params.index, &params.start_date, &params.end_date, limit)
        .await
        .map_err(|e| FlapjackError::InvalidQuery(format!("Analytics error: {}", e)))?;
    let result = maybe_fan_out(&headers, "searches/noResults", "/2/searches/noResults", &raw_query.unwrap_or_default(), result, limit).await;
    Ok(Json(result))
}

/// GET /2/searches/noResultRate - No-results rate with daily breakdown
pub async fn get_no_result_rate(
    headers: HeaderMap,
    State(engine): State<Arc<AnalyticsQueryEngine>>,
    RawQuery(raw_query): RawQuery,
    Query(params): Query<AnalyticsParams>,
) -> Result<Json<serde_json::Value>, FlapjackError> {
    let result = engine
        .no_results_rate(&params.index, &params.start_date, &params.end_date)
        .await
        .map_err(|e| FlapjackError::InvalidQuery(format!("Analytics error: {}", e)))?;
    let result = maybe_fan_out(&headers, "searches/noResultRate", "/2/searches/noResultRate", &raw_query.unwrap_or_default(), result, 1000).await;
    Ok(Json(result))
}

/// GET /2/searches/noClicks - Top searches with 0 clicks
pub async fn get_no_clicks(
    headers: HeaderMap,
    State(engine): State<Arc<AnalyticsQueryEngine>>,
    RawQuery(raw_query): RawQuery,
    Query(params): Query<AnalyticsParams>,
) -> Result<Json<serde_json::Value>, FlapjackError> {
    let limit = params.limit.unwrap_or(1000);
    let result = engine
        .no_click_searches(&params.index, &params.start_date, &params.end_date, limit)
        .await
        .map_err(|e| FlapjackError::InvalidQuery(format!("Analytics error: {}", e)))?;
    let result = maybe_fan_out(&headers, "searches/noClicks", "/2/searches/noClicks", &raw_query.unwrap_or_default(), result, limit).await;
    Ok(Json(result))
}

/// GET /2/searches/noClickRate - No-click rate with daily breakdown
pub async fn get_no_click_rate(
    headers: HeaderMap,
    State(engine): State<Arc<AnalyticsQueryEngine>>,
    RawQuery(raw_query): RawQuery,
    Query(params): Query<AnalyticsParams>,
) -> Result<Json<serde_json::Value>, FlapjackError> {
    let result = engine
        .no_click_rate(&params.index, &params.start_date, &params.end_date)
        .await
        .map_err(|e| FlapjackError::InvalidQuery(format!("Analytics error: {}", e)))?;
    let result = maybe_fan_out(&headers, "searches/noClickRate", "/2/searches/noClickRate", &raw_query.unwrap_or_default(), result, 1000).await;
    Ok(Json(result))
}

/// GET /2/clicks/clickThroughRate - CTR with daily breakdown
pub async fn get_click_through_rate(
    headers: HeaderMap,
    State(engine): State<Arc<AnalyticsQueryEngine>>,
    RawQuery(raw_query): RawQuery,
    Query(params): Query<AnalyticsParams>,
) -> Result<Json<serde_json::Value>, FlapjackError> {
    let result = engine
        .click_through_rate(&params.index, &params.start_date, &params.end_date)
        .await
        .map_err(|e| FlapjackError::InvalidQuery(format!("Analytics error: {}", e)))?;
    let result = maybe_fan_out(&headers, "clicks/clickThroughRate", "/2/clicks/clickThroughRate", &raw_query.unwrap_or_default(), result, 1000).await;
    Ok(Json(result))
}

/// GET /2/clicks/averageClickPosition - Average click position
pub async fn get_average_click_position(
    headers: HeaderMap,
    State(engine): State<Arc<AnalyticsQueryEngine>>,
    RawQuery(raw_query): RawQuery,
    Query(params): Query<AnalyticsParams>,
) -> Result<Json<serde_json::Value>, FlapjackError> {
    let result = engine
        .average_click_position(&params.index, &params.start_date, &params.end_date)
        .await
        .map_err(|e| FlapjackError::InvalidQuery(format!("Analytics error: {}", e)))?;
    let result = maybe_fan_out(&headers, "clicks/averageClickPosition", "/2/clicks/averageClickPosition", &raw_query.unwrap_or_default(), result, 1000).await;
    Ok(Json(result))
}

/// GET /2/clicks/positions - Click position distribution (Algolia-style buckets)
pub async fn get_click_positions(
    headers: HeaderMap,
    State(engine): State<Arc<AnalyticsQueryEngine>>,
    RawQuery(raw_query): RawQuery,
    Query(params): Query<AnalyticsParams>,
) -> Result<Json<serde_json::Value>, FlapjackError> {
    let result = engine
        .click_positions(&params.index, &params.start_date, &params.end_date)
        .await
        .map_err(|e| FlapjackError::InvalidQuery(format!("Analytics error: {}", e)))?;
    let result = maybe_fan_out(&headers, "clicks/positions", "/2/clicks/positions", &raw_query.unwrap_or_default(), result, 1000).await;
    Ok(Json(result))
}

/// GET /2/conversions/conversionRate - Conversion rate
pub async fn get_conversion_rate(
    headers: HeaderMap,
    State(engine): State<Arc<AnalyticsQueryEngine>>,
    RawQuery(raw_query): RawQuery,
    Query(params): Query<AnalyticsParams>,
) -> Result<Json<serde_json::Value>, FlapjackError> {
    let result = engine
        .conversion_rate(&params.index, &params.start_date, &params.end_date)
        .await
        .map_err(|e| FlapjackError::InvalidQuery(format!("Analytics error: {}", e)))?;
    let result = maybe_fan_out(&headers, "conversions/conversionRate", "/2/conversions/conversionRate", &raw_query.unwrap_or_default(), result, 1000).await;
    Ok(Json(result))
}

/// GET /2/hits - Top clicked objectIDs
pub async fn get_top_hits(
    headers: HeaderMap,
    State(engine): State<Arc<AnalyticsQueryEngine>>,
    RawQuery(raw_query): RawQuery,
    Query(params): Query<AnalyticsParams>,
) -> Result<Json<serde_json::Value>, FlapjackError> {
    let limit = params.limit.unwrap_or(1000);
    let result = engine
        .top_hits(&params.index, &params.start_date, &params.end_date, limit)
        .await
        .map_err(|e| FlapjackError::InvalidQuery(format!("Analytics error: {}", e)))?;
    let result = maybe_fan_out(&headers, "hits", "/2/hits", &raw_query.unwrap_or_default(), result, limit).await;
    Ok(Json(result))
}

/// GET /2/filters - Top filter attributes
pub async fn get_top_filters(
    headers: HeaderMap,
    State(engine): State<Arc<AnalyticsQueryEngine>>,
    RawQuery(raw_query): RawQuery,
    Query(params): Query<AnalyticsParams>,
) -> Result<Json<serde_json::Value>, FlapjackError> {
    let limit = params.limit.unwrap_or(1000);
    let result = engine
        .top_filters(&params.index, &params.start_date, &params.end_date, limit)
        .await
        .map_err(|e| FlapjackError::InvalidQuery(format!("Analytics error: {}", e)))?;
    let result = maybe_fan_out(&headers, "filters", "/2/filters", &raw_query.unwrap_or_default(), result, limit).await;
    Ok(Json(result))
}

/// GET /2/filters/:attribute - Top values for a filter attribute
pub async fn get_filter_values(
    headers: HeaderMap,
    State(engine): State<Arc<AnalyticsQueryEngine>>,
    axum::extract::Path(attribute): axum::extract::Path<String>,
    RawQuery(raw_query): RawQuery,
    Query(params): Query<AnalyticsParams>,
) -> Result<Json<serde_json::Value>, FlapjackError> {
    let limit = params.limit.unwrap_or(1000);
    let result = engine
        .filter_values(
            &params.index,
            &attribute,
            &params.start_date,
            &params.end_date,
            limit,
        )
        .await
        .map_err(|e| FlapjackError::InvalidQuery(format!("Analytics error: {}", e)))?;
    let endpoint = format!("filters/{}", attribute);
    let path = format!("/2/filters/{}", attribute);
    let result = maybe_fan_out(&headers, &endpoint, &path, &raw_query.unwrap_or_default(), result, limit).await;
    Ok(Json(result))
}

/// GET /2/filters/noResults - Filters causing no results
pub async fn get_filters_no_results(
    headers: HeaderMap,
    State(engine): State<Arc<AnalyticsQueryEngine>>,
    RawQuery(raw_query): RawQuery,
    Query(params): Query<AnalyticsParams>,
) -> Result<Json<serde_json::Value>, FlapjackError> {
    let limit = params.limit.unwrap_or(1000);
    let result = engine
        .filters_no_results(&params.index, &params.start_date, &params.end_date, limit)
        .await
        .map_err(|e| FlapjackError::InvalidQuery(format!("Analytics error: {}", e)))?;
    let result = maybe_fan_out(&headers, "filters/noResults", "/2/filters/noResults", &raw_query.unwrap_or_default(), result, limit).await;
    Ok(Json(result))
}

/// GET /2/users/count - Unique user count
pub async fn get_users_count(
    headers: HeaderMap,
    State(engine): State<Arc<AnalyticsQueryEngine>>,
    RawQuery(raw_query): RawQuery,
    Query(params): Query<AnalyticsParams>,
) -> Result<Json<serde_json::Value>, FlapjackError> {
    let result = engine
        .users_count(&params.index, &params.start_date, &params.end_date)
        .await
        .map_err(|e| FlapjackError::InvalidQuery(format!("Analytics error: {}", e)))?;
    let result = maybe_fan_out(&headers, "users/count", "/2/users/count", &raw_query.unwrap_or_default(), result, 1000).await;
    Ok(Json(result))
}

/// GET /2/overview - Server-wide analytics overview across all indices
pub async fn get_overview(
    headers: HeaderMap,
    State(engine): State<Arc<AnalyticsQueryEngine>>,
    RawQuery(raw_query): RawQuery,
    Query(params): Query<OverviewParams>,
) -> Result<Json<serde_json::Value>, FlapjackError> {
    let result = engine
        .overview(&params.start_date, &params.end_date)
        .await
        .map_err(|e| FlapjackError::InvalidQuery(format!("Analytics error: {}", e)))?;
    let result = maybe_fan_out(&headers, "overview", "/2/overview", &raw_query.unwrap_or_default(), result, 1000).await;
    Ok(Json(result))
}

/// POST /2/analytics/seed - Seed demo analytics data for an index (local only, no fan-out)
pub async fn seed_analytics(
    State(engine): State<Arc<AnalyticsQueryEngine>>,
    Json(body): Json<SeedRequest>,
) -> Result<Json<serde_json::Value>, FlapjackError> {
    let index = body
        .index
        .ok_or_else(|| FlapjackError::InvalidQuery("Missing 'index' field".to_string()))?;
    let days = body.days.unwrap_or(30).min(90);

    let config = engine.config();
    let result = flapjack::analytics::seed::seed_analytics(config, &index, days)
        .map_err(|e| FlapjackError::InvalidQuery(format!("Seed error: {}", e)))?;

    Ok(Json(serde_json::json!({
        "status": "ok",
        "index": index,
        "days": result.days,
        "totalSearches": result.total_searches,
        "totalClicks": result.total_clicks,
        "totalConversions": result.total_conversions,
    })))
}

#[derive(Debug, Deserialize)]
pub struct SeedRequest {
    pub index: Option<String>,
    pub days: Option<u32>,
}

/// POST /2/analytics/flush - Flush buffered analytics events to disk immediately (local only)
pub async fn flush_analytics() -> Result<Json<serde_json::Value>, FlapjackError> {
    if let Some(collector) = flapjack::analytics::get_global_collector() {
        collector.flush_all();
        Ok(Json(serde_json::json!({ "status": "ok" })))
    } else {
        Ok(Json(
            serde_json::json!({ "status": "ok", "note": "analytics not initialized" }),
        ))
    }
}

/// DELETE /2/analytics/clear - Clear all analytics data for an index (local only)
pub async fn clear_analytics(
    State(engine): State<Arc<AnalyticsQueryEngine>>,
    Json(body): Json<SeedRequest>,
) -> Result<Json<serde_json::Value>, FlapjackError> {
    let index = body
        .index
        .ok_or_else(|| FlapjackError::InvalidQuery("Missing 'index' field".to_string()))?;

    let config = engine.config();
    let searches_dir = config.searches_dir(&index);
    let events_dir = config.events_dir(&index);

    let mut removed = 0u64;
    for dir in [&searches_dir, &events_dir] {
        if dir.exists() {
            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        let _ = std::fs::remove_dir_all(&path);
                        removed += 1;
                    } else if path.is_file() {
                        let _ = std::fs::remove_file(&path);
                        removed += 1;
                    }
                }
            }
        }
    }

    Ok(Json(serde_json::json!({
        "status": "ok",
        "index": index,
        "partitionsRemoved": removed,
    })))
}

/// GET /2/devices - Device/platform breakdown from analytics_tags
pub async fn get_device_breakdown(
    headers: HeaderMap,
    State(engine): State<Arc<AnalyticsQueryEngine>>,
    RawQuery(raw_query): RawQuery,
    Query(params): Query<AnalyticsParams>,
) -> Result<Json<serde_json::Value>, FlapjackError> {
    let result = engine
        .device_breakdown(&params.index, &params.start_date, &params.end_date)
        .await
        .map_err(|e| FlapjackError::InvalidQuery(format!("Analytics error: {}", e)))?;
    let result = maybe_fan_out(&headers, "devices", "/2/devices", &raw_query.unwrap_or_default(), result, 1000).await;
    Ok(Json(result))
}

/// GET /2/geo - Geographic breakdown from country field
pub async fn get_geo_breakdown(
    headers: HeaderMap,
    State(engine): State<Arc<AnalyticsQueryEngine>>,
    RawQuery(raw_query): RawQuery,
    Query(params): Query<AnalyticsParams>,
) -> Result<Json<serde_json::Value>, FlapjackError> {
    let limit = params.limit.unwrap_or(50);
    let result = engine
        .geo_breakdown(&params.index, &params.start_date, &params.end_date, limit)
        .await
        .map_err(|e| FlapjackError::InvalidQuery(format!("Analytics error: {}", e)))?;
    let result = maybe_fan_out(&headers, "geo", "/2/geo", &raw_query.unwrap_or_default(), result, limit).await;
    Ok(Json(result))
}

/// GET /2/geo/:country - Top searches for a specific country
pub async fn get_geo_top_searches(
    headers: HeaderMap,
    State(engine): State<Arc<AnalyticsQueryEngine>>,
    axum::extract::Path(country): axum::extract::Path<String>,
    RawQuery(raw_query): RawQuery,
    Query(params): Query<AnalyticsParams>,
) -> Result<Json<serde_json::Value>, FlapjackError> {
    let limit = params.limit.unwrap_or(10);
    let result = engine
        .geo_top_searches(
            &params.index,
            &country,
            &params.start_date,
            &params.end_date,
            limit,
        )
        .await
        .map_err(|e| FlapjackError::InvalidQuery(format!("Analytics error: {}", e)))?;
    let endpoint = format!("geo/{}", country);
    let path = format!("/2/geo/{}", country);
    let result = maybe_fan_out(&headers, &endpoint, &path, &raw_query.unwrap_or_default(), result, limit).await;
    Ok(Json(result))
}

/// GET /2/geo/:country/regions - Region (state) breakdown for a country
pub async fn get_geo_regions(
    headers: HeaderMap,
    State(engine): State<Arc<AnalyticsQueryEngine>>,
    axum::extract::Path(country): axum::extract::Path<String>,
    RawQuery(raw_query): RawQuery,
    Query(params): Query<AnalyticsParams>,
) -> Result<Json<serde_json::Value>, FlapjackError> {
    let limit = params.limit.unwrap_or(50);
    let result = engine
        .geo_region_breakdown(
            &params.index,
            &country,
            &params.start_date,
            &params.end_date,
            limit,
        )
        .await
        .map_err(|e| FlapjackError::InvalidQuery(format!("Analytics error: {}", e)))?;
    let endpoint = format!("geo/{}/regions", country);
    let path = format!("/2/geo/{}/regions", country);
    let result = maybe_fan_out(&headers, &endpoint, &path, &raw_query.unwrap_or_default(), result, limit).await;
    Ok(Json(result))
}

/// GET /2/status - Analytics status (local only, no fan-out)
pub async fn get_analytics_status(
    State(engine): State<Arc<AnalyticsQueryEngine>>,
    Query(params): Query<AnalyticsParams>,
) -> Result<Json<serde_json::Value>, FlapjackError> {
    let result = engine
        .status(&params.index)
        .await
        .map_err(|e| FlapjackError::InvalidQuery(format!("Analytics error: {}", e)))?;
    Ok(Json(result))
}

/// POST /2/analytics/cleanup - Remove analytics data for indexes that no longer exist (local only)
pub async fn cleanup_analytics(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, FlapjackError> {
    let engine = state
        .analytics_engine
        .as_ref()
        .ok_or_else(|| FlapjackError::InvalidQuery("Analytics not available".to_string()))?;

    // Get analytics index names
    let analytics_indices = engine
        .list_analytics_indices()
        .map_err(|e| FlapjackError::InvalidQuery(format!("Analytics error: {}", e)))?;

    // Get active index names from the IndexManager's base_path
    let mut active_indices: HashSet<String> = HashSet::new();
    if state.manager.base_path.exists() {
        if let Ok(entries) = std::fs::read_dir(&state.manager.base_path) {
            for entry in entries.flatten() {
                if entry.path().is_dir() {
                    if let Some(name) = entry.file_name().to_str() {
                        active_indices.insert(name.to_string());
                    }
                }
            }
        }
    }

    // Diff: orphaned = analytics_indices - active_indices
    let orphaned: Vec<String> = analytics_indices
        .into_iter()
        .filter(|name| !active_indices.contains(name))
        .collect();

    // Delete analytics directories for orphaned indexes
    let config = engine.config();
    for index_name in &orphaned {
        let index_dir = config.data_dir.join(index_name);
        if index_dir.exists() {
            let _ = std::fs::remove_dir_all(&index_dir);
        }
    }

    let count = orphaned.len();
    Ok(Json(serde_json::json!({
        "status": "ok",
        "removedIndices": orphaned,
        "removedCount": count,
    })))
}
