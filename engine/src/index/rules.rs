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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn bare_rule(id: &str) -> Rule {
        Rule {
            object_id: id.to_string(),
            conditions: vec![],
            consequence: Consequence {
                promote: None,
                hide: None,
                filter_promotes: None,
                user_data: None,
                params: None,
            },
            description: None,
            enabled: None,
            validity: None,
        }
    }

    fn rule_with_pattern(id: &str, pattern: &str, anchoring: Anchoring) -> Rule {
        Rule {
            object_id: id.to_string(),
            conditions: vec![Condition {
                pattern: pattern.to_string(),
                anchoring,
                alternatives: None,
                context: None,
                filters: None,
            }],
            consequence: Consequence {
                promote: None,
                hide: None,
                filter_promotes: None,
                user_data: None,
                params: None,
            },
            description: None,
            enabled: None,
            validity: None,
        }
    }

    // --- Rule::is_enabled ---

    #[test]
    fn enabled_defaults_to_true() {
        let r = bare_rule("x");
        assert!(r.is_enabled());
    }

    #[test]
    fn enabled_explicit_true() {
        let mut r = bare_rule("x");
        r.enabled = Some(true);
        assert!(r.is_enabled());
    }

    #[test]
    fn enabled_explicit_false() {
        let mut r = bare_rule("x");
        r.enabled = Some(false);
        assert!(!r.is_enabled());
    }

    // --- Rule::is_valid_at ---

    #[test]
    fn validity_none_always_valid() {
        let r = bare_rule("x");
        assert!(r.is_valid_at(0));
        assert!(r.is_valid_at(i64::MAX));
    }

    #[test]
    fn validity_within_range() {
        let mut r = bare_rule("x");
        r.validity = Some(vec![TimeRange {
            from: 1000,
            until: 2000,
        }]);
        assert!(r.is_valid_at(1000));
        assert!(r.is_valid_at(1500));
        assert!(r.is_valid_at(2000));
    }

    #[test]
    fn validity_outside_range() {
        let mut r = bare_rule("x");
        r.validity = Some(vec![TimeRange {
            from: 1000,
            until: 2000,
        }]);
        assert!(!r.is_valid_at(999));
        assert!(!r.is_valid_at(2001));
    }

    #[test]
    fn validity_multiple_ranges_matches_any() {
        let mut r = bare_rule("x");
        r.validity = Some(vec![
            TimeRange {
                from: 100,
                until: 200,
            },
            TimeRange {
                from: 500,
                until: 600,
            },
        ]);
        assert!(r.is_valid_at(150));
        assert!(r.is_valid_at(550));
        assert!(!r.is_valid_at(350));
    }

    // --- Rule::matches (no conditions → always matches) ---

    #[test]
    fn no_conditions_always_matches() {
        let r = bare_rule("x");
        assert!(r.matches("anything", None));
        assert!(r.matches("", None));
    }

    #[test]
    fn disabled_rule_never_matches() {
        let mut r = rule_with_pattern("x", "laptop", Anchoring::Is);
        r.enabled = Some(false);
        assert!(!r.matches("laptop", None));
    }

    // --- RuleStore::apply_rules ---

    #[test]
    fn apply_rules_promotes_single() {
        let mut store = RuleStore::new();
        let mut rule = rule_with_pattern("r1", "laptop", Anchoring::Is);
        rule.consequence.promote = Some(vec![Promote::Single {
            object_id: "doc-1".to_string(),
            position: 0,
        }]);
        store.insert(rule);

        let effects = store.apply_rules("laptop", None);
        assert_eq!(effects.applied_rules, vec!["r1"]);
        assert_eq!(effects.pins, vec![("doc-1".to_string(), 0)]);
        assert!(effects.hidden.is_empty());
    }

    #[test]
    fn apply_rules_promotes_multiple() {
        let mut store = RuleStore::new();
        let mut rule = rule_with_pattern("r1", "sale", Anchoring::Contains);
        rule.consequence.promote = Some(vec![Promote::Multiple {
            object_ids: vec!["a".to_string(), "b".to_string(), "c".to_string()],
            position: 2,
        }]);
        store.insert(rule);

        let effects = store.apply_rules("big sale today", None);
        assert_eq!(
            effects.pins,
            vec![
                ("a".to_string(), 2),
                ("b".to_string(), 3),
                ("c".to_string(), 4),
            ]
        );
    }

    #[test]
    fn apply_rules_hides() {
        let mut store = RuleStore::new();
        let mut rule = rule_with_pattern("r1", "laptop", Anchoring::Is);
        rule.consequence.hide = Some(vec![Hide {
            object_id: "bad-doc".to_string(),
        }]);
        store.insert(rule);

        let effects = store.apply_rules("laptop", None);
        assert_eq!(effects.hidden, vec!["bad-doc"]);
    }

    #[test]
    fn apply_rules_user_data() {
        let mut store = RuleStore::new();
        let mut rule = rule_with_pattern("r1", "promo", Anchoring::Contains);
        rule.consequence.user_data = Some(json!({"banner": "sale"}));
        store.insert(rule);

        let effects = store.apply_rules("promo items", None);
        assert_eq!(effects.user_data, vec![json!({"banner": "sale"})]);
    }

    #[test]
    fn apply_rules_no_match_returns_empty() {
        let mut store = RuleStore::new();
        store.insert(rule_with_pattern("r1", "laptop", Anchoring::Is));

        let effects = store.apply_rules("phone", None);
        assert!(effects.applied_rules.is_empty());
        assert!(effects.pins.is_empty());
    }

    #[test]
    fn apply_rules_pins_sorted_by_position() {
        // Two rules both match; their pins should come out sorted
        let mut store = RuleStore::new();
        let mut r1 = rule_with_pattern("r1", "sale", Anchoring::Contains);
        r1.consequence.promote = Some(vec![Promote::Single {
            object_id: "b".to_string(),
            position: 5,
        }]);
        let mut r2 = rule_with_pattern("r2", "sale", Anchoring::Contains);
        r2.consequence.promote = Some(vec![Promote::Single {
            object_id: "a".to_string(),
            position: 1,
        }]);
        store.insert(r1);
        store.insert(r2);

        let effects = store.apply_rules("sale", None);
        // pins sorted by position: 1, then 5
        assert_eq!(effects.pins[0], ("a".to_string(), 1));
        assert_eq!(effects.pins[1], ("b".to_string(), 5));
    }

    // --- RuleStore::apply_query_rewrite ---

    #[test]
    fn query_rewrite_matches() {
        let mut store = RuleStore::new();
        let mut rule = rule_with_pattern("r1", "tv", Anchoring::Is);
        rule.consequence.params = Some(ConsequenceParams {
            query: Some("television".to_string()),
        });
        store.insert(rule);

        assert_eq!(
            store.apply_query_rewrite("tv", None),
            Some("television".to_string())
        );
    }

    #[test]
    fn query_rewrite_no_match() {
        let mut store = RuleStore::new();
        let mut rule = rule_with_pattern("r1", "tv", Anchoring::Is);
        rule.consequence.params = Some(ConsequenceParams {
            query: Some("television".to_string()),
        });
        store.insert(rule);

        assert_eq!(store.apply_query_rewrite("phone", None), None);
    }

    // --- RuleStore::search ---

    #[test]
    fn search_empty_query_returns_all() {
        let mut store = RuleStore::new();
        store.insert(bare_rule("alpha"));
        store.insert(bare_rule("beta"));
        store.insert(bare_rule("gamma"));

        let (hits, total) = store.search("", 0, 10);
        assert_eq!(total, 3);
        assert_eq!(hits.len(), 3);
    }

    #[test]
    fn search_filters_by_id() {
        let mut store = RuleStore::new();
        store.insert(bare_rule("laptop-rule"));
        store.insert(bare_rule("phone-rule"));

        let (hits, total) = store.search("laptop", 0, 10);
        assert_eq!(total, 1);
        assert_eq!(hits[0].object_id, "laptop-rule");
    }

    #[test]
    fn search_pagination() {
        let mut store = RuleStore::new();
        for i in 0..5 {
            store.insert(bare_rule(&format!("rule-{}", i)));
        }

        let (page0, total) = store.search("", 0, 2);
        assert_eq!(total, 5);
        assert_eq!(page0.len(), 2);

        let (page1, _) = store.search("", 1, 2);
        assert_eq!(page1.len(), 2);

        let (page2, _) = store.search("", 2, 2);
        assert_eq!(page2.len(), 1);
    }

    #[test]
    fn search_past_end_returns_empty() {
        let mut store = RuleStore::new();
        store.insert(bare_rule("only-one"));

        let (hits, total) = store.search("", 5, 10);
        assert_eq!(total, 1);
        assert!(hits.is_empty());
    }

    #[test]
    fn search_filters_by_pattern() {
        let mut store = RuleStore::new();
        let mut r = bare_rule("boost-electronics");
        r.conditions.push(Condition {
            pattern: "gaming".to_string(),
            anchoring: Anchoring::Contains,
            alternatives: None,
            context: None,
            filters: None,
        });
        store.insert(r);
        store.insert(bare_rule("other-rule"));

        let (hits, total) = store.search("gaming", 0, 10);
        assert_eq!(total, 1);
        assert_eq!(hits[0].object_id, "boost-electronics");
    }

    // --- Anchoring variants (matches()) ---

    #[test]
    fn anchoring_is() {
        let r = rule_with_pattern("x", "laptop", Anchoring::Is);
        assert!(r.matches("laptop", None));
        assert!(r.matches("LAPTOP", None));
        assert!(!r.matches("gaming laptop", None));
        assert!(!r.matches("lapto", None));
    }

    #[test]
    fn anchoring_starts_with() {
        let r = rule_with_pattern("x", "gam", Anchoring::StartsWith);
        assert!(r.matches("gaming", None));
        assert!(r.matches("GAMing laptop", None));
        assert!(!r.matches("laptop gaming", None));
    }

    #[test]
    fn anchoring_ends_with() {
        let r = rule_with_pattern("x", "top", Anchoring::EndsWith);
        assert!(r.matches("laptop", None));
        assert!(r.matches("gaming LAPTOP", None));
        assert!(!r.matches("laptop gaming", None));
    }

    #[test]
    fn anchoring_contains() {
        let r = rule_with_pattern("x", "lap", Anchoring::Contains);
        assert!(r.matches("laptop", None));
        assert!(r.matches("gaming LAPTOP", None));
        assert!(r.matches("overlap", None));
        assert!(!r.matches("computer", None));
    }

    #[test]
    fn anchoring_is_empty_pattern() {
        let r = rule_with_pattern("x", "", Anchoring::Is);
        assert!(r.matches("", None));
        assert!(!r.matches("anything", None));
    }

    #[test]
    fn context_required_mismatched_skips_condition() {
        let mut r = rule_with_pattern("x", "laptop", Anchoring::Contains);
        r.conditions[0].context = Some("mobile".to_string());
        // context matches → matches
        assert!(r.matches("laptop", Some("mobile")));
        // context doesn't match → condition skipped → no conditions left → false
        assert!(!r.matches("laptop", Some("desktop")));
        assert!(!r.matches("laptop", None));
    }

    #[test]
    fn multi_condition_any_match() {
        let mut r = bare_rule("x");
        r.conditions.push(Condition {
            pattern: "laptop".to_string(),
            anchoring: Anchoring::Contains,
            alternatives: None,
            context: None,
            filters: None,
        });
        r.conditions.push(Condition {
            pattern: "computer".to_string(),
            anchoring: Anchoring::Contains,
            alternatives: None,
            context: None,
            filters: None,
        });
        assert!(r.matches("laptop", None));
        assert!(r.matches("computer", None));
        assert!(!r.matches("phone", None));
    }

    #[test]
    fn hide_and_pin_from_separate_rules() {
        let mut store = RuleStore::new();
        let mut r1 = rule_with_pattern("r1", "laptop", Anchoring::Contains);
        r1.consequence.promote = Some(vec![Promote::Single {
            object_id: "item1".to_string(),
            position: 0,
        }]);
        let mut r2 = rule_with_pattern("r2", "laptop", Anchoring::Contains);
        r2.consequence.hide = Some(vec![Hide {
            object_id: "item1".to_string(),
        }]);
        store.insert(r1);
        store.insert(r2);

        let effects = store.apply_rules("laptop", None);
        assert_eq!(effects.pins.len(), 1);
        assert_eq!(effects.hidden.len(), 1);
        assert_eq!(effects.pins[0].0, "item1");
        assert_eq!(effects.hidden[0], "item1");
    }

    #[test]
    fn multiple_pins_same_position() {
        let mut store = RuleStore::new();
        let mut r1 = rule_with_pattern("r1", "laptop", Anchoring::Contains);
        r1.consequence.promote = Some(vec![Promote::Single {
            object_id: "a".to_string(),
            position: 0,
        }]);
        let mut r2 = rule_with_pattern("r2", "laptop", Anchoring::Contains);
        r2.consequence.promote = Some(vec![Promote::Single {
            object_id: "b".to_string(),
            position: 0,
        }]);
        store.insert(r1);
        store.insert(r2);

        let effects = store.apply_rules("laptop", None);
        assert_eq!(effects.pins.len(), 2);
        assert!(effects.pins.iter().all(|(_, pos)| *pos == 0));
    }
}
