use arrow::datatypes::{DataType, Field, Schema};
use std::sync::Arc;

/// Recorded automatically on every search request.
#[derive(Debug, Clone)]
pub struct SearchEvent {
    pub timestamp_ms: i64,
    pub query: String,
    pub query_id: Option<String>,
    pub index_name: String,
    pub nb_hits: u32,
    pub processing_time_ms: u32,
    pub user_token: Option<String>,
    pub user_ip: Option<String>,
    pub filters: Option<String>,
    pub facets: Option<String>,
    pub analytics_tags: Option<String>,
    pub page: u32,
    pub hits_per_page: u32,
    pub has_results: bool,
    pub country: Option<String>,
    pub region: Option<String>,
}

/// Sent by client via Insights API (click, conversion, view events).
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InsightEvent {
    pub event_type: String,
    #[serde(default)]
    pub event_subtype: Option<String>,
    pub event_name: String,
    pub index: String,
    pub user_token: String,
    #[serde(default)]
    pub authenticated_user_token: Option<String>,
    #[serde(default)]
    pub query_id: Option<String>,
    #[serde(default)]
    pub object_ids: Vec<String>,
    #[serde(default, rename = "objectIDs")]
    pub object_ids_alt: Vec<String>,
    #[serde(default)]
    pub positions: Option<Vec<u32>>,
    #[serde(default)]
    pub timestamp: Option<i64>,
    #[serde(default)]
    pub value: Option<f64>,
    #[serde(default)]
    pub currency: Option<String>,
}

impl InsightEvent {
    /// Get the effective objectIDs (handles both camelCase variants from Algolia SDK).
    pub fn effective_object_ids(&self) -> &[String] {
        if !self.object_ids.is_empty() {
            &self.object_ids
        } else {
            &self.object_ids_alt
        }
    }

    /// Validate per Algolia spec.
    pub fn validate(&self) -> Result<(), String> {
        if !matches!(self.event_type.as_str(), "click" | "conversion" | "view") {
            return Err(format!("Invalid eventType: {}", self.event_type));
        }
        if self.event_name.is_empty() || self.event_name.len() > 64 {
            return Err("eventName must be 1-64 characters".to_string());
        }
        if self.user_token.is_empty() || self.user_token.len() > 129 {
            return Err("userToken must be 1-129 characters".to_string());
        }
        let oids = self.effective_object_ids();
        if oids.is_empty() || oids.len() > 20 {
            return Err("objectIDs must have 1-20 items".to_string());
        }
        // For click-after-search, positions are required and must match objectIDs length
        if self.event_type == "click" && self.query_id.is_some() {
            match &self.positions {
                None => return Err("positions required for click-after-search events".to_string()),
                Some(pos) if pos.len() != oids.len() => {
                    return Err("positions length must match objectIDs length".to_string());
                }
                _ => {}
            }
        }
        if let Some(ref qid) = self.query_id {
            if qid.len() != 32 || !qid.chars().all(|c| c.is_ascii_hexdigit()) {
                return Err("queryID must be 32-char hex string".to_string());
            }
        }
        // Reject events older than 4 days
        if let Some(ts) = self.timestamp {
            let now_ms = chrono::Utc::now().timestamp_millis();
            let four_days_ms = 4 * 24 * 60 * 60 * 1000_i64;
            if ts < now_ms - four_days_ms {
                return Err("timestamp must be within the last 4 days".to_string());
            }
        }
        Ok(())
    }
}

/// Arrow schema for search events stored in Parquet.
pub fn search_event_schema() -> Arc<Schema> {
    Arc::new(Schema::new(vec![
        Field::new("timestamp_ms", DataType::Int64, false),
        Field::new("query", DataType::Utf8, false),
        Field::new("query_id", DataType::Utf8, true),
        Field::new("index_name", DataType::Utf8, false),
        Field::new("nb_hits", DataType::UInt32, false),
        Field::new("processing_time_ms", DataType::UInt32, false),
        Field::new("user_token", DataType::Utf8, true),
        Field::new("user_ip", DataType::Utf8, true),
        Field::new("filters", DataType::Utf8, true),
        Field::new("facets", DataType::Utf8, true),
        Field::new("analytics_tags", DataType::Utf8, true),
        Field::new("page", DataType::UInt32, false),
        Field::new("hits_per_page", DataType::UInt32, false),
        Field::new("has_results", DataType::Boolean, false),
        Field::new("country", DataType::Utf8, true),
        Field::new("region", DataType::Utf8, true),
    ]))
}

/// Arrow schema for insight events (clicks, conversions, views) stored in Parquet.
pub fn insight_event_schema() -> Arc<Schema> {
    Arc::new(Schema::new(vec![
        Field::new("timestamp_ms", DataType::Int64, false),
        Field::new("event_type", DataType::Utf8, false),
        Field::new("event_subtype", DataType::Utf8, true),
        Field::new("event_name", DataType::Utf8, false),
        Field::new("index_name", DataType::Utf8, false),
        Field::new("user_token", DataType::Utf8, false),
        Field::new("authenticated_user_token", DataType::Utf8, true),
        Field::new("query_id", DataType::Utf8, true),
        Field::new("object_ids", DataType::Utf8, false), // JSON array string
        Field::new("positions", DataType::Utf8, true),   // JSON array string
        Field::new("value", DataType::Float64, true),
        Field::new("currency", DataType::Utf8, true),
    ]))
}
