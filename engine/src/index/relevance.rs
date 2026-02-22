use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RelevanceConfig {
    #[serde(default, rename = "searchableAttributes")]
    pub searchable_attributes: Option<Vec<String>>,

    #[serde(default, rename = "attributeWeights")]
    pub attribute_weights: HashMap<String, f32>,
}

impl RelevanceConfig {
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: RelevanceConfig = serde_json::from_str(&content)?;
        Ok(config)
    }

    pub fn derive_weights(&self) -> HashMap<String, f32> {
        let mut weights = HashMap::new();

        if let Some(attrs) = &self.searchable_attributes {
            for (idx, field) in attrs.iter().enumerate() {
                let default_weight = 100_f32.powi(-(idx as i32));
                let weight = self
                    .attribute_weights
                    .get(field)
                    .copied()
                    .unwrap_or(default_weight);
                weights.insert(field.clone(), weight);
            }
        }

        weights
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_empty() {
        let cfg = RelevanceConfig::default();
        assert!(cfg.searchable_attributes.is_none());
        assert!(cfg.attribute_weights.is_empty());
    }

    #[test]
    fn derive_weights_no_attributes_returns_empty() {
        let cfg = RelevanceConfig::default();
        assert!(cfg.derive_weights().is_empty());
    }

    #[test]
    fn derive_weights_single_attribute_weight_1() {
        let cfg = RelevanceConfig {
            searchable_attributes: Some(vec!["title".to_string()]),
            attribute_weights: HashMap::new(),
        };
        let w = cfg.derive_weights();
        assert_eq!(w.len(), 1);
        // 100^0 = 1.0
        assert!((w["title"] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn derive_weights_decays_exponentially() {
        let cfg = RelevanceConfig {
            searchable_attributes: Some(vec![
                "title".to_string(),
                "description".to_string(),
                "tags".to_string(),
            ]),
            attribute_weights: HashMap::new(),
        };
        let w = cfg.derive_weights();
        // title: 100^0 = 1.0, description: 100^-1 = 0.01, tags: 100^-2 = 0.0001
        assert!((w["title"] - 1.0).abs() < 1e-6);
        assert!((w["description"] - 0.01).abs() < 1e-6);
        assert!((w["tags"] - 0.0001).abs() < 1e-6);
    }

    #[test]
    fn derive_weights_custom_override() {
        let mut custom = HashMap::new();
        custom.insert("title".to_string(), 5.0_f32);
        let cfg = RelevanceConfig {
            searchable_attributes: Some(vec!["title".to_string(), "body".to_string()]),
            attribute_weights: custom,
        };
        let w = cfg.derive_weights();
        assert!((w["title"] - 5.0).abs() < 1e-6);
        // body still uses default: 100^-1 = 0.01
        assert!((w["body"] - 0.01).abs() < 1e-6);
    }

    #[test]
    fn derive_weights_partial_override() {
        let mut custom = HashMap::new();
        custom.insert("body".to_string(), 2.0_f32);
        let cfg = RelevanceConfig {
            searchable_attributes: Some(vec!["title".to_string(), "body".to_string()]),
            attribute_weights: custom,
        };
        let w = cfg.derive_weights();
        // title uses default: 100^0 = 1.0
        assert!((w["title"] - 1.0).abs() < 1e-6);
        // body uses custom: 2.0
        assert!((w["body"] - 2.0).abs() < 1e-6);
    }

    #[test]
    fn config_deserializes_from_json() {
        let json = r#"{"searchableAttributes":["title","body"],"attributeWeights":{"title":10.0}}"#;
        let cfg: RelevanceConfig = serde_json::from_str(json).unwrap();
        assert_eq!(
            cfg.searchable_attributes,
            Some(vec!["title".to_string(), "body".to_string()])
        );
        assert!((cfg.attribute_weights["title"] - 10.0).abs() < 1e-6);
    }

    #[test]
    fn config_deserializes_empty_json() {
        let cfg: RelevanceConfig = serde_json::from_str("{}").unwrap();
        assert!(cfg.searchable_attributes.is_none());
        assert!(cfg.attribute_weights.is_empty());
    }
}
