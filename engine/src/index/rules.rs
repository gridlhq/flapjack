use crate::error::Result;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rule {
    #[serde(rename = "objectID")]
    pub object_id: String,

    #[serde(default)]
    pub conditions: Vec<Condition>,

    pub consequence: Consequence,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub validity: Option<Vec<TimeRange>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Condition {
    pub pattern: String,
    pub anchoring: Anchoring,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub alternatives: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub filters: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Anchoring {
    Is,
    StartsWith,
    EndsWith,
    Contains,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeRange {
    pub from: i64,
    pub until: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Consequence {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub promote: Option<Vec<Promote>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub hide: Option<Vec<Hide>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub filter_promotes: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_data: Option<serde_json::Value>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<ConsequenceParams>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsequenceParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Promote {
    Single {
        #[serde(rename = "objectID")]
        object_id: String,
        position: usize,
    },
    Multiple {
        #[serde(rename = "objectIDs")]
        object_ids: Vec<String>,
        position: usize,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hide {
    #[serde(rename = "objectID")]
    pub object_id: String,
}

impl Rule {
    pub fn is_enabled(&self) -> bool {
        self.enabled.unwrap_or(true)
    }

    pub fn is_valid_at(&self, timestamp: i64) -> bool {
        match &self.validity {
            None => true,
            Some(ranges) => ranges
                .iter()
                .any(|r| timestamp >= r.from && timestamp <= r.until),
        }
    }

    pub fn matches(&self, query_text: &str, context: Option<&str>) -> bool {
        if !self.is_enabled() {
            return false;
        }

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        if !self.is_valid_at(now) {
            return false;
        }

        if self.conditions.is_empty() {
            return true;
        }

        for condition in &self.conditions {
            if let Some(ctx) = &condition.context {
                if context != Some(ctx.as_str()) {
                    continue;
                }
            }

            if self.matches_pattern(query_text, &condition.pattern, &condition.anchoring) {
                return true;
            }
        }

        false
    }

    fn matches_pattern(&self, query_text: &str, pattern: &str, anchoring: &Anchoring) -> bool {
        let query_lower = query_text.to_lowercase();
        let pattern_lower = pattern.to_lowercase();

        match anchoring {
            Anchoring::Is => query_lower == pattern_lower,
            Anchoring::StartsWith => query_lower.starts_with(&pattern_lower),
            Anchoring::EndsWith => query_lower.ends_with(&pattern_lower),
            Anchoring::Contains => query_lower.contains(&pattern_lower),
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct RuleEffects {
    pub pins: Vec<(String, usize)>,
    pub hidden: Vec<String>,
    pub user_data: Vec<serde_json::Value>,
    pub applied_rules: Vec<String>,
    pub query_rewrite: Option<String>,
}

pub struct RuleStore {
    rules: IndexMap<String, Rule>,
}

impl Default for RuleStore {
    fn default() -> Self {
        Self::new()
    }
}

impl RuleStore {
    pub fn new() -> Self {
        RuleStore {
            rules: IndexMap::new(),
        }
    }

    pub fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let rules: Vec<Rule> = serde_json::from_str(&content)?;

        let mut store = RuleStore::new();
        for rule in rules {
            store.rules.insert(rule.object_id.clone(), rule);
        }
        Ok(store)
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let rules: Vec<&Rule> = self.rules.values().collect();
        let content = serde_json::to_string_pretty(&rules)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    pub fn get(&self, object_id: &str) -> Option<&Rule> {
        self.rules.get(object_id)
    }

    pub fn insert(&mut self, rule: Rule) {
        self.rules.insert(rule.object_id.clone(), rule);
    }

    pub fn remove(&mut self, object_id: &str) -> Option<Rule> {
        self.rules.shift_remove(object_id)
    }

    pub fn clear(&mut self) {
        self.rules.clear();
    }

    pub fn all(&self) -> Vec<Rule> {
        self.rules.values().cloned().collect()
    }

    pub fn search(&self, query: &str, page: usize, hits_per_page: usize) -> (Vec<Rule>, usize) {
        let query_lower = query.to_lowercase();

        let mut matching: Vec<Rule> = self
            .rules
            .values()
            .filter(|rule| {
                if query.is_empty() {
                    return true;
                }

                if rule.object_id.to_lowercase().contains(&query_lower) {
                    return true;
                }

                if let Some(ref desc) = rule.description {
                    if desc.to_lowercase().contains(&query_lower) {
                        return true;
                    }
                }

                for condition in &rule.conditions {
                    if condition.pattern.to_lowercase().contains(&query_lower) {
                        return true;
                    }
                }

                false
            })
            .cloned()
            .collect();

        matching.sort_by(|a, b| a.object_id.cmp(&b.object_id));

        let total = matching.len();
        let start = page * hits_per_page;
        let end = (start + hits_per_page).min(total);

        let hits = if start < total {
            matching[start..end].to_vec()
        } else {
            Vec::new()
        };

        (hits, total)
    }

    pub fn apply_rules(&self, query_text: &str, context: Option<&str>) -> RuleEffects {
        let mut effects = RuleEffects::default();

        for rule in self.rules.values() {
            if !rule.matches(query_text, context) {
                continue;
            }

            effects.applied_rules.push(rule.object_id.clone());

            if let Some(promote) = &rule.consequence.promote {
                for p in promote {
                    match p {
                        Promote::Single {
                            object_id,
                            position,
                        } => {
                            effects.pins.push((object_id.clone(), *position));
                        }
                        Promote::Multiple {
                            object_ids,
                            position,
                        } => {
                            for (idx, id) in object_ids.iter().enumerate() {
                                effects.pins.push((id.clone(), position + idx));
                            }
                        }
                    }
                }
            }

            if let Some(hide) = &rule.consequence.hide {
                for h in hide {
                    effects.hidden.push(h.object_id.clone());
                }
            }

            if let Some(user_data) = &rule.consequence.user_data {
                effects.user_data.push(user_data.clone());
            }
        }

        effects.pins.sort_by_key(|(_, pos)| *pos);

        effects
    }

    pub fn apply_query_rewrite(&self, query_text: &str, context: Option<&str>) -> Option<String> {
        for rule in self.rules.values() {
            if !rule.matches(query_text, context) {
                continue;
            }

            if let Some(ref params) = rule.consequence.params {
                if let Some(ref query) = params.query {
                    return Some(query.clone());
                }
            }
        }
        None
    }
}
