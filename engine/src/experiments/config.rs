use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Experiment {
    pub id: String,
    pub name: String,
    pub index_name: String,
    pub status: ExperimentStatus,
    pub traffic_split: f64,
    pub control: ExperimentArm,
    pub variant: ExperimentArm,
    pub primary_metric: PrimaryMetric,
    pub created_at: i64,
    pub started_at: Option<i64>,
    pub ended_at: Option<i64>,
    pub minimum_days: u32,
    pub winsorization_cap: Option<f64>,
    pub conclusion: Option<ExperimentConclusion>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ExperimentStatus {
    Draft,
    Running,
    Stopped,
    Concluded,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ExperimentArm {
    pub name: String,
    pub query_overrides: Option<QueryOverrides>,
    pub index_name: Option<String>,
}

/// Query-time parameters overridable per variant arm (Mode A).
/// All fields optional — only set what differs from the main index settings.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct QueryOverrides {
    pub typo_tolerance: Option<serde_json::Value>,
    pub enable_synonyms: Option<bool>,
    pub enable_rules: Option<bool>,
    pub rule_contexts: Option<Vec<String>>,
    pub filters: Option<String>,
    pub optional_filters: Option<Vec<String>>,
    pub custom_ranking: Option<Vec<String>>,
    pub attribute_weights: Option<HashMap<String, f32>>,
    pub remove_words_if_no_results: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum PrimaryMetric {
    Ctr,
    ConversionRate,
    RevenuePerSearch,
    ZeroResultRate,
    AbandonmentRate,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ExperimentConclusion {
    pub winner: Option<String>,
    pub reason: String,
    pub control_metric: f64,
    pub variant_metric: f64,
    pub confidence: f64,
    pub significant: bool,
    pub promoted: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum ExperimentError {
    #[error("experiment not found: {0}")]
    NotFound(String),
    #[error("experiment already exists: {0}")]
    AlreadyExists(String),
    #[error("invalid status transition: experiment is {0}")]
    InvalidStatus(String),
    #[error("invalid configuration: {0}")]
    InvalidConfig(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("serialization error: {0}")]
    Json(#[from] serde_json::Error),
}

impl Experiment {
    pub fn validate(&self) -> Result<(), ExperimentError> {
        if self.traffic_split <= 0.0 || self.traffic_split >= 1.0 {
            return Err(ExperimentError::InvalidConfig(
                "trafficSplit must be in (0.0, 1.0) exclusive".to_string(),
            ));
        }
        let has_query_overrides = self.variant.query_overrides.is_some();
        let has_index_name = self.variant.index_name.is_some();
        if has_query_overrides == has_index_name {
            return Err(ExperimentError::InvalidConfig(
                "variant must define exactly one mode: queryOverrides (Mode A) or indexName (Mode B)"
                    .to_string(),
            ));
        }
        if self.control.query_overrides.is_some() || self.control.index_name.is_some() {
            return Err(ExperimentError::InvalidConfig(
                "control arm must not have queryOverrides or indexName — it is the baseline"
                    .to_string(),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_experiment() -> Experiment {
        Experiment {
            id: "550e8400-e29b-41d4-a716-446655440000".to_string(),
            name: "Test experiment".to_string(),
            index_name: "products".to_string(),
            status: ExperimentStatus::Draft,
            traffic_split: 0.5,
            control: ExperimentArm {
                name: "control".to_string(),
                query_overrides: None,
                index_name: None,
            },
            variant: ExperimentArm {
                name: "variant".to_string(),
                query_overrides: Some(QueryOverrides {
                    enable_synonyms: Some(false),
                    ..Default::default()
                }),
                index_name: None,
            },
            primary_metric: PrimaryMetric::Ctr,
            created_at: 1700000000000,
            started_at: None,
            ended_at: None,
            minimum_days: 14,
            winsorization_cap: None,
            conclusion: None,
        }
    }

    #[test]
    fn validate_valid_experiment_succeeds() {
        assert!(valid_experiment().validate().is_ok());
    }

    #[test]
    fn validate_traffic_split_zero_fails() {
        let mut e = valid_experiment();
        e.traffic_split = 0.0;
        assert!(e.validate().is_err());
    }

    #[test]
    fn validate_traffic_split_one_fails() {
        let mut e = valid_experiment();
        e.traffic_split = 1.0;
        assert!(e.validate().is_err());
    }

    #[test]
    fn validate_traffic_split_0_01_passes() {
        let mut e = valid_experiment();
        e.traffic_split = 0.01;
        assert!(e.validate().is_ok());
    }

    #[test]
    fn validate_variant_with_no_config_fails() {
        let mut e = valid_experiment();
        e.variant.query_overrides = None;
        e.variant.index_name = None;
        assert!(e.validate().is_err());
    }

    #[test]
    fn validate_mode_b_variant_index_passes() {
        let mut e = valid_experiment();
        e.variant.query_overrides = None;
        e.variant.index_name = Some("products_v2".to_string());
        assert!(e.validate().is_ok());
    }

    #[test]
    fn validate_variant_with_both_mode_a_and_mode_b_fails() {
        let mut e = valid_experiment();
        e.variant.query_overrides = Some(QueryOverrides {
            enable_synonyms: Some(false),
            ..Default::default()
        });
        e.variant.index_name = Some("products_v2".to_string());
        assert!(e.validate().is_err());
    }

    #[test]
    fn validate_control_with_overrides_fails() {
        let mut e = valid_experiment();
        e.control.query_overrides = Some(QueryOverrides {
            enable_synonyms: Some(true),
            ..Default::default()
        });
        assert!(e.validate().is_err());
    }

    #[test]
    fn experiment_serializes_to_camel_case() {
        let e = valid_experiment();
        let json = serde_json::to_string(&e).unwrap();
        assert!(json.contains("indexName"));
        assert!(json.contains("trafficSplit"));
        assert!(!json.contains("index_name"));
    }

    #[test]
    fn experiment_roundtrips_through_json() {
        let e = valid_experiment();
        let json = serde_json::to_string(&e).unwrap();
        let back: Experiment = serde_json::from_str(&json).unwrap();
        assert_eq!(back.id, e.id);
        assert_eq!(back.traffic_split, e.traffic_split);
        assert_eq!(back.primary_metric, e.primary_metric);
    }

    #[test]
    fn query_overrides_default_is_all_none() {
        let q = QueryOverrides::default();
        assert!(q.enable_synonyms.is_none());
        assert!(q.enable_rules.is_none());
        assert!(q.filters.is_none());
        assert!(q.custom_ranking.is_none());
    }

    #[test]
    fn experiment_status_serializes_correctly() {
        assert_eq!(
            serde_json::to_string(&ExperimentStatus::Draft).unwrap(),
            "\"draft\""
        );
        assert_eq!(
            serde_json::to_string(&ExperimentStatus::Running).unwrap(),
            "\"running\""
        );
    }
}
