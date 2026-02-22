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
    pub experiment_id: Option<String>,
    pub variant_id: Option<String>,
    pub assignment_method: Option<String>,
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
    #[serde(default)]
    pub interleaving_team: Option<String>,
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
        // Validate interleaving team label matches search response values
        if let Some(ref team) = self.interleaving_team {
            if team != "control" && team != "variant" {
                return Err(format!(
                    "interleavingTeam must be \"control\" or \"variant\", got \"{}\"",
                    team
                ));
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
        Field::new("experiment_id", DataType::Utf8, true),
        Field::new("variant_id", DataType::Utf8, true),
        Field::new("assignment_method", DataType::Utf8, true),
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
        Field::new("interleaving_team", DataType::Utf8, true),
    ]))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_event() -> InsightEvent {
        InsightEvent {
            event_type: "click".to_string(),
            event_subtype: None,
            event_name: "Product Clicked".to_string(),
            index: "products".to_string(),
            user_token: "user123".to_string(),
            authenticated_user_token: None,
            query_id: None,
            object_ids: vec!["obj1".to_string()],
            object_ids_alt: vec![],
            positions: None,
            timestamp: None,
            value: None,
            currency: None,
            interleaving_team: None,
        }
    }

    // ── effective_object_ids ────────────────────────────────────────────

    #[test]
    fn effective_oids_prefers_object_ids() {
        let mut e = valid_event();
        e.object_ids = vec!["a".to_string()];
        e.object_ids_alt = vec!["b".to_string()];
        assert_eq!(e.effective_object_ids(), &["a"]);
    }

    #[test]
    fn effective_oids_falls_back_to_alt() {
        let mut e = valid_event();
        e.object_ids = vec![];
        e.object_ids_alt = vec!["b".to_string()];
        assert_eq!(e.effective_object_ids(), &["b"]);
    }

    // ── validate: event_type ────────────────────────────────────────────

    #[test]
    fn validate_click_ok() {
        assert!(valid_event().validate().is_ok());
    }

    #[test]
    fn validate_conversion_ok() {
        let mut e = valid_event();
        e.event_type = "conversion".to_string();
        assert!(e.validate().is_ok());
    }

    #[test]
    fn validate_view_ok() {
        let mut e = valid_event();
        e.event_type = "view".to_string();
        assert!(e.validate().is_ok());
    }

    #[test]
    fn validate_invalid_event_type() {
        let mut e = valid_event();
        e.event_type = "hover".to_string();
        assert!(e.validate().is_err());
    }

    // ── validate: event_name ────────────────────────────────────────────

    #[test]
    fn validate_empty_event_name() {
        let mut e = valid_event();
        e.event_name = "".to_string();
        assert!(e.validate().is_err());
    }

    #[test]
    fn validate_event_name_too_long() {
        let mut e = valid_event();
        e.event_name = "x".repeat(65);
        assert!(e.validate().is_err());
    }

    #[test]
    fn validate_event_name_at_max_64() {
        let mut e = valid_event();
        e.event_name = "x".repeat(64);
        assert!(e.validate().is_ok());
    }

    // ── validate: user_token ────────────────────────────────────────────

    #[test]
    fn validate_empty_user_token() {
        let mut e = valid_event();
        e.user_token = "".to_string();
        assert!(e.validate().is_err());
    }

    #[test]
    fn validate_user_token_too_long() {
        let mut e = valid_event();
        e.user_token = "x".repeat(130);
        assert!(e.validate().is_err());
    }

    // ── validate: object_ids ────────────────────────────────────────────

    #[test]
    fn validate_no_object_ids() {
        let mut e = valid_event();
        e.object_ids = vec![];
        e.object_ids_alt = vec![];
        assert!(e.validate().is_err());
    }

    #[test]
    fn validate_too_many_object_ids() {
        let mut e = valid_event();
        e.object_ids = (0..21).map(|i| format!("obj{}", i)).collect();
        assert!(e.validate().is_err());
    }

    #[test]
    fn validate_20_object_ids_ok() {
        let mut e = valid_event();
        e.object_ids = (0..20).map(|i| format!("obj{}", i)).collect();
        assert!(e.validate().is_ok());
    }

    // ── validate: click-after-search positions ──────────────────────────

    #[test]
    fn validate_click_with_query_id_needs_positions() {
        let mut e = valid_event();
        e.event_type = "click".to_string();
        e.query_id = Some("a".repeat(32));
        e.positions = None;
        assert!(e.validate().is_err());
    }

    #[test]
    fn validate_click_with_query_id_positions_length_mismatch() {
        let mut e = valid_event();
        e.event_type = "click".to_string();
        e.query_id = Some("a".repeat(32));
        e.object_ids = vec!["obj1".to_string(), "obj2".to_string()];
        e.positions = Some(vec![1]); // mismatch: 2 objects, 1 position
        assert!(e.validate().is_err());
    }

    #[test]
    fn validate_click_with_query_id_positions_match() {
        let mut e = valid_event();
        e.event_type = "click".to_string();
        e.query_id = Some("a".repeat(32));
        e.positions = Some(vec![1]);
        assert!(e.validate().is_ok());
    }

    // ── validate: query_id format ───────────────────────────────────────

    #[test]
    fn validate_query_id_not_32_chars() {
        let mut e = valid_event();
        e.query_id = Some("abc".to_string());
        assert!(e.validate().is_err());
    }

    #[test]
    fn validate_query_id_non_hex() {
        let mut e = valid_event();
        e.query_id = Some("g".repeat(32));
        assert!(e.validate().is_err());
    }

    #[test]
    fn validate_query_id_valid_hex() {
        let mut e = valid_event();
        // Don't need positions since this is not a click-after-search
        e.event_type = "view".to_string();
        e.query_id = Some("abcdef0123456789abcdef0123456789".to_string());
        assert!(e.validate().is_ok());
    }

    // ── validate: timestamp ─────────────────────────────────────────────

    #[test]
    fn validate_recent_timestamp_ok() {
        let mut e = valid_event();
        e.timestamp = Some(chrono::Utc::now().timestamp_millis() - 1000);
        assert!(e.validate().is_ok());
    }

    #[test]
    fn validate_old_timestamp_rejected() {
        let mut e = valid_event();
        // 5 days ago
        let five_days_ms = 5 * 24 * 60 * 60 * 1000_i64;
        e.timestamp = Some(chrono::Utc::now().timestamp_millis() - five_days_ms);
        assert!(e.validate().is_err());
    }

    // ── Arrow schemas ───────────────────────────────────────────────────

    #[test]
    fn search_event_schema_has_19_fields() {
        let schema = search_event_schema();
        assert_eq!(schema.fields().len(), 19);
    }

    #[test]
    fn search_event_schema_has_experiment_id_field() {
        let schema = search_event_schema();
        let field = schema.field_with_name("experiment_id").unwrap();
        assert!(field.is_nullable());
        assert_eq!(*field.data_type(), DataType::Utf8);
    }

    #[test]
    fn search_event_schema_has_variant_id_field() {
        let schema = search_event_schema();
        let field = schema.field_with_name("variant_id").unwrap();
        assert!(field.is_nullable());
        assert_eq!(*field.data_type(), DataType::Utf8);
    }

    #[test]
    fn search_event_schema_has_assignment_method_field() {
        let schema = search_event_schema();
        let field = schema.field_with_name("assignment_method").unwrap();
        assert!(field.is_nullable());
        assert_eq!(*field.data_type(), DataType::Utf8);
    }

    #[test]
    fn insight_event_schema_has_13_fields() {
        let schema = insight_event_schema();
        assert_eq!(schema.fields().len(), 13);
    }

    #[test]
    fn insight_event_schema_has_interleaving_team_field() {
        let schema = insight_event_schema();
        let field = schema.field_with_name("interleaving_team").unwrap();
        assert!(field.is_nullable());
        assert_eq!(*field.data_type(), DataType::Utf8);
    }

    #[test]
    fn insight_event_deserializes_interleaving_team() {
        let json = r#"{"eventType":"click","eventName":"Clicked","index":"products","userToken":"user1","objectIDs":["obj1"],"interleavingTeam":"control"}"#;
        let event: InsightEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event.interleaving_team.as_deref(), Some("control"));
    }

    #[test]
    fn insight_event_without_interleaving_team_defaults_to_none() {
        let json = r#"{"eventType":"click","eventName":"Clicked","index":"products","userToken":"user1","objectIDs":["obj1"]}"#;
        let event: InsightEvent = serde_json::from_str(json).unwrap();
        assert!(event.interleaving_team.is_none());
    }

    #[test]
    fn validate_interleaving_team_control_accepted() {
        let mut ev = valid_event();
        ev.interleaving_team = Some("control".to_string());
        assert!(ev.validate().is_ok());
    }

    #[test]
    fn validate_interleaving_team_variant_accepted() {
        let mut ev = valid_event();
        ev.interleaving_team = Some("variant".to_string());
        assert!(ev.validate().is_ok());
    }

    #[test]
    fn validate_interleaving_team_none_accepted() {
        let ev = valid_event();
        assert!(ev.interleaving_team.is_none());
        assert!(ev.validate().is_ok());
    }

    #[test]
    fn validate_interleaving_team_rejects_arbitrary_value() {
        let mut ev = valid_event();
        ev.interleaving_team = Some("A".to_string());
        let err = ev.validate().unwrap_err();
        assert!(err.contains("interleavingTeam"), "error should mention field name: {err}");
    }

    #[test]
    fn search_event_schema_timestamp_is_i64() {
        let schema = search_event_schema();
        let field = schema.field_with_name("timestamp_ms").unwrap();
        assert_eq!(*field.data_type(), DataType::Int64);
    }

    // ── InsightEvent deserialization ─────────────────────────────────────

    #[test]
    fn insight_event_deserializes_from_json() {
        let json = r#"{"eventType":"click","eventName":"Clicked","index":"products","userToken":"user1","objectIDs":["obj1"]}"#;
        let event: InsightEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event.event_type, "click");
        assert_eq!(event.effective_object_ids(), &["obj1"]);
    }

    #[test]
    fn insight_event_deserializes_alt_object_ids() {
        let json = r#"{"eventType":"click","eventName":"Clicked","index":"products","userToken":"user1","objectIDs":["obj1"]}"#;
        let event: InsightEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event.effective_object_ids(), &["obj1"]);
    }
}
