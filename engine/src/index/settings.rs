use crate::query::plurals::IgnorePluralsValue;
use crate::query::stopwords::RemoveStopWordsValue;
use serde::{Deserialize, Serialize, Serializer};
use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;

fn default_hits_per_page() -> u32 {
    20
}

fn serialize_vec_as_null_if_empty<S>(vec: &Vec<String>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    if vec.is_empty() {
        serializer.serialize_none()
    } else {
        vec.serialize(serializer)
    }
}

fn deserialize_null_as_empty_vec<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Option::<Vec<String>>::deserialize(deserializer).map(|opt| opt.unwrap_or_default())
}

fn remove_stop_words_is_default(v: &RemoveStopWordsValue) -> bool {
    matches!(v, RemoveStopWordsValue::Disabled)
}

fn ignore_plurals_is_default(v: &IgnorePluralsValue) -> bool {
    matches!(v, IgnorePluralsValue::Disabled)
}

fn vec_is_empty(v: &[String]) -> bool {
    v.is_empty()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub enum IndexMode {
    KeywordSearch,
    NeuralSearch,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SemanticSearchSettings {
    #[serde(rename = "eventSources", skip_serializing_if = "Option::is_none")]
    pub event_sources: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct IndexSettings {
    #[serde(
        rename = "attributesForFaceting",
        serialize_with = "serialize_vec_as_null_if_empty",
        deserialize_with = "deserialize_null_as_empty_vec"
    )]
    pub attributes_for_faceting: Vec<String>,

    #[serde(rename = "searchableAttributes")]
    pub searchable_attributes: Option<Vec<String>>,

    #[serde(rename = "ranking")]
    pub ranking: Option<Vec<String>>,

    #[serde(rename = "customRanking")]
    pub custom_ranking: Option<Vec<String>>,

    #[serde(rename = "attributesToRetrieve")]
    pub attributes_to_retrieve: Option<Vec<String>>,

    #[serde(rename = "unretrievableAttributes")]
    pub unretrievable_attributes: Option<Vec<String>>,

    #[serde(rename = "attributesToHighlight")]
    pub attributes_to_highlight: Option<Vec<String>>,

    #[serde(rename = "attributesToSnippet")]
    pub attributes_to_snippet: Option<Vec<String>>,

    #[serde(rename = "highlightPreTag")]
    pub highlight_pre_tag: Option<String>,

    #[serde(rename = "highlightPostTag")]
    pub highlight_post_tag: Option<String>,

    #[serde(rename = "hitsPerPage", default = "default_hits_per_page")]
    pub hits_per_page: u32,

    #[serde(rename = "minWordSizefor1Typo")]
    pub min_word_size_for_1_typo: u32,

    #[serde(rename = "minWordSizefor2Typos")]
    pub min_word_size_for_2_typos: u32,

    #[serde(rename = "maxValuesPerFacet")]
    pub max_values_per_facet: u32,

    #[serde(rename = "paginationLimitedTo")]
    pub pagination_limited_to: u32,

    #[serde(rename = "exactOnSingleWordQuery")]
    pub exact_on_single_word_query: String,

    #[serde(rename = "queryType")]
    pub query_type: String,

    #[serde(rename = "removeWordsIfNoResults")]
    pub remove_words_if_no_results: String,

    #[serde(rename = "separatorsToIndex")]
    pub separators_to_index: String,

    #[serde(
        rename = "alternativesAsExact",
        serialize_with = "serialize_vec_as_null_if_empty",
        deserialize_with = "deserialize_null_as_empty_vec"
    )]
    pub alternatives_as_exact: Vec<String>,

    #[serde(
        rename = "optionalWords",
        serialize_with = "serialize_vec_as_null_if_empty",
        deserialize_with = "deserialize_null_as_empty_vec"
    )]
    pub optional_words: Vec<String>,

    #[serde(rename = "numericAttributesToIndex")]
    pub numeric_attributes_to_index: Option<Vec<String>>,

    #[serde(rename = "attributesToIndex", skip_serializing_if = "Option::is_none")]
    pub attributes_to_index: Option<Vec<String>>,

    pub version: u32,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub synonyms: Option<serde_json::Value>,

    #[serde(rename = "attributeForDistinct")]
    pub attribute_for_distinct: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub distinct: Option<DistinctValue>,

    #[serde(
        rename = "removeStopWords",
        default,
        skip_serializing_if = "remove_stop_words_is_default"
    )]
    pub remove_stop_words: RemoveStopWordsValue,

    #[serde(
        rename = "queryLanguages",
        default,
        skip_serializing_if = "vec_is_empty"
    )]
    pub query_languages: Vec<String>,

    #[serde(
        rename = "ignorePlurals",
        default,
        skip_serializing_if = "ignore_plurals_is_default"
    )]
    pub ignore_plurals: IgnorePluralsValue,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub embedders: Option<HashMap<String, serde_json::Value>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<IndexMode>,

    #[serde(rename = "semanticSearch", skip_serializing_if = "Option::is_none")]
    pub semantic_search: Option<SemanticSearchSettings>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum DistinctValue {
    Bool(bool),
    Integer(u32),
}

impl Default for IndexSettings {
    fn default() -> Self {
        IndexSettings {
            attributes_for_faceting: Vec::new(),
            searchable_attributes: None,
            ranking: Some(vec![
                "typo".to_string(),
                "geo".to_string(),
                "words".to_string(),
                "filters".to_string(),
                "proximity".to_string(),
                "attribute".to_string(),
                "exact".to_string(),
                "custom".to_string(),
            ]),
            custom_ranking: None,
            attributes_to_retrieve: None,
            unretrievable_attributes: None,
            attributes_to_highlight: None,
            attributes_to_snippet: None,
            highlight_pre_tag: Some("<em>".to_string()),
            highlight_post_tag: Some("</em>".to_string()),
            hits_per_page: 20,
            min_word_size_for_1_typo: 4,
            min_word_size_for_2_typos: 8,
            max_values_per_facet: 100,
            pagination_limited_to: 1000,
            exact_on_single_word_query: "attribute".to_string(),
            query_type: "prefixLast".to_string(),
            remove_words_if_no_results: "none".to_string(),
            separators_to_index: "".to_string(),
            alternatives_as_exact: vec![
                "ignorePlurals".to_string(),
                "singleWordSynonym".to_string(),
            ],
            optional_words: Vec::new(),
            numeric_attributes_to_index: None,
            attributes_to_index: None,
            version: 1,
            synonyms: None,
            attribute_for_distinct: None,
            distinct: None,
            remove_stop_words: RemoveStopWordsValue::Disabled,
            query_languages: Vec::new(),
            ignore_plurals: IgnorePluralsValue::Disabled,
            embedders: None,
            mode: None,
            semantic_search: None,
        }
    }
}

impl DistinctValue {
    pub fn as_count(&self) -> u32 {
        match self {
            DistinctValue::Bool(false) => 0,
            DistinctValue::Bool(true) => 1,
            DistinctValue::Integer(n) => *n,
        }
    }
}

impl IndexSettings {
    pub fn load<P: AsRef<Path>>(path: P) -> crate::error::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let settings: IndexSettings = serde_json::from_str(&content)?;
        Ok(settings)
    }

    pub fn save<P: AsRef<Path>>(&self, path: P) -> crate::error::Result<()> {
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    pub fn facet_set(&self) -> HashSet<String> {
        self.attributes_for_faceting
            .iter()
            .map(|s| parse_facet_modifier(s))
            .collect()
    }

    pub fn searchable_facet_set(&self) -> HashSet<String> {
        self.attributes_for_faceting
            .iter()
            .filter(|s| s.starts_with("searchable("))
            .map(|s| parse_facet_modifier(s))
            .collect()
    }

    pub fn default_with_facets(facets: Vec<String>) -> Self {
        Self {
            attributes_for_faceting: facets,
            ..Self::default()
        }
    }

    pub fn should_retrieve(&self, field: &str) -> bool {
        if let Some(unretrievable) = &self.unretrievable_attributes {
            if unretrievable.contains(&field.to_string()) {
                return false;
            }
        }

        if let Some(retrievable) = &self.attributes_to_retrieve {
            if retrievable.contains(&"*".to_string()) {
                return true;
            }
            return retrievable.contains(&field.to_string());
        }

        true
    }

    pub fn is_neural_search_active(&self) -> bool {
        matches!(self.mode, Some(IndexMode::NeuralSearch))
    }

    /// Validate embedder configurations. Returns Ok(()) if no embedders or if
    /// the vector-search feature is not enabled. With the feature, each config
    /// is parsed into EmbedderConfig and validated.
    pub fn validate_embedders(&self) -> Result<(), String> {
        let embedders = match &self.embedders {
            Some(map) if !map.is_empty() => map,
            _ => return Ok(()),
        };
        self.validate_embedders_inner(embedders)
    }

    #[cfg(feature = "vector-search")]
    fn validate_embedders_inner(
        &self,
        embedders: &HashMap<String, serde_json::Value>,
    ) -> Result<(), String> {
        use crate::vector::config::EmbedderConfig;
        for (name, value) in embedders {
            if value.is_null() {
                continue;
            }
            let config: EmbedderConfig = serde_json::from_value(value.clone())
                .map_err(|e| format!("embedder '{}': {}", name, e))?;
            config
                .validate()
                .map_err(|e| format!("embedder '{}': {}", name, e))?;
        }
        Ok(())
    }

    #[cfg(not(feature = "vector-search"))]
    fn validate_embedders_inner(
        &self,
        embedders: &HashMap<String, serde_json::Value>,
    ) -> Result<(), String> {
        let _ = embedders;
        Ok(())
    }
}

#[derive(Debug, PartialEq)]
pub enum EmbedderChange {
    Added(String),
    Removed(String),
    Modified(String),
}

pub fn detect_embedder_changes(
    old: &Option<HashMap<String, serde_json::Value>>,
    new: &Option<HashMap<String, serde_json::Value>>,
) -> Vec<EmbedderChange> {
    let empty = HashMap::new();
    let old_map = old.as_ref().unwrap_or(&empty);
    let new_map = new.as_ref().unwrap_or(&empty);

    let mut changes = Vec::new();
    let mut all_keys: HashSet<&String> = old_map.keys().collect();
    all_keys.extend(new_map.keys());

    for key in all_keys {
        match (old_map.get(key), new_map.get(key)) {
            (None, Some(_)) => changes.push(EmbedderChange::Added(key.clone())),
            (Some(_), None) => changes.push(EmbedderChange::Removed(key.clone())),
            (Some(old_val), Some(new_val)) => {
                let fields_changed = ["source", "model", "dimensions"]
                    .iter()
                    .any(|field| old_val.get(field) != new_val.get(field));
                if fields_changed {
                    changes.push(EmbedderChange::Modified(key.clone()));
                }
            }
            (None, None) => unreachable!(),
        }
    }

    changes
}

fn parse_facet_modifier(attr: &str) -> String {
    if let Some(stripped) = attr.strip_prefix("filterOnly(") {
        stripped.trim_end_matches(')').to_string()
    } else if let Some(stripped) = attr.strip_prefix("searchable(") {
        stripped.trim_end_matches(')').to_string()
    } else if let Some(stripped) = attr.strip_prefix("afterDistinct(") {
        stripped.trim_end_matches(')').to_string()
    } else {
        attr.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_modifiers() {
        assert_eq!(parse_facet_modifier("category"), "category");
        assert_eq!(parse_facet_modifier("filterOnly(price)"), "price");
        assert_eq!(parse_facet_modifier("searchable(brand)"), "brand");
    }

    #[test]
    fn test_facet_set() {
        let settings = IndexSettings {
            attributes_for_faceting: vec![
                "category".to_string(),
                "filterOnly(price)".to_string(),
                "searchable(brand)".to_string(),
            ],
            ..Default::default()
        };

        let facets = settings.facet_set();
        assert!(facets.contains("category"));
        assert!(facets.contains("price"));
        assert!(facets.contains("brand"));
    }

    #[test]
    fn test_distinct_value() {
        let bool_false = DistinctValue::Bool(false);
        assert_eq!(bool_false.as_count(), 0);

        let bool_true = DistinctValue::Bool(true);
        assert_eq!(bool_true.as_count(), 1);

        let int_val = DistinctValue::Integer(3);
        assert_eq!(int_val.as_count(), 3);
    }

    #[test]
    fn test_settings_roundtrip_preserves_all_fields() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("settings.json");

        let original = IndexSettings {
            attributes_for_faceting: vec!["category".to_string(), "filterOnly(price)".to_string()],
            searchable_attributes: Some(vec!["title".to_string(), "brand".to_string()]),
            custom_ranking: Some(vec!["desc(popularity)".to_string()]),
            attributes_to_retrieve: Some(vec!["title".to_string(), "price".to_string()]),
            unretrievable_attributes: Some(vec!["internal_id".to_string()]),
            attribute_for_distinct: Some("product_id".to_string()),
            distinct: Some(DistinctValue::Integer(2)),
            ..Default::default()
        };

        original.save(&path).unwrap();
        let loaded = IndexSettings::load(&path).unwrap();

        assert_eq!(
            loaded.attributes_for_faceting, original.attributes_for_faceting,
            "attributes_for_faceting mismatch"
        );
        assert_eq!(
            loaded.searchable_attributes, original.searchable_attributes,
            "searchable_attributes mismatch"
        );
        assert_eq!(loaded.ranking, original.ranking, "ranking mismatch");
        assert_eq!(
            loaded.custom_ranking, original.custom_ranking,
            "custom_ranking mismatch"
        );
        assert_eq!(
            loaded.attributes_to_retrieve, original.attributes_to_retrieve,
            "attributes_to_retrieve mismatch"
        );
        assert_eq!(
            loaded.unretrievable_attributes, original.unretrievable_attributes,
            "unretrievable_attributes mismatch"
        );
        assert_eq!(
            loaded.attribute_for_distinct, original.attribute_for_distinct,
            "attribute_for_distinct mismatch"
        );
        assert_eq!(loaded.distinct, original.distinct, "distinct mismatch");
    }

    #[test]
    fn test_partial_json_uses_defaults() {
        let json = r#"{"queryType":"prefixAll"}"#;
        let settings: IndexSettings = serde_json::from_str(json).unwrap();
        assert_eq!(settings.query_type, "prefixAll");
        assert_eq!(settings.min_word_size_for_1_typo, 4); // default value
    }

    // ── Embedders field tests (4.1) ──

    #[test]
    fn test_settings_embedders_default_none() {
        let settings = IndexSettings::default();
        assert!(settings.embedders.is_none());
    }

    #[test]
    fn test_settings_embedders_roundtrip() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("settings.json");

        let mut embedders = HashMap::new();
        embedders.insert(
            "default".to_string(),
            serde_json::json!({"source": "userProvided", "dimensions": 384}),
        );

        let original = IndexSettings {
            embedders: Some(embedders),
            ..Default::default()
        };

        original.save(&path).unwrap();
        let loaded = IndexSettings::load(&path).unwrap();

        let loaded_embedders = loaded
            .embedders
            .as_ref()
            .expect("embedders should survive roundtrip");
        let default_config = loaded_embedders
            .get("default")
            .expect("should have 'default' key");
        assert_eq!(default_config["source"], "userProvided");
        assert_eq!(default_config["dimensions"], 384);
    }

    #[test]
    fn test_settings_embedders_skip_serializing_when_none() {
        let settings = IndexSettings::default();
        let json_str = serde_json::to_string(&settings).unwrap();
        assert!(
            !json_str.contains("embedders"),
            "JSON should not contain 'embedders' when None"
        );
    }

    #[test]
    fn test_settings_embedders_partial_update() {
        let json = r#"{"embedders": {"myEmb": {"source": "userProvided", "dimensions": 128}}}"#;
        let settings: IndexSettings = serde_json::from_str(json).unwrap();

        // Embedders populated
        let emb = settings.embedders.as_ref().unwrap();
        assert_eq!(emb["myEmb"]["dimensions"], 128);

        // Other fields got defaults
        assert_eq!(settings.hits_per_page, 20);
        assert_eq!(settings.query_type, "prefixLast");
    }

    #[test]
    fn test_settings_backward_compat_no_embedders() {
        let json = r#"{"queryType":"prefixAll","hitsPerPage":10}"#;
        let settings: IndexSettings = serde_json::from_str(json).unwrap();
        assert!(settings.embedders.is_none());
        assert_eq!(settings.query_type, "prefixAll");
        assert_eq!(settings.hits_per_page, 10);
    }

    // ── Embedder validation tests (4.5) ──

    #[cfg(feature = "vector-search")]
    #[test]
    fn test_validate_embedders_valid() {
        let mut embedders = HashMap::new();
        embedders.insert(
            "default".to_string(),
            serde_json::json!({"source": "userProvided", "dimensions": 384}),
        );
        let settings = IndexSettings {
            embedders: Some(embedders),
            ..Default::default()
        };
        assert!(settings.validate_embedders().is_ok());
    }

    #[cfg(feature = "vector-search")]
    #[test]
    fn test_validate_embedders_invalid_source() {
        let mut embedders = HashMap::new();
        embedders.insert(
            "broken".to_string(),
            serde_json::json!({"source": "nonExistent"}),
        );
        let settings = IndexSettings {
            embedders: Some(embedders),
            ..Default::default()
        };
        let err = settings.validate_embedders().unwrap_err();
        assert!(
            err.contains("broken"),
            "error should mention embedder name: {}",
            err
        );
    }

    #[cfg(feature = "vector-search")]
    #[test]
    fn test_validate_embedders_missing_required_field() {
        let mut embedders = HashMap::new();
        embedders.insert("myEmb".to_string(), serde_json::json!({"source": "openAi"}));
        let settings = IndexSettings {
            embedders: Some(embedders),
            ..Default::default()
        };
        let err = settings.validate_embedders().unwrap_err();
        assert!(
            err.contains("myEmb"),
            "error should mention embedder name: {}",
            err
        );
        assert!(
            err.contains("apiKey"),
            "error should mention missing field: {}",
            err
        );
    }

    #[cfg(feature = "vector-search")]
    #[test]
    fn test_validate_embedders_null_value_skipped() {
        let mut embedders = HashMap::new();
        embedders.insert("default".to_string(), serde_json::Value::Null);
        let settings = IndexSettings {
            embedders: Some(embedders),
            ..Default::default()
        };
        assert!(settings.validate_embedders().is_ok());
    }

    #[test]
    fn test_validate_embedders_none_is_ok() {
        let settings = IndexSettings::default();
        assert!(settings.validate_embedders().is_ok());
    }

    #[test]
    fn test_validate_embedders_empty_map_is_ok() {
        let settings = IndexSettings {
            embedders: Some(HashMap::new()),
            ..Default::default()
        };
        assert!(settings.validate_embedders().is_ok());
    }

    // ── Stale vector detection tests (4.8) ──

    #[test]
    fn test_embedder_changes_detects_model_change() {
        let old = Some(HashMap::from([(
            "emb1".to_string(),
            serde_json::json!({"source": "openAi", "model": "A"}),
        )]));
        let new = Some(HashMap::from([(
            "emb1".to_string(),
            serde_json::json!({"source": "openAi", "model": "B"}),
        )]));
        let changes = detect_embedder_changes(&old, &new);
        assert_eq!(changes, vec![EmbedderChange::Modified("emb1".to_string())]);
    }

    #[test]
    fn test_embedder_changes_detects_source_change() {
        let old = Some(HashMap::from([(
            "emb1".to_string(),
            serde_json::json!({"source": "openAi"}),
        )]));
        let new = Some(HashMap::from([(
            "emb1".to_string(),
            serde_json::json!({"source": "rest"}),
        )]));
        let changes = detect_embedder_changes(&old, &new);
        assert_eq!(changes, vec![EmbedderChange::Modified("emb1".to_string())]);
    }

    #[test]
    fn test_embedder_changes_detects_dimensions_change() {
        let old = Some(HashMap::from([(
            "emb1".to_string(),
            serde_json::json!({"source": "userProvided", "dimensions": 128}),
        )]));
        let new = Some(HashMap::from([(
            "emb1".to_string(),
            serde_json::json!({"source": "userProvided", "dimensions": 384}),
        )]));
        let changes = detect_embedder_changes(&old, &new);
        assert_eq!(changes, vec![EmbedderChange::Modified("emb1".to_string())]);
    }

    #[test]
    fn test_embedder_changes_unchanged() {
        let config = serde_json::json!({"source": "userProvided", "dimensions": 384});
        let old = Some(HashMap::from([("emb1".to_string(), config.clone())]));
        let new = Some(HashMap::from([("emb1".to_string(), config)]));
        let changes = detect_embedder_changes(&old, &new);
        assert!(changes.is_empty());
    }

    #[test]
    fn test_embedder_changes_new_embedder() {
        let old: Option<HashMap<String, serde_json::Value>> = Some(HashMap::new());
        let new = Some(HashMap::from([(
            "new_emb".to_string(),
            serde_json::json!({"source": "userProvided", "dimensions": 128}),
        )]));
        let changes = detect_embedder_changes(&old, &new);
        assert_eq!(changes, vec![EmbedderChange::Added("new_emb".to_string())]);
    }

    #[test]
    fn test_embedder_changes_removed_embedder() {
        let old = Some(HashMap::from([(
            "old_emb".to_string(),
            serde_json::json!({"source": "userProvided", "dimensions": 128}),
        )]));
        let new: Option<HashMap<String, serde_json::Value>> = Some(HashMap::new());
        let changes = detect_embedder_changes(&old, &new);
        assert_eq!(
            changes,
            vec![EmbedderChange::Removed("old_emb".to_string())]
        );
    }

    #[test]
    fn test_embedder_changes_both_none() {
        let changes = detect_embedder_changes(&None, &None);
        assert!(changes.is_empty());
    }

    // ── Mode and SemanticSearch tests (5.2) ──

    #[test]
    fn test_settings_mode_default_none() {
        let settings = IndexSettings::default();
        assert!(settings.mode.is_none());
    }

    #[test]
    fn test_settings_mode_roundtrip() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("settings.json");

        let original = IndexSettings {
            mode: Some(IndexMode::NeuralSearch),
            ..Default::default()
        };

        original.save(&path).unwrap();
        let loaded = IndexSettings::load(&path).unwrap();

        assert_eq!(loaded.mode, Some(IndexMode::NeuralSearch));
    }

    #[test]
    fn test_settings_mode_skip_serializing_when_none() {
        let settings = IndexSettings::default();
        let json_str = serde_json::to_string(&settings).unwrap();
        assert!(
            !json_str.contains("\"mode\""),
            "JSON should not contain 'mode' when None"
        );
    }

    #[test]
    fn test_settings_mode_serde_values() {
        // Deserialize
        let neural: IndexMode = serde_json::from_str("\"neuralSearch\"").unwrap();
        assert_eq!(neural, IndexMode::NeuralSearch);

        let keyword: IndexMode = serde_json::from_str("\"keywordSearch\"").unwrap();
        assert_eq!(keyword, IndexMode::KeywordSearch);

        // Serialize
        assert_eq!(
            serde_json::to_string(&IndexMode::NeuralSearch).unwrap(),
            "\"neuralSearch\""
        );
        assert_eq!(
            serde_json::to_string(&IndexMode::KeywordSearch).unwrap(),
            "\"keywordSearch\""
        );
    }

    #[test]
    fn test_settings_mode_backward_compat() {
        let json = r#"{"queryType":"prefixAll","hitsPerPage":10}"#;
        let settings: IndexSettings = serde_json::from_str(json).unwrap();
        assert!(settings.mode.is_none());
        assert_eq!(settings.query_type, "prefixAll");
    }

    #[test]
    fn test_settings_semantic_search_default_none() {
        let settings = IndexSettings::default();
        assert!(settings.semantic_search.is_none());
    }

    #[test]
    fn test_settings_semantic_search_roundtrip() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("settings.json");

        let original = IndexSettings {
            semantic_search: Some(SemanticSearchSettings {
                event_sources: Some(vec!["idx1".to_string(), "idx2".to_string()]),
            }),
            ..Default::default()
        };

        original.save(&path).unwrap();
        let loaded = IndexSettings::load(&path).unwrap();

        let ss = loaded
            .semantic_search
            .expect("semantic_search should survive roundtrip");
        assert_eq!(
            ss.event_sources,
            Some(vec!["idx1".to_string(), "idx2".to_string()])
        );
    }

    #[test]
    fn test_settings_semantic_search_event_sources() {
        let ss = SemanticSearchSettings {
            event_sources: Some(vec!["idx1".to_string()]),
        };
        let json_str = serde_json::to_string(&ss).unwrap();
        assert!(
            json_str.contains("eventSources"),
            "should use camelCase: {}",
            json_str
        );
        assert!(
            !json_str.contains("event_sources"),
            "should not use snake_case: {}",
            json_str
        );

        // Roundtrip
        let deserialized: SemanticSearchSettings = serde_json::from_str(&json_str).unwrap();
        assert_eq!(deserialized.event_sources, Some(vec!["idx1".to_string()]));
    }

    #[test]
    fn test_settings_is_neural_search_active() {
        let mut settings = IndexSettings::default();
        assert!(
            !settings.is_neural_search_active(),
            "default should not be neural"
        );

        settings.mode = Some(IndexMode::KeywordSearch);
        assert!(
            !settings.is_neural_search_active(),
            "keywordSearch should not be neural"
        );

        settings.mode = Some(IndexMode::NeuralSearch);
        assert!(
            settings.is_neural_search_active(),
            "neuralSearch should be neural"
        );
    }
}
