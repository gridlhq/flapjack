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
