use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use utoipa::ToSchema;

/// Custom deserializer that accepts both a single string and an array of strings.
/// e.g. `"facets": "brand"` → `Some(vec!["brand"])` and `"facets": ["brand","category"]` → `Some(vec!["brand","category"])`
fn deserialize_string_or_vec<'de, D>(deserializer: D) -> Result<Option<Vec<String>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de;

    struct StringOrVec;

    impl<'de> de::Visitor<'de> for StringOrVec {
        type Value = Option<Vec<String>>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a string, an array of strings, or null")
        }

        fn visit_none<E: de::Error>(self) -> Result<Self::Value, E> {
            Ok(None)
        }

        fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
            Ok(None)
        }

        fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
            Ok(Some(vec![v.to_string()]))
        }

        fn visit_string<E: de::Error>(self, v: String) -> Result<Self::Value, E> {
            Ok(Some(vec![v]))
        }

        fn visit_seq<A: de::SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
            let mut vec = Vec::new();
            while let Some(s) = seq.next_element::<String>()? {
                vec.push(s);
            }
            Ok(Some(vec))
        }
    }

    deserializer.deserialize_any(StringOrVec)
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateIndexRequest {
    pub uid: String,
    #[serde(default)]
    pub schema: IndexSchema,
}

#[derive(Debug, Deserialize, Default, ToSchema)]
pub struct IndexSchema {
    #[serde(default)]
    pub text_fields: Vec<String>,
    #[serde(default)]
    pub integer_fields: Vec<String>,
    #[serde(default)]
    pub float_fields: Vec<String>,
    #[serde(default)]
    pub facet_fields: Vec<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct CreateIndexResponse {
    pub uid: String,
    pub created_at: String,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(untagged)]
pub enum AddDocumentsRequest {
    Batch {
        requests: Vec<BatchOperation>,
    },
    Legacy {
        documents: Vec<HashMap<String, serde_json::Value>>,
    },
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BatchOperation {
    pub action: String,
    pub body: HashMap<String, serde_json::Value>,
    #[serde(default)]
    pub create_if_not_exists: Option<bool>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(untagged)]
pub enum AddDocumentsResponse {
    Algolia {
        #[serde(rename = "taskID")]
        task_id: i64,
        #[serde(rename = "objectIDs")]
        object_ids: Vec<String>,
    },
    Legacy {
        task_uid: String,
        status: String,
        received_documents: usize,
    },
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TaskResponse {
    pub task_uid: String,
    pub status: String,
    pub received_documents: usize,
    pub indexed_documents: usize,
    pub rejected_documents: Vec<DocFailureDto>,
    pub rejected_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DocFailureDto {
    pub doc_id: String,
    pub error: String,
    pub message: String,
}

fn default_semantic_ratio() -> f64 {
    0.5
}

fn default_embedder_name() -> String {
    "default".to_string()
}

/// Parameters for hybrid (keyword + vector) search.
///
/// Meilisearch-style: `"hybrid": {"semanticRatio": 0.8, "embedder": "mymodel"}`
/// Algolia-style: synthesized internally when `mode: "neuralSearch"`.
#[derive(Debug, Deserialize, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct HybridSearchParams {
    #[serde(default = "default_semantic_ratio")]
    pub semantic_ratio: f64,
    #[serde(default = "default_embedder_name")]
    pub embedder: String,
}

impl HybridSearchParams {
    /// Clamp `semantic_ratio` to [0.0, 1.0].
    pub fn clamp_ratio(&mut self) {
        self.semantic_ratio = self.semantic_ratio.clamp(0.0, 1.0);
    }
}

#[derive(Debug, Default, Deserialize, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SearchRequest {
    #[serde(default)]
    pub index_name: Option<String>,
    #[serde(default)]
    pub query: String,
    #[serde(default)]
    pub filters: Option<String>,
    #[serde(default)]
    pub hits_per_page: Option<usize>,
    #[serde(default)]
    pub page: usize,
    #[serde(default, deserialize_with = "deserialize_string_or_vec")]
    pub facets: Option<Vec<String>>,
    #[serde(default)]
    pub sort: Option<Vec<String>>,
    #[serde(default)]
    pub distinct: Option<serde_json::Value>,
    #[serde(default)]
    pub highlight_pre_tag: Option<String>,
    #[serde(default)]
    pub highlight_post_tag: Option<String>,
    #[serde(default, rename = "attributesToRetrieve")]
    pub attributes_to_retrieve: Option<Vec<String>>,
    #[serde(default, rename = "attributesToHighlight")]
    pub attributes_to_highlight: Option<Vec<String>>,
    #[serde(default, rename = "attributesToSnippet")]
    pub attributes_to_snippet: Option<Vec<String>>,
    #[serde(default, rename = "queryType")]
    pub query_type_prefix: Option<String>,
    #[serde(default, rename = "typoTolerance")]
    pub typo_tolerance: Option<serde_json::Value>,
    #[serde(default, rename = "advancedSyntax")]
    pub advanced_syntax: Option<bool>,
    #[serde(default, rename = "removeWordsIfNoResults")]
    pub remove_words_if_no_results: Option<String>,
    #[serde(default, rename = "optionalFilters")]
    pub optional_filters: Option<serde_json::Value>,
    #[serde(default, rename = "enableSynonyms")]
    pub enable_synonyms: Option<bool>,
    #[serde(default, rename = "enableRules")]
    pub enable_rules: Option<bool>,
    #[serde(default, rename = "ruleContexts")]
    pub rule_contexts: Option<Vec<String>>,
    #[serde(default, rename = "restrictSearchableAttributes")]
    pub restrict_searchable_attributes: Option<Vec<String>>,
    #[serde(default, rename = "facetFilters")]
    pub facet_filters: Option<serde_json::Value>,
    #[serde(default, rename = "numericFilters")]
    pub numeric_filters: Option<serde_json::Value>,
    #[serde(default, rename = "tagFilters")]
    pub tag_filters: Option<serde_json::Value>,
    #[serde(default, rename = "maxValuesPerFacet")]
    pub max_values_per_facet: Option<usize>,
    #[serde(default)]
    pub analytics: Option<bool>,
    #[serde(default, rename = "clickAnalytics")]
    pub click_analytics: Option<bool>,
    #[serde(default, rename = "analyticsTags")]
    pub analytics_tags: Option<Vec<String>>,
    /// URL-encoded params string (used by multi-query). Merged during deserialization.
    #[serde(default)]
    pub params: Option<String>,
    #[serde(default, rename = "type")]
    pub query_type: Option<String>,
    /// Facet name for type=facet multi-search queries
    #[serde(default)]
    pub facet: Option<String>,
    /// Facet query string for type=facet multi-search queries
    #[serde(default, rename = "facetQuery")]
    pub facet_query: Option<String>,
    /// Max facet hits for type=facet multi-search queries
    #[serde(default, rename = "maxFacetHits")]
    pub max_facet_hits: Option<usize>,
    #[serde(default, rename = "getRankingInfo")]
    pub get_ranking_info: Option<bool>,
    #[serde(default, rename = "responseFields")]
    pub response_fields: Option<Vec<String>>,
    #[serde(default, rename = "aroundLatLng")]
    pub around_lat_lng: Option<String>,
    #[serde(default, rename = "aroundRadius")]
    pub around_radius: Option<serde_json::Value>,
    #[serde(default, rename = "insideBoundingBox")]
    pub inside_bounding_box: Option<serde_json::Value>,
    #[serde(default, rename = "insidePolygon")]
    pub inside_polygon: Option<serde_json::Value>,
    #[serde(default, rename = "aroundPrecision")]
    pub around_precision: Option<serde_json::Value>,
    #[serde(default, rename = "minimumAroundRadius")]
    pub minimum_around_radius: Option<u64>,
    #[serde(default, rename = "userToken")]
    pub user_token: Option<String>,
    /// Client IP — not deserialized from JSON, set by handler from headers
    #[serde(skip)]
    pub user_ip: Option<String>,
    #[serde(default, rename = "aroundLatLngViaIP")]
    pub around_lat_lng_via_ip: Option<bool>,
    #[serde(default, rename = "removeStopWords")]
    pub remove_stop_words: Option<flapjack::query::stopwords::RemoveStopWordsValue>,
    #[serde(default, rename = "ignorePlurals")]
    pub ignore_plurals: Option<flapjack::query::plurals::IgnorePluralsValue>,
    #[serde(default, rename = "queryLanguages")]
    pub query_languages: Option<Vec<String>>,
    #[serde(default)]
    pub mode: Option<flapjack::index::settings::IndexMode>,
    #[serde(default)]
    pub hybrid: Option<HybridSearchParams>,
}

impl SearchRequest {
    pub fn effective_hits_per_page(&self) -> usize {
        self.hits_per_page.unwrap_or(20)
    }

    /// Clamp hybrid search ratio to [0.0, 1.0] if present.
    pub fn clamp_hybrid_ratio(&mut self) {
        if let Some(ref mut h) = self.hybrid {
            h.clamp_ratio();
        }
    }

    pub fn apply_params_string(&mut self) {
        let params_str = match self.params.take() {
            Some(s) if !s.is_empty() => s,
            _ => return,
        };
        for (key, value) in url::form_urlencoded::parse(params_str.as_bytes()) {
            match key.as_ref() {
                "query" => {
                    if self.query.is_empty() {
                        self.query = value.into_owned();
                    }
                }
                "filters" => {
                    if self.filters.is_none() {
                        self.filters = Some(value.into_owned());
                    }
                }
                "hitsPerPage" => {
                    if self.hits_per_page.is_none() {
                        self.hits_per_page = value.parse().ok();
                    }
                }
                "page" => {
                    self.page = value.parse().unwrap_or(0);
                }
                "facets" => {
                    if self.facets.is_none() {
                        if let Ok(v) = serde_json::from_str::<Vec<String>>(&value) {
                            self.facets = Some(v);
                        } else {
                            self.facets =
                                Some(value.split(',').map(|s| s.trim().to_string()).collect());
                        }
                    }
                }
                "facetFilters" => {
                    if self.facet_filters.is_none() {
                        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&value) {
                            self.facet_filters = Some(v);
                        }
                    }
                }
                "numericFilters" => {
                    if self.numeric_filters.is_none() {
                        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&value) {
                            self.numeric_filters = Some(v);
                        }
                    }
                }
                "tagFilters" => {
                    if self.tag_filters.is_none() {
                        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&value) {
                            self.tag_filters = Some(v);
                        }
                    }
                }
                "maxValuesPerFacet" => {
                    if self.max_values_per_facet.is_none() {
                        self.max_values_per_facet = value.parse().ok();
                    }
                }
                "attributesToRetrieve" => {
                    if self.attributes_to_retrieve.is_none() {
                        if let Ok(v) = serde_json::from_str::<Vec<String>>(&value) {
                            self.attributes_to_retrieve = Some(v);
                        }
                    }
                }
                "attributesToHighlight" => {
                    if self.attributes_to_highlight.is_none() {
                        if let Ok(v) = serde_json::from_str::<Vec<String>>(&value) {
                            self.attributes_to_highlight = Some(v);
                        }
                    }
                }
                "attributesToSnippet" => {
                    if self.attributes_to_snippet.is_none() {
                        if let Ok(v) = serde_json::from_str::<Vec<String>>(&value) {
                            self.attributes_to_snippet = Some(v);
                        }
                    }
                }
                "queryType" => {
                    if self.query_type_prefix.is_none() {
                        self.query_type_prefix = Some(value.into_owned());
                    }
                }
                "typoTolerance" => {
                    if self.typo_tolerance.is_none() {
                        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&value) {
                            self.typo_tolerance = Some(v);
                        } else {
                            // Handle bare "true"/"false" strings
                            match value.as_ref() {
                                "true" => self.typo_tolerance = Some(serde_json::Value::Bool(true)),
                                "false" => {
                                    self.typo_tolerance = Some(serde_json::Value::Bool(false))
                                }
                                _ => {
                                    self.typo_tolerance =
                                        Some(serde_json::Value::String(value.into_owned()))
                                }
                            }
                        }
                    }
                }
                "advancedSyntax" => {
                    if self.advanced_syntax.is_none() {
                        self.advanced_syntax = value.parse().ok();
                    }
                }
                "removeWordsIfNoResults" => {
                    if self.remove_words_if_no_results.is_none() {
                        self.remove_words_if_no_results = Some(value.into_owned());
                    }
                }
                "optionalFilters" => {
                    if self.optional_filters.is_none() {
                        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&value) {
                            self.optional_filters = Some(v);
                        }
                    }
                }
                "enableSynonyms" => {
                    if self.enable_synonyms.is_none() {
                        self.enable_synonyms = value.parse().ok();
                    }
                }
                "enableRules" => {
                    if self.enable_rules.is_none() {
                        self.enable_rules = value.parse().ok();
                    }
                }
                "ruleContexts" => {
                    if self.rule_contexts.is_none() {
                        if let Ok(v) = serde_json::from_str::<Vec<String>>(&value) {
                            self.rule_contexts = Some(v);
                        }
                    }
                }
                "restrictSearchableAttributes" => {
                    if self.restrict_searchable_attributes.is_none() {
                        if let Ok(v) = serde_json::from_str::<Vec<String>>(&value) {
                            self.restrict_searchable_attributes = Some(v);
                        }
                    }
                }
                "highlightPreTag" => {
                    if self.highlight_pre_tag.is_none() {
                        self.highlight_pre_tag = Some(value.into_owned());
                    }
                }
                "highlightPostTag" => {
                    if self.highlight_post_tag.is_none() {
                        self.highlight_post_tag = Some(value.into_owned());
                    }
                }
                "analytics" => {
                    self.analytics = value.parse().ok();
                }
                "clickAnalytics" => {
                    self.click_analytics = value.parse().ok();
                }
                "facetQuery" => {
                    if self.facet_query.is_none() {
                        self.facet_query = Some(value.into_owned());
                    }
                }
                "maxFacetHits" => {
                    if self.max_facet_hits.is_none() {
                        self.max_facet_hits = value.parse().ok();
                    }
                }
                "analyticsTags" => {
                    if self.analytics_tags.is_none() {
                        if let Ok(v) = serde_json::from_str::<Vec<String>>(&value) {
                            self.analytics_tags = Some(v);
                        }
                    }
                }
                "distinct" => {
                    if self.distinct.is_none() {
                        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&value) {
                            self.distinct = Some(v);
                        }
                    }
                }
                "getRankingInfo" => {
                    self.get_ranking_info = value.parse().ok();
                }
                "responseFields" => {
                    if self.response_fields.is_none() {
                        if let Ok(v) = serde_json::from_str::<Vec<String>>(&value) {
                            self.response_fields = Some(v);
                        }
                    }
                }
                "aroundLatLng" => {
                    if self.around_lat_lng.is_none() {
                        self.around_lat_lng = Some(value.into_owned());
                    }
                }
                "aroundRadius" => {
                    if self.around_radius.is_none() {
                        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&value) {
                            self.around_radius = Some(v);
                        } else if value == "all" {
                            self.around_radius = Some(serde_json::Value::String("all".to_string()));
                        } else if let Ok(n) = value.parse::<u64>() {
                            self.around_radius = Some(serde_json::json!(n));
                        }
                    }
                }
                "insideBoundingBox" => {
                    if self.inside_bounding_box.is_none() {
                        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&value) {
                            self.inside_bounding_box = Some(v);
                        }
                    }
                }
                "insidePolygon" => {
                    if self.inside_polygon.is_none() {
                        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&value) {
                            self.inside_polygon = Some(v);
                        }
                    }
                }
                "aroundPrecision" => {
                    if self.around_precision.is_none() {
                        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&value) {
                            self.around_precision = Some(v);
                        } else if let Ok(n) = value.parse::<u64>() {
                            self.around_precision = Some(serde_json::json!(n));
                        }
                    }
                }
                "minimumAroundRadius" => {
                    if self.minimum_around_radius.is_none() {
                        self.minimum_around_radius = value.parse().ok();
                    }
                }
                "userToken" => {
                    if self.user_token.is_none() {
                        self.user_token = Some(value.into_owned());
                    }
                }
                "aroundLatLngViaIP" => {
                    self.around_lat_lng_via_ip = value.parse().ok();
                }
                "removeStopWords" => {
                    if self.remove_stop_words.is_none() {
                        if let Ok(v) = serde_json::from_str::<
                            flapjack::query::stopwords::RemoveStopWordsValue,
                        >(&value)
                        {
                            self.remove_stop_words = Some(v);
                        }
                    }
                }
                "ignorePlurals" => {
                    if self.ignore_plurals.is_none() {
                        if let Ok(v) = serde_json::from_str::<
                            flapjack::query::plurals::IgnorePluralsValue,
                        >(&value)
                        {
                            self.ignore_plurals = Some(v);
                        }
                    }
                }
                "queryLanguages" => {
                    if self.query_languages.is_none() {
                        if let Ok(v) = serde_json::from_str::<Vec<String>>(&value) {
                            self.query_languages = Some(v);
                        }
                    }
                }
                "mode" => {
                    if self.mode.is_none() {
                        self.mode = match value.as_ref() {
                            "neuralSearch" => {
                                Some(flapjack::index::settings::IndexMode::NeuralSearch)
                            }
                            "keywordSearch" => {
                                Some(flapjack::index::settings::IndexMode::KeywordSearch)
                            }
                            _ => None,
                        };
                    }
                }
                "hybrid" => {
                    if self.hybrid.is_none() {
                        if let Ok(mut h) = serde_json::from_str::<HybridSearchParams>(&value) {
                            h.clamp_ratio();
                            self.hybrid = Some(h);
                        }
                    }
                }
                _ => {}
            }
        }
    }

    pub fn build_geo_params(&self) -> flapjack::query::geo::GeoParams {
        use flapjack::query::geo::*;

        let has_bbox = self.inside_bounding_box.is_some();
        let has_poly = self.inside_polygon.is_some();

        let bounding_boxes = self
            .inside_bounding_box
            .as_ref()
            .map(parse_bounding_boxes)
            .unwrap_or_default();

        let polygons = self
            .inside_polygon
            .as_ref()
            .map(parse_polygons)
            .unwrap_or_default();

        let around = if has_bbox || has_poly {
            None
        } else if let Some(point) = self
            .around_lat_lng
            .as_ref()
            .and_then(|s| parse_around_lat_lng(s))
        {
            Some(point)
        } else if self.around_lat_lng_via_ip == Some(true) {
            tracing::warn!("[GEO] aroundLatLngViaIP=true but no GeoIP database configured (set FLAPJACK_GEOIP_DB). Ignoring.");
            None
        } else {
            None
        };

        let around_radius = if around.is_some() {
            self.around_radius.as_ref().and_then(parse_around_radius)
        } else {
            None
        };

        let around_precision = self
            .around_precision
            .as_ref()
            .map(parse_around_precision)
            .unwrap_or_default();

        let minimum_around_radius = if around.is_some() && around_radius.is_none() {
            self.minimum_around_radius
        } else {
            None
        };

        GeoParams {
            around,
            around_radius,
            bounding_boxes,
            polygons,
            around_precision,
            minimum_around_radius,
        }
    }

    pub fn build_combined_filter(&self) -> Option<flapjack::types::Filter> {
        use flapjack::types::Filter;
        let mut parts: Vec<Filter> = Vec::new();

        if let Some(ref filter_str) = self.filters {
            if let Ok(f) = crate::filter_parser::parse_filter(filter_str) {
                parts.push(f);
            }
        }

        if let Some(ref ff) = self.facet_filters {
            if let Some(f) = facet_filters_to_ast(ff) {
                parts.push(f);
            }
        }

        if let Some(ref nf) = self.numeric_filters {
            if let Some(f) = numeric_filters_to_ast(nf) {
                parts.push(f);
            }
        }

        if let Some(ref tf) = self.tag_filters {
            if let Some(f) = tag_filters_to_ast(tf) {
                parts.push(f);
            }
        }

        match parts.len() {
            0 => None,
            1 => Some(parts.remove(0)),
            _ => Some(Filter::And(parts)),
        }
    }
}

fn parse_facet_filter_string(s: &str) -> Option<flapjack::types::Filter> {
    use flapjack::types::{FieldValue, Filter};
    let s = s.trim();
    let (negated, s) = if let Some(rest) = s.strip_prefix('-') {
        (true, rest)
    } else {
        (false, s)
    };
    let colon_pos = s.find(':')?;
    let field = s[..colon_pos].to_string();
    let value = s[colon_pos + 1..]
        .trim_matches('"')
        .trim_matches('\'')
        .to_string();
    let filter = Filter::Equals {
        field,
        value: FieldValue::Text(value),
    };
    if negated {
        Some(Filter::Not(Box::new(filter)))
    } else {
        Some(filter)
    }
}

fn facet_filters_to_ast(value: &serde_json::Value) -> Option<flapjack::types::Filter> {
    use flapjack::types::Filter;
    match value {
        serde_json::Value::Array(items) => {
            let mut and_parts: Vec<Filter> = Vec::new();
            for item in items {
                match item {
                    serde_json::Value::Array(or_items) => {
                        let or_filters: Vec<Filter> = or_items
                            .iter()
                            .filter_map(|v| v.as_str().and_then(parse_facet_filter_string))
                            .collect();
                        match or_filters.len() {
                            0 => {}
                            1 => and_parts.push(or_filters.into_iter().next().unwrap()),
                            _ => and_parts.push(Filter::Or(or_filters)),
                        }
                    }
                    serde_json::Value::String(s) => {
                        if let Some(f) = parse_facet_filter_string(s) {
                            and_parts.push(f);
                        }
                    }
                    _ => {}
                }
            }
            match and_parts.len() {
                0 => None,
                1 => Some(and_parts.remove(0)),
                _ => Some(Filter::And(and_parts)),
            }
        }
        serde_json::Value::String(s) => parse_facet_filter_string(s),
        _ => None,
    }
}

fn parse_numeric_filter_string(s: &str) -> Option<flapjack::types::Filter> {
    use flapjack::types::{FieldValue, Filter};
    let s = s.trim();
    let ops = [">=", "<=", "!=", ">", "<", "="];
    for op in &ops {
        if let Some(pos) = s.find(op) {
            let field = s[..pos].trim().to_string();
            let val_str = s[pos + op.len()..].trim();
            let value = if let Ok(i) = val_str.parse::<i64>() {
                FieldValue::Integer(i)
            } else if let Ok(f) = val_str.parse::<f64>() {
                FieldValue::Float(f)
            } else {
                return None;
            };
            return Some(match *op {
                ">=" => Filter::GreaterThanOrEqual { field, value },
                "<=" => Filter::LessThanOrEqual { field, value },
                ">" => Filter::GreaterThan { field, value },
                "<" => Filter::LessThan { field, value },
                "!=" => Filter::NotEquals { field, value },
                "=" => Filter::Equals { field, value },
                _ => return None,
            });
        }
    }
    None
}

fn numeric_filters_to_ast(value: &serde_json::Value) -> Option<flapjack::types::Filter> {
    use flapjack::types::Filter;
    match value {
        serde_json::Value::Array(items) => {
            let mut and_parts: Vec<Filter> = Vec::new();
            for item in items {
                match item {
                    serde_json::Value::Array(or_items) => {
                        let or_filters: Vec<Filter> = or_items
                            .iter()
                            .filter_map(|v| v.as_str().and_then(parse_numeric_filter_string))
                            .collect();
                        match or_filters.len() {
                            0 => {}
                            1 => and_parts.push(or_filters.into_iter().next().unwrap()),
                            _ => and_parts.push(Filter::Or(or_filters)),
                        }
                    }
                    serde_json::Value::String(s) => {
                        if let Some(f) = parse_numeric_filter_string(s) {
                            and_parts.push(f);
                        }
                    }
                    _ => {}
                }
            }
            match and_parts.len() {
                0 => None,
                1 => Some(and_parts.remove(0)),
                _ => Some(Filter::And(and_parts)),
            }
        }
        serde_json::Value::String(s) => parse_numeric_filter_string(s),
        _ => None,
    }
}

fn tag_filters_to_ast(value: &serde_json::Value) -> Option<flapjack::types::Filter> {
    use flapjack::types::{FieldValue, Filter};
    match value {
        serde_json::Value::Array(items) => {
            let mut and_parts: Vec<Filter> = Vec::new();
            for item in items {
                match item {
                    serde_json::Value::Array(or_items) => {
                        let or_filters: Vec<Filter> = or_items
                            .iter()
                            .filter_map(|v| {
                                v.as_str().map(|s| Filter::Equals {
                                    field: "_tags".to_string(),
                                    value: FieldValue::Text(s.to_string()),
                                })
                            })
                            .collect();
                        match or_filters.len() {
                            0 => {}
                            1 => and_parts.push(or_filters.into_iter().next().unwrap()),
                            _ => and_parts.push(Filter::Or(or_filters)),
                        }
                    }
                    serde_json::Value::String(s) => {
                        and_parts.push(Filter::Equals {
                            field: "_tags".to_string(),
                            value: FieldValue::Text(s.to_string()),
                        });
                    }
                    _ => {}
                }
            }
            match and_parts.len() {
                0 => None,
                1 => Some(and_parts.remove(0)),
                _ => Some(Filter::And(and_parts)),
            }
        }
        serde_json::Value::String(s) => Some(Filter::Equals {
            field: "_tags".to_string(),
            value: FieldValue::Text(s.to_string()),
        }),
        _ => None,
    }
}

/// Parse Algolia `optionalFilters` JSON into `(field, value, score)` tuples.
///
/// Accepts:
///   - `"category:Book"` — single string
///   - `["category:Book", "author:John"]` — flat array (independent boosts)
///   - `[["category:Book", "category:Movie"], "author:John"]` — nested OR groups
///   - `"category:Book<score=2>"` — custom score weight
pub fn parse_optional_filters(value: &serde_json::Value) -> Vec<(String, String, f32)> {
    fn parse_one(s: &str) -> Option<(String, String, f32)> {
        let s = s.trim();
        // Strip optional <score=N> suffix
        let (s, score) = if let Some(idx) = s.find("<score=") {
            let rest = &s[idx + 7..];
            let end = rest.find('>').unwrap_or(rest.len());
            let sc: f32 = rest[..end].parse().unwrap_or(1.0);
            (&s[..idx], sc)
        } else {
            (s, 1.0)
        };
        // Strip leading '-' for negation (we treat as score=0 penalty — skip)
        let s = s.strip_prefix('-').unwrap_or(s);
        let colon = s.find(':')?;
        let field = s[..colon].to_string();
        let value = s[colon + 1..]
            .trim_matches('"')
            .trim_matches('\'')
            .to_string();
        Some((field, value, score))
    }

    let mut specs = Vec::new();
    match value {
        serde_json::Value::String(s) => {
            if let Some(spec) = parse_one(s) {
                specs.push(spec);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                match item {
                    serde_json::Value::String(s) => {
                        if let Some(spec) = parse_one(s) {
                            specs.push(spec);
                        }
                    }
                    serde_json::Value::Array(or_items) => {
                        for sub in or_items {
                            if let Some(s) = sub.as_str() {
                                if let Some(spec) = parse_one(s) {
                                    specs.push(spec);
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        _ => {}
    }
    specs
}

#[allow(dead_code)]
fn default_hits_per_page() -> usize {
    20
}

#[allow(dead_code)]
fn deserialize_option_hits_per_page<'de, D>(deserializer: D) -> Result<Option<usize>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[allow(unused_imports)]
    use serde::de::Error;
    #[derive(Deserialize)]
    struct Wrapper(#[serde(deserialize_with = "deserialize_null_default")] Option<usize>);

    fn deserialize_null_default<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
    where
        D: serde::Deserializer<'de>,
        T: serde::Deserialize<'de>,
    {
        Ok(Option::<T>::deserialize(deserializer).ok().flatten())
    }

    Wrapper::deserialize(deserializer).map(|w| w.0)
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SearchHit {
    #[serde(flatten)]
    pub document: HashMap<String, serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub _score: Option<f32>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct GetObjectsRequest {
    pub requests: Vec<GetObjectRequest>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct GetObjectRequest {
    pub index_name: String,
    #[serde(rename = "objectID")]
    pub object_id: String,
    #[serde(default)]
    pub attributes_to_retrieve: Option<Vec<String>>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct GetObjectsResponse {
    pub results: Vec<serde_json::Value>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DeleteByQueryRequest {
    #[serde(default)]
    pub filters: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SearchFacetValuesRequest {
    #[serde(rename = "facetQuery")]
    pub facet_query: String,

    #[serde(default)]
    pub filters: Option<String>,

    #[serde(default = "default_max_facet_hits")]
    #[serde(rename = "maxFacetHits")]
    pub max_facet_hits: usize,
}

fn default_max_facet_hits() -> usize {
    10
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── effective_hits_per_page ──

    #[test]
    fn effective_hits_per_page_default() {
        let req = SearchRequest::default();
        assert_eq!(req.effective_hits_per_page(), 20);
    }

    #[test]
    fn effective_hits_per_page_custom() {
        let req = SearchRequest {
            hits_per_page: Some(50),
            ..Default::default()
        };
        assert_eq!(req.effective_hits_per_page(), 50);
    }

    // ── apply_params_string ──

    #[test]
    fn apply_params_string_sets_query() {
        let mut req = SearchRequest::default();
        req.params = Some("query=hello".to_string());
        req.apply_params_string();
        assert_eq!(req.query, "hello");
    }

    #[test]
    fn apply_params_string_does_not_override_existing_query() {
        let mut req = SearchRequest {
            query: "existing".to_string(),
            params: Some("query=new".to_string()),
            ..Default::default()
        };
        req.apply_params_string();
        assert_eq!(req.query, "existing");
    }

    #[test]
    fn apply_params_string_sets_hits_per_page() {
        let mut req = SearchRequest::default();
        req.params = Some("hitsPerPage=5".to_string());
        req.apply_params_string();
        assert_eq!(req.hits_per_page, Some(5));
    }

    #[test]
    fn apply_params_string_sets_page() {
        let mut req = SearchRequest::default();
        req.params = Some("page=3".to_string());
        req.apply_params_string();
        assert_eq!(req.page, 3);
    }

    #[test]
    fn apply_params_string_sets_filters() {
        let mut req = SearchRequest::default();
        req.params = Some("filters=brand%3ANike".to_string());
        req.apply_params_string();
        assert_eq!(req.filters, Some("brand:Nike".to_string()));
    }

    #[test]
    fn apply_params_string_empty_noop() {
        let mut req = SearchRequest::default();
        req.params = Some("".to_string());
        req.apply_params_string();
        assert!(req.query.is_empty());
    }

    #[test]
    fn apply_params_string_none_noop() {
        let mut req = SearchRequest::default();
        req.apply_params_string();
        assert!(req.query.is_empty());
    }

    #[test]
    fn apply_params_string_multiple_fields() {
        let mut req = SearchRequest::default();
        req.params = Some("query=test&hitsPerPage=10&page=2&analytics=true".to_string());
        req.apply_params_string();
        assert_eq!(req.query, "test");
        assert_eq!(req.hits_per_page, Some(10));
        assert_eq!(req.page, 2);
        assert_eq!(req.analytics, Some(true));
    }

    // ── parse_facet_filter_string ──

    #[test]
    fn parse_facet_filter_basic() {
        let f = parse_facet_filter_string("brand:Nike").unwrap();
        match f {
            flapjack::types::Filter::Equals { field, value } => {
                assert_eq!(field, "brand");
                assert_eq!(value, flapjack::types::FieldValue::Text("Nike".to_string()));
            }
            _ => panic!("expected Equals"),
        }
    }

    #[test]
    fn parse_facet_filter_negated() {
        let f = parse_facet_filter_string("-brand:Nike").unwrap();
        match f {
            flapjack::types::Filter::Not(inner) => match *inner {
                flapjack::types::Filter::Equals { field, value } => {
                    assert_eq!(field, "brand");
                    assert_eq!(value, flapjack::types::FieldValue::Text("Nike".to_string()));
                }
                _ => panic!("expected Equals inside Not"),
            },
            _ => panic!("expected Not"),
        }
    }

    #[test]
    fn parse_facet_filter_quoted_value() {
        let f = parse_facet_filter_string("brand:\"Air Max\"").unwrap();
        match f {
            flapjack::types::Filter::Equals { value, .. } => {
                assert_eq!(
                    value,
                    flapjack::types::FieldValue::Text("Air Max".to_string())
                );
            }
            _ => panic!("expected Equals"),
        }
    }

    #[test]
    fn parse_facet_filter_no_colon() {
        assert!(parse_facet_filter_string("nocolon").is_none());
    }

    // ── parse_numeric_filter_string ──

    #[test]
    fn parse_numeric_equals() {
        let f = parse_numeric_filter_string("price=100").unwrap();
        match f {
            flapjack::types::Filter::Equals { field, value } => {
                assert_eq!(field, "price");
                assert_eq!(value, flapjack::types::FieldValue::Integer(100));
            }
            _ => panic!("expected Equals"),
        }
    }

    #[test]
    fn parse_numeric_gte() {
        let f = parse_numeric_filter_string("price>=50").unwrap();
        match f {
            flapjack::types::Filter::GreaterThanOrEqual { field, value } => {
                assert_eq!(field, "price");
                assert_eq!(value, flapjack::types::FieldValue::Integer(50));
            }
            _ => panic!("expected GreaterThanOrEqual"),
        }
    }

    #[test]
    fn parse_numeric_lt() {
        let f = parse_numeric_filter_string("price<200").unwrap();
        match f {
            flapjack::types::Filter::LessThan { field, value } => {
                assert_eq!(field, "price");
                assert_eq!(value, flapjack::types::FieldValue::Integer(200));
            }
            _ => panic!("expected LessThan"),
        }
    }

    #[test]
    fn parse_numeric_float() {
        let f = parse_numeric_filter_string("rating>=4.5").unwrap();
        match f {
            flapjack::types::Filter::GreaterThanOrEqual { field, value } => {
                assert_eq!(field, "rating");
                assert_eq!(value, flapjack::types::FieldValue::Float(4.5));
            }
            _ => panic!("expected GreaterThanOrEqual"),
        }
    }

    #[test]
    fn parse_numeric_not_equals() {
        let f = parse_numeric_filter_string("status!=0").unwrap();
        match f {
            flapjack::types::Filter::NotEquals { field, value } => {
                assert_eq!(field, "status");
                assert_eq!(value, flapjack::types::FieldValue::Integer(0));
            }
            _ => panic!("expected NotEquals"),
        }
    }

    #[test]
    fn parse_numeric_invalid_value() {
        assert!(parse_numeric_filter_string("price=abc").is_none());
    }

    // ── facet_filters_to_ast ──

    #[test]
    fn facet_filters_single_string() {
        let v = serde_json::json!("brand:Nike");
        let f = facet_filters_to_ast(&v).unwrap();
        match f {
            flapjack::types::Filter::Equals { field, value } => {
                assert_eq!(field, "brand");
                assert_eq!(value, flapjack::types::FieldValue::Text("Nike".to_string()));
            }
            _ => panic!("expected Equals"),
        }
    }

    #[test]
    fn facet_filters_array_and() {
        let v = serde_json::json!(["brand:Nike", "color:Red"]);
        let f = facet_filters_to_ast(&v).unwrap();
        match f {
            flapjack::types::Filter::And(parts) => {
                assert_eq!(parts.len(), 2);
                // Verify both filters parsed correctly
                match &parts[0] {
                    flapjack::types::Filter::Equals { field, value } => {
                        assert_eq!(field, "brand");
                        assert_eq!(
                            *value,
                            flapjack::types::FieldValue::Text("Nike".to_string())
                        );
                    }
                    _ => panic!("expected Equals for first filter"),
                }
            }
            _ => panic!("expected And"),
        }
    }

    #[test]
    fn facet_filters_nested_or() {
        let v = serde_json::json!([["brand:Nike", "brand:Adidas"], "color:Red"]);
        let f = facet_filters_to_ast(&v).unwrap();
        match f {
            flapjack::types::Filter::And(parts) => {
                assert_eq!(parts.len(), 2);
                match &parts[0] {
                    flapjack::types::Filter::Or(or_parts) => assert_eq!(or_parts.len(), 2),
                    _ => panic!("expected Or"),
                }
            }
            _ => panic!("expected And"),
        }
    }

    #[test]
    fn facet_filters_empty_array() {
        let v = serde_json::json!([]);
        assert!(facet_filters_to_ast(&v).is_none());
    }

    // ── numeric_filters_to_ast ──

    #[test]
    fn numeric_filters_single_string() {
        let v = serde_json::json!("price>=10");
        let f = numeric_filters_to_ast(&v).unwrap();
        match f {
            flapjack::types::Filter::GreaterThanOrEqual { field, value } => {
                assert_eq!(field, "price");
                assert_eq!(value, flapjack::types::FieldValue::Integer(10));
            }
            _ => panic!("expected GreaterThanOrEqual"),
        }
    }

    #[test]
    fn numeric_filters_array_and() {
        let v = serde_json::json!(["price>=10", "price<=100"]);
        let f = numeric_filters_to_ast(&v).unwrap();
        match f {
            flapjack::types::Filter::And(parts) => {
                assert_eq!(parts.len(), 2);
                match &parts[0] {
                    flapjack::types::Filter::GreaterThanOrEqual { field, value } => {
                        assert_eq!(field, "price");
                        assert_eq!(*value, flapjack::types::FieldValue::Integer(10));
                    }
                    _ => panic!("expected GreaterThanOrEqual"),
                }
                match &parts[1] {
                    flapjack::types::Filter::LessThanOrEqual { field, value } => {
                        assert_eq!(field, "price");
                        assert_eq!(*value, flapjack::types::FieldValue::Integer(100));
                    }
                    _ => panic!("expected LessThanOrEqual"),
                }
            }
            _ => panic!("expected And"),
        }
    }

    // ── tag_filters_to_ast ──

    #[test]
    fn tag_filters_single_string() {
        let v = serde_json::json!("electronics");
        let f = tag_filters_to_ast(&v).unwrap();
        match f {
            flapjack::types::Filter::Equals { field, value } => {
                assert_eq!(field, "_tags");
                assert_eq!(
                    value,
                    flapjack::types::FieldValue::Text("electronics".to_string())
                );
            }
            _ => panic!("expected Equals"),
        }
    }

    #[test]
    fn tag_filters_array_and() {
        let v = serde_json::json!(["electronics", "sale"]);
        let f = tag_filters_to_ast(&v).unwrap();
        match f {
            flapjack::types::Filter::And(parts) => assert_eq!(parts.len(), 2),
            _ => panic!("expected And"),
        }
    }

    #[test]
    fn tag_filters_nested_or() {
        let v = serde_json::json!([["electronics", "books"], "sale"]);
        let f = tag_filters_to_ast(&v).unwrap();
        match f {
            flapjack::types::Filter::And(parts) => {
                assert_eq!(parts.len(), 2);
                match &parts[0] {
                    flapjack::types::Filter::Or(or_parts) => assert_eq!(or_parts.len(), 2),
                    _ => panic!("expected Or"),
                }
            }
            _ => panic!("expected And"),
        }
    }

    // ── parse_optional_filters ──

    #[test]
    fn optional_filters_single_string() {
        let v = serde_json::json!("category:Book");
        let specs = parse_optional_filters(&v);
        assert_eq!(specs.len(), 1);
        assert_eq!(specs[0].0, "category");
        assert_eq!(specs[0].1, "Book");
        assert_eq!(specs[0].2, 1.0);
    }

    #[test]
    fn optional_filters_with_score() {
        let v = serde_json::json!("category:Book<score=2>");
        let specs = parse_optional_filters(&v);
        assert_eq!(specs[0].2, 2.0);
    }

    #[test]
    fn optional_filters_flat_array() {
        let v = serde_json::json!(["category:Book", "author:John"]);
        let specs = parse_optional_filters(&v);
        assert_eq!(specs.len(), 2);
    }

    #[test]
    fn optional_filters_nested_or() {
        let v = serde_json::json!([["category:Book", "category:Movie"], "author:John"]);
        let specs = parse_optional_filters(&v);
        assert_eq!(specs.len(), 3);
    }

    #[test]
    fn optional_filters_negated() {
        let v = serde_json::json!("-category:Book");
        let specs = parse_optional_filters(&v);
        assert_eq!(specs.len(), 1);
        assert_eq!(specs[0].0, "category");
    }

    #[test]
    fn optional_filters_empty_value() {
        let v = serde_json::json!(null);
        let specs = parse_optional_filters(&v);
        assert!(specs.is_empty());
    }

    // ── deserialize_string_or_vec ──

    #[test]
    fn search_request_facets_string() {
        let json = r#"{"facets": "brand"}"#;
        let req: SearchRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.facets, Some(vec!["brand".to_string()]));
    }

    #[test]
    fn search_request_facets_array() {
        let json = r#"{"facets": ["brand", "category"]}"#;
        let req: SearchRequest = serde_json::from_str(json).unwrap();
        assert_eq!(
            req.facets,
            Some(vec!["brand".to_string(), "category".to_string()])
        );
    }

    #[test]
    fn search_request_facets_null() {
        let json = r#"{"facets": null}"#;
        let req: SearchRequest = serde_json::from_str(json).unwrap();
        assert!(req.facets.is_none());
    }

    #[test]
    fn search_request_facets_missing() {
        let json = r#"{}"#;
        let req: SearchRequest = serde_json::from_str(json).unwrap();
        assert!(req.facets.is_none());
    }

    // ── build_combined_filter ──

    #[test]
    fn build_combined_filter_none_when_empty() {
        let req = SearchRequest::default();
        assert!(req.build_combined_filter().is_none());
    }

    #[test]
    fn build_combined_filter_filters_only() {
        let req = SearchRequest {
            filters: Some("brand:Nike".to_string()),
            ..Default::default()
        };
        let f = req.build_combined_filter().unwrap();
        // Should be a single filter, not wrapped in And
        match f {
            flapjack::types::Filter::Equals { field, .. } => assert_eq!(field, "brand"),
            _ => panic!("expected Equals from filter string, got {:?}", f),
        }
    }

    #[test]
    fn build_combined_filter_facet_filters_only() {
        let req = SearchRequest {
            facet_filters: Some(serde_json::json!("color:Red")),
            ..Default::default()
        };
        let f = req.build_combined_filter().unwrap();
        match f {
            flapjack::types::Filter::Equals { field, .. } => assert_eq!(field, "color"),
            _ => panic!("expected Equals from facet filter"),
        }
    }

    #[test]
    fn build_combined_filter_combines_multiple_with_and() {
        let req = SearchRequest {
            filters: Some("brand:Nike".to_string()),
            facet_filters: Some(serde_json::json!("color:Red")),
            ..Default::default()
        };
        let f = req.build_combined_filter().unwrap();
        match f {
            flapjack::types::Filter::And(parts) => assert_eq!(parts.len(), 2),
            _ => panic!("expected And when combining filters + facet_filters"),
        }
    }

    #[test]
    fn build_combined_filter_all_three_types() {
        let req = SearchRequest {
            filters: Some("brand:Nike".to_string()),
            facet_filters: Some(serde_json::json!("color:Red")),
            numeric_filters: Some(serde_json::json!("price>=10")),
            ..Default::default()
        };
        let f = req.build_combined_filter().unwrap();
        match f {
            flapjack::types::Filter::And(parts) => assert_eq!(parts.len(), 3),
            _ => panic!("expected And with 3 parts"),
        }
    }

    #[test]
    fn build_combined_filter_with_tag_filters() {
        let req = SearchRequest {
            tag_filters: Some(serde_json::json!("electronics")),
            ..Default::default()
        };
        let f = req.build_combined_filter().unwrap();
        match f {
            flapjack::types::Filter::Equals { field, .. } => assert_eq!(field, "_tags"),
            _ => panic!("expected Equals from tag filter"),
        }
    }

    #[test]
    fn build_combined_filter_invalid_filter_string_skipped() {
        let req = SearchRequest {
            filters: Some(":::invalid:::".to_string()),
            facet_filters: Some(serde_json::json!("color:Red")),
            ..Default::default()
        };
        // Invalid filters string should be skipped, facet filter should still work
        let f = req.build_combined_filter();
        assert!(f.is_some());
    }

    // ── parse_numeric_filter_string edge cases ──

    #[test]
    fn parse_numeric_negative_value() {
        let f = parse_numeric_filter_string("temp=-10").unwrap();
        match f {
            flapjack::types::Filter::Equals { field, value } => {
                assert_eq!(field, "temp");
                assert_eq!(value, flapjack::types::FieldValue::Integer(-10));
            }
            _ => panic!("expected Equals"),
        }
    }

    #[test]
    fn parse_numeric_negative_float() {
        let f = parse_numeric_filter_string("rating>=-1.5").unwrap();
        match f {
            flapjack::types::Filter::GreaterThanOrEqual { field, value } => {
                assert_eq!(field, "rating");
                assert_eq!(value, flapjack::types::FieldValue::Float(-1.5));
            }
            _ => panic!("expected GreaterThanOrEqual"),
        }
    }

    #[test]
    fn parse_numeric_no_operator() {
        assert!(parse_numeric_filter_string("justanumber").is_none());
    }

    #[test]
    fn parse_numeric_gt() {
        let f = parse_numeric_filter_string("count>5").unwrap();
        match f {
            flapjack::types::Filter::GreaterThan { field, value } => {
                assert_eq!(field, "count");
                assert_eq!(value, flapjack::types::FieldValue::Integer(5));
            }
            _ => panic!("expected GreaterThan"),
        }
    }

    #[test]
    fn parse_numeric_lte() {
        let f = parse_numeric_filter_string("count<=99").unwrap();
        match f {
            flapjack::types::Filter::LessThanOrEqual { field, value } => {
                assert_eq!(field, "count");
                assert_eq!(value, flapjack::types::FieldValue::Integer(99));
            }
            _ => panic!("expected LessThanOrEqual"),
        }
    }

    // ── malformed input edge cases ──

    #[test]
    fn parse_facet_filter_empty_string() {
        assert!(parse_facet_filter_string("").is_none());
    }

    #[test]
    fn parse_facet_filter_multiple_colons() {
        // "a:b:c" — should take first colon, value is "b:c"
        let f = parse_facet_filter_string("a:b:c").unwrap();
        match f {
            flapjack::types::Filter::Equals { field, value } => {
                assert_eq!(field, "a");
                assert_eq!(value, flapjack::types::FieldValue::Text("b:c".to_string()));
            }
            _ => panic!("expected Equals"),
        }
    }

    #[test]
    fn facet_filters_non_string_in_array_skipped() {
        // Arrays with non-string values should be silently skipped
        let v = serde_json::json!([123, "brand:Nike"]);
        let f = facet_filters_to_ast(&v);
        // Should still produce a result (the valid string filter)
        assert!(f.is_some());
    }

    #[test]
    fn numeric_filters_empty_array() {
        let v = serde_json::json!([]);
        assert!(numeric_filters_to_ast(&v).is_none());
    }

    #[test]
    fn tag_filters_empty_array() {
        let v = serde_json::json!([]);
        assert!(tag_filters_to_ast(&v).is_none());
    }

    #[test]
    fn search_request_facets_empty_array() {
        let json = r#"{"facets": []}"#;
        let req: SearchRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.facets, Some(vec![]));
    }

    // ── HybridSearchParams tests (6.1) ──

    #[test]
    fn test_search_request_hybrid_from_json() {
        let json = r#"{"query": "test", "hybrid": {"semanticRatio": 0.8, "embedder": "mymodel"}}"#;
        let req: SearchRequest = serde_json::from_str(json).unwrap();
        let hybrid = req.hybrid.unwrap();
        assert!((hybrid.semantic_ratio - 0.8).abs() < f64::EPSILON);
        assert_eq!(hybrid.embedder, "mymodel");
    }

    #[test]
    fn test_search_request_hybrid_defaults() {
        let json = r#"{"query": "test", "hybrid": {}}"#;
        let req: SearchRequest = serde_json::from_str(json).unwrap();
        let hybrid = req.hybrid.unwrap();
        assert!((hybrid.semantic_ratio - 0.5).abs() < f64::EPSILON);
        assert_eq!(hybrid.embedder, "default");
    }

    #[test]
    fn test_search_request_hybrid_none_by_default() {
        let json = r#"{"query": "test"}"#;
        let req: SearchRequest = serde_json::from_str(json).unwrap();
        assert!(req.hybrid.is_none());
    }

    #[test]
    fn test_search_request_hybrid_from_params_string() {
        let mut req: SearchRequest = serde_json::from_str(
            r#"{"params": "query=test&hybrid=%7B%22semanticRatio%22%3A0.7%7D"}"#,
        )
        .unwrap();
        req.apply_params_string();
        let hybrid = req.hybrid.unwrap();
        assert!((hybrid.semantic_ratio - 0.7).abs() < f64::EPSILON);
    }

    #[test]
    fn test_search_request_hybrid_semantic_ratio_clamped() {
        // > 1.0 clamped to 1.0
        let json = r#"{"query": "test", "hybrid": {"semanticRatio": 1.5}}"#;
        let mut req: SearchRequest = serde_json::from_str(json).unwrap();
        req.clamp_hybrid_ratio();
        let hybrid = req.hybrid.unwrap();
        assert!((hybrid.semantic_ratio - 1.0).abs() < f64::EPSILON);

        // < 0.0 clamped to 0.0
        let json = r#"{"query": "test", "hybrid": {"semanticRatio": -0.5}}"#;
        let mut req: SearchRequest = serde_json::from_str(json).unwrap();
        req.clamp_hybrid_ratio();
        let hybrid = req.hybrid.unwrap();
        assert!(hybrid.semantic_ratio.abs() < f64::EPSILON);
    }

    // ── SearchRequest mode tests (5.12) ──

    #[test]
    fn test_search_request_mode_from_json() {
        use flapjack::index::settings::IndexMode;
        let json = r#"{"query": "test", "mode": "neuralSearch"}"#;
        let req: SearchRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.mode, Some(IndexMode::NeuralSearch));
    }

    #[test]
    fn test_search_request_mode_default_none() {
        let json = r#"{"query": "test"}"#;
        let req: SearchRequest = serde_json::from_str(json).unwrap();
        assert!(req.mode.is_none());
    }

    #[test]
    fn test_search_request_mode_from_params_string() {
        use flapjack::index::settings::IndexMode;
        let mut req: SearchRequest =
            serde_json::from_str(r#"{"params": "query=test&mode=neuralSearch"}"#).unwrap();
        req.apply_params_string();
        assert_eq!(req.mode, Some(IndexMode::NeuralSearch));
    }

    #[test]
    fn test_search_request_mode_keyword_from_params() {
        use flapjack::index::settings::IndexMode;
        let mut req: SearchRequest =
            serde_json::from_str(r#"{"params": "mode=keywordSearch"}"#).unwrap();
        req.apply_params_string();
        assert_eq!(req.mode, Some(IndexMode::KeywordSearch));
    }
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SearchFacetValuesResponse {
    pub facet_hits: Vec<FacetHit>,
    pub exhaustive_facets_count: bool,
    #[serde(rename = "processingTimeMS")]
    pub processing_time_ms: u64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct FacetHit {
    pub value: String,
    pub highlighted: String,
    pub count: u64,
}
