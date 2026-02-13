use axum::{extract::State, Json};
use std::sync::Arc;

use flapjack::analytics::schema::InsightEvent;
use flapjack::analytics::AnalyticsCollector;
use flapjack::error::FlapjackError;

/// POST /1/events - Algolia Insights API compatible event ingestion
pub async fn post_events(
    State(collector): State<Arc<AnalyticsCollector>>,
    Json(body): Json<InsightsRequest>,
) -> Result<Json<serde_json::Value>, FlapjackError> {
    if body.events.len() > 1000 {
        return Err(FlapjackError::InvalidQuery(
            "Maximum 1000 events per request".to_string(),
        ));
    }

    let mut accepted = 0;
    let mut errors: Vec<String> = Vec::new();

    for event in body.events {
        match event.validate() {
            Ok(()) => {
                collector.record_insight(event);
                accepted += 1;
            }
            Err(e) => {
                errors.push(e);
            }
        }
    }

    if !errors.is_empty() && accepted == 0 {
        return Err(FlapjackError::InvalidQuery(format!(
            "All events rejected: {}",
            errors.join("; ")
        )));
    }

    Ok(Json(serde_json::json!({
        "status": 200,
        "message": "OK"
    })))
}

#[derive(Debug, serde::Deserialize)]
pub struct InsightsRequest {
    pub events: Vec<InsightEvent>,
}
