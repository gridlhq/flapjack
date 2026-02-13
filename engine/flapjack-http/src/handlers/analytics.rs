use axum::{
    extract::{Query, State},
    Json,
};
use serde::Deserialize;
use std::sync::Arc;

use flapjack::analytics::AnalyticsQueryEngine;
use flapjack::error::FlapjackError;

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

/// GET /2/searches - Top searches ranked by frequency
pub async fn get_top_searches(
    State(engine): State<Arc<AnalyticsQueryEngine>>,
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
    Ok(Json(result))
}

/// GET /2/searches/count - Total search count with daily breakdown
pub async fn get_search_count(
    State(engine): State<Arc<AnalyticsQueryEngine>>,
    Query(params): Query<AnalyticsParams>,
) -> Result<Json<serde_json::Value>, FlapjackError> {
    let result = engine
        .search_count(&params.index, &params.start_date, &params.end_date)
        .await
        .map_err(|e| FlapjackError::InvalidQuery(format!("Analytics error: {}", e)))?;
    Ok(Json(result))
}

/// GET /2/searches/noResults - Top queries with 0 results
pub async fn get_no_results(
    State(engine): State<Arc<AnalyticsQueryEngine>>,
    Query(params): Query<AnalyticsParams>,
) -> Result<Json<serde_json::Value>, FlapjackError> {
    let limit = params.limit.unwrap_or(1000);
    let result = engine
        .no_results_searches(&params.index, &params.start_date, &params.end_date, limit)
        .await
        .map_err(|e| FlapjackError::InvalidQuery(format!("Analytics error: {}", e)))?;
    Ok(Json(result))
}

/// GET /2/searches/noResultRate - No-results rate with daily breakdown
pub async fn get_no_result_rate(
    State(engine): State<Arc<AnalyticsQueryEngine>>,
    Query(params): Query<AnalyticsParams>,
) -> Result<Json<serde_json::Value>, FlapjackError> {
    let result = engine
        .no_results_rate(&params.index, &params.start_date, &params.end_date)
        .await
        .map_err(|e| FlapjackError::InvalidQuery(format!("Analytics error: {}", e)))?;
    Ok(Json(result))
}

/// GET /2/searches/noClicks - Top searches with 0 clicks
pub async fn get_no_clicks(
    State(engine): State<Arc<AnalyticsQueryEngine>>,
    Query(params): Query<AnalyticsParams>,
) -> Result<Json<serde_json::Value>, FlapjackError> {
    let limit = params.limit.unwrap_or(1000);
    let result = engine
        .no_click_searches(&params.index, &params.start_date, &params.end_date, limit)
        .await
        .map_err(|e| FlapjackError::InvalidQuery(format!("Analytics error: {}", e)))?;
    Ok(Json(result))
}

/// GET /2/searches/noClickRate - No-click rate with daily breakdown
pub async fn get_no_click_rate(
    State(engine): State<Arc<AnalyticsQueryEngine>>,
    Query(params): Query<AnalyticsParams>,
) -> Result<Json<serde_json::Value>, FlapjackError> {
    let result = engine
        .no_click_rate(&params.index, &params.start_date, &params.end_date)
        .await
        .map_err(|e| FlapjackError::InvalidQuery(format!("Analytics error: {}", e)))?;
    Ok(Json(result))
}

/// GET /2/clicks/clickThroughRate - CTR with daily breakdown
pub async fn get_click_through_rate(
    State(engine): State<Arc<AnalyticsQueryEngine>>,
    Query(params): Query<AnalyticsParams>,
) -> Result<Json<serde_json::Value>, FlapjackError> {
    let result = engine
        .click_through_rate(&params.index, &params.start_date, &params.end_date)
        .await
        .map_err(|e| FlapjackError::InvalidQuery(format!("Analytics error: {}", e)))?;
    Ok(Json(result))
}

/// GET /2/clicks/averageClickPosition - Average click position
pub async fn get_average_click_position(
    State(engine): State<Arc<AnalyticsQueryEngine>>,
    Query(params): Query<AnalyticsParams>,
) -> Result<Json<serde_json::Value>, FlapjackError> {
    let result = engine
        .average_click_position(&params.index, &params.start_date, &params.end_date)
        .await
        .map_err(|e| FlapjackError::InvalidQuery(format!("Analytics error: {}", e)))?;
    Ok(Json(result))
}

/// GET /2/clicks/positions - Click position distribution (Algolia-style buckets)
pub async fn get_click_positions(
    State(engine): State<Arc<AnalyticsQueryEngine>>,
    Query(params): Query<AnalyticsParams>,
) -> Result<Json<serde_json::Value>, FlapjackError> {
    let result = engine
        .click_positions(&params.index, &params.start_date, &params.end_date)
        .await
        .map_err(|e| FlapjackError::InvalidQuery(format!("Analytics error: {}", e)))?;
    Ok(Json(result))
}

/// GET /2/conversions/conversionRate - Conversion rate
pub async fn get_conversion_rate(
    State(engine): State<Arc<AnalyticsQueryEngine>>,
    Query(params): Query<AnalyticsParams>,
) -> Result<Json<serde_json::Value>, FlapjackError> {
    let result = engine
        .conversion_rate(&params.index, &params.start_date, &params.end_date)
        .await
        .map_err(|e| FlapjackError::InvalidQuery(format!("Analytics error: {}", e)))?;
    Ok(Json(result))
}

/// GET /2/hits - Top clicked objectIDs
pub async fn get_top_hits(
    State(engine): State<Arc<AnalyticsQueryEngine>>,
    Query(params): Query<AnalyticsParams>,
) -> Result<Json<serde_json::Value>, FlapjackError> {
    let limit = params.limit.unwrap_or(1000);
    let result = engine
        .top_hits(&params.index, &params.start_date, &params.end_date, limit)
        .await
        .map_err(|e| FlapjackError::InvalidQuery(format!("Analytics error: {}", e)))?;
    Ok(Json(result))
}

/// GET /2/filters - Top filter attributes
pub async fn get_top_filters(
    State(engine): State<Arc<AnalyticsQueryEngine>>,
    Query(params): Query<AnalyticsParams>,
) -> Result<Json<serde_json::Value>, FlapjackError> {
    let limit = params.limit.unwrap_or(1000);
    let result = engine
        .top_filters(&params.index, &params.start_date, &params.end_date, limit)
        .await
        .map_err(|e| FlapjackError::InvalidQuery(format!("Analytics error: {}", e)))?;
    Ok(Json(result))
}

/// GET /2/filters/:attribute - Top values for a filter attribute
pub async fn get_filter_values(
    State(engine): State<Arc<AnalyticsQueryEngine>>,
    axum::extract::Path(attribute): axum::extract::Path<String>,
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
    Ok(Json(result))
}

/// GET /2/filters/noResults - Filters causing no results
pub async fn get_filters_no_results(
    State(engine): State<Arc<AnalyticsQueryEngine>>,
    Query(params): Query<AnalyticsParams>,
) -> Result<Json<serde_json::Value>, FlapjackError> {
    let limit = params.limit.unwrap_or(1000);
    let result = engine
        .filters_no_results(&params.index, &params.start_date, &params.end_date, limit)
        .await
        .map_err(|e| FlapjackError::InvalidQuery(format!("Analytics error: {}", e)))?;
    Ok(Json(result))
}

/// GET /2/users/count - Unique user count
pub async fn get_users_count(
    State(engine): State<Arc<AnalyticsQueryEngine>>,
    Query(params): Query<AnalyticsParams>,
) -> Result<Json<serde_json::Value>, FlapjackError> {
    let result = engine
        .users_count(&params.index, &params.start_date, &params.end_date)
        .await
        .map_err(|e| FlapjackError::InvalidQuery(format!("Analytics error: {}", e)))?;
    Ok(Json(result))
}

/// GET /2/overview - Server-wide analytics overview across all indices
pub async fn get_overview(
    State(engine): State<Arc<AnalyticsQueryEngine>>,
    Query(params): Query<OverviewParams>,
) -> Result<Json<serde_json::Value>, FlapjackError> {
    let result = engine
        .overview(&params.start_date, &params.end_date)
        .await
        .map_err(|e| FlapjackError::InvalidQuery(format!("Analytics error: {}", e)))?;
    Ok(Json(result))
}

/// POST /2/analytics/seed - Seed demo analytics data for an index
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

/// POST /2/analytics/flush - Flush buffered analytics events to disk immediately
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

/// DELETE /2/analytics/clear - Clear all analytics data for an index
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
    State(engine): State<Arc<AnalyticsQueryEngine>>,
    Query(params): Query<AnalyticsParams>,
) -> Result<Json<serde_json::Value>, FlapjackError> {
    let result = engine
        .device_breakdown(&params.index, &params.start_date, &params.end_date)
        .await
        .map_err(|e| FlapjackError::InvalidQuery(format!("Analytics error: {}", e)))?;
    Ok(Json(result))
}

/// GET /2/geo - Geographic breakdown from country field
pub async fn get_geo_breakdown(
    State(engine): State<Arc<AnalyticsQueryEngine>>,
    Query(params): Query<AnalyticsParams>,
) -> Result<Json<serde_json::Value>, FlapjackError> {
    let limit = params.limit.unwrap_or(50);
    let result = engine
        .geo_breakdown(&params.index, &params.start_date, &params.end_date, limit)
        .await
        .map_err(|e| FlapjackError::InvalidQuery(format!("Analytics error: {}", e)))?;
    Ok(Json(result))
}

/// GET /2/geo/:country - Top searches for a specific country
pub async fn get_geo_top_searches(
    State(engine): State<Arc<AnalyticsQueryEngine>>,
    axum::extract::Path(country): axum::extract::Path<String>,
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
    Ok(Json(result))
}

/// GET /2/geo/:country/regions - Region (state) breakdown for a country
pub async fn get_geo_regions(
    State(engine): State<Arc<AnalyticsQueryEngine>>,
    axum::extract::Path(country): axum::extract::Path<String>,
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
    Ok(Json(result))
}

/// GET /2/status - Analytics status
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
