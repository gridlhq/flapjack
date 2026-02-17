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
}

impl SearchRequest {
    pub fn effective_hits_per_page(&self) -> usize {
        self.hits_per_page.unwrap_or(20)
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
