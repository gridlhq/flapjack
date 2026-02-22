use axum::{
    extract::{Path, State},
    Json,
};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Instant;

use flapjack::error::FlapjackError;
use flapjack::experiments::{assignment, assignment::AssignmentMethod, config::QueryOverrides};

use super::AppState;
use crate::dto::SearchRequest;
use flapjack::query::highlighter::{
    extract_query_words, parse_snippet_spec, HighlightValue, Highlighter, MatchLevel, SnippetValue,
};
use flapjack::types::{FacetRequest, FieldValue, Sort, SortOrder};

use super::field_value_to_json;

#[derive(Debug, Clone)]
struct ExperimentContext {
    experiment_id: String,
    variant_id: String,
    assignment_method: String,
}

/// Extract userToken and client IP from request headers for analytics.
fn extract_analytics_headers(headers: &axum::http::HeaderMap) -> (Option<String>, Option<String>) {
    let user_token = headers
        .get("x-algolia-usertoken")
        .and_then(|v| v.to_str().ok())
        .map(String::from);
    let user_ip = headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.split(',').next().unwrap_or(s).trim().to_string())
        .or_else(|| {
            headers
                .get("x-real-ip")
                .and_then(|v| v.to_str().ok())
                .map(String::from)
        });
    (user_token, user_ip)
}

fn extract_single_geoloc(value: &FieldValue) -> Option<(f64, f64)> {
    match value {
        FieldValue::Object(map) => {
            let lat = match map.get("lat")? {
                FieldValue::Float(f) => *f,
                FieldValue::Integer(i) => *i as f64,
                _ => return None,
            };
            let lng = match map.get("lng")? {
                FieldValue::Float(f) => *f,
                FieldValue::Integer(i) => *i as f64,
                _ => return None,
            };
            Some((lat, lng))
        }
        _ => None,
    }
}

fn extract_all_geolocs(geoloc: Option<&FieldValue>) -> Vec<(f64, f64)> {
    match geoloc {
        None => vec![],
        Some(FieldValue::Object(_)) => extract_single_geoloc(geoloc.unwrap()).into_iter().collect(),
        Some(FieldValue::Array(arr)) => arr.iter().filter_map(extract_single_geoloc).collect(),
        _ => vec![],
    }
}

fn best_geoloc_for_filter(
    points: &[(f64, f64)],
    geo_params: &flapjack::query::geo::GeoParams,
) -> Option<(f64, f64)> {
    if points.is_empty() {
        return None;
    }
    if let Some(ref center) = geo_params.around {
        points
            .iter()
            .filter(|(lat, lng)| geo_params.filter_point(*lat, *lng))
            .min_by(|(lat_a, lng_a), (lat_b, lng_b)| {
                let da = flapjack::query::geo::haversine(center.lat, center.lng, *lat_a, *lng_a);
                let db = flapjack::query::geo::haversine(center.lat, center.lng, *lat_b, *lng_b);
                da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
            })
            .copied()
    } else {
        points
            .iter()
            .find(|(lat, lng)| geo_params.filter_point(*lat, *lng))
            .copied()
    }
}

fn highlight_value_map_to_json(map: &HashMap<String, HighlightValue>) -> serde_json::Value {
    let mut obj = serde_json::Map::new();
    for (k, v) in map {
        obj.insert(k.clone(), highlight_value_to_json(v));
    }
    serde_json::Value::Object(obj)
}

fn highlight_value_to_json(value: &HighlightValue) -> serde_json::Value {
    match value {
        HighlightValue::Single(result) => serde_json::to_value(result).unwrap(),
        HighlightValue::Array(results) => serde_json::Value::Array(
            results
                .iter()
                .map(|r| serde_json::to_value(r).unwrap())
                .collect(),
        ),
        HighlightValue::Object(map) => highlight_value_map_to_json(map),
    }
}

fn snippet_value_map_to_json(map: &HashMap<String, SnippetValue>) -> serde_json::Value {
    let mut obj = serde_json::Map::new();
    for (k, v) in map {
        obj.insert(k.clone(), snippet_value_to_json(v));
    }
    serde_json::Value::Object(obj)
}

fn snippet_value_to_json(value: &SnippetValue) -> serde_json::Value {
    match value {
        SnippetValue::Single(result) => serde_json::to_value(result).unwrap(),
        SnippetValue::Array(results) => serde_json::Value::Array(
            results
                .iter()
                .map(|r| serde_json::to_value(r).unwrap())
                .collect(),
        ),
        SnippetValue::Object(map) => snippet_value_map_to_json(map),
    }
}

/// Map synonym-matched words back to their original query terms.
/// This implements Algolia's replaceSynonymsInHighlight=false behavior:
/// - matchedWords shows original query terms (e.g., "notebook")
/// - Highlighted text shows document words (e.g., "laptop")
/// - Only replaces words that are actually synonyms, preserving partial matches
fn map_synonym_matches(
    value: HighlightValue,
    original_query_words: &[String],
    synonym_map: &HashMap<String, HashSet<String>>,
) -> HighlightValue {
    match value {
        HighlightValue::Single(mut result) => {
            if !result.matched_words.is_empty() {
                let mut mapped_words = HashSet::new();

                // For each matched word, check if it's a synonym and map back to original
                for matched in &result.matched_words {
                    let matched_lower = matched.to_lowercase();
                    let mut found_original = false;

                    // Check if this matched word is a synonym of any original query word
                    for original in original_query_words {
                        let original_lower = original.to_lowercase();
                        if let Some(synonyms) = synonym_map.get(&original_lower) {
                            if synonyms.contains(&matched_lower) || matched_lower == original_lower
                            {
                                mapped_words.insert(original_lower);
                                found_original = true;
                                break;
                            }
                        }
                    }

                    // If not a synonym, keep the original matched word
                    if !found_original {
                        mapped_words.insert(matched_lower);
                    }
                }

                result.matched_words = mapped_words.into_iter().collect();

                // Update matchLevel based on original query coverage
                if result.matched_words.len() == original_query_words.len() {
                    result.match_level = MatchLevel::Full;
                } else if !result.matched_words.is_empty() {
                    result.match_level = MatchLevel::Partial;
                }
            }
            HighlightValue::Single(result)
        }
        HighlightValue::Array(results) => {
            let updated = results
                .into_iter()
                .map(|r| {
                    if let HighlightValue::Single(s) = map_synonym_matches(
                        HighlightValue::Single(r),
                        original_query_words,
                        synonym_map,
                    ) {
                        s
                    } else {
                        unreachable!()
                    }
                })
                .collect();
            HighlightValue::Array(updated)
        }
        HighlightValue::Object(map) => {
            let updated = map
                .into_iter()
                .map(|(k, v)| (k, map_synonym_matches(v, original_query_words, synonym_map)))
                .collect();
            HighlightValue::Object(updated)
        }
    }
}

fn assignment_method_str(method: &AssignmentMethod) -> &'static str {
    match method {
        AssignmentMethod::UserToken => "user_token",
        AssignmentMethod::SessionId => "session_id",
        AssignmentMethod::QueryId => "query_id",
    }
}

fn apply_query_overrides(req: &mut SearchRequest, overrides: &QueryOverrides) {
    if let Some(ref typo_tolerance) = overrides.typo_tolerance {
        req.typo_tolerance = Some(typo_tolerance.clone());
    }
    if let Some(enable_synonyms) = overrides.enable_synonyms {
        req.enable_synonyms = Some(enable_synonyms);
    }
    if let Some(enable_rules) = overrides.enable_rules {
        req.enable_rules = Some(enable_rules);
    }
    if let Some(ref rule_contexts) = overrides.rule_contexts {
        req.rule_contexts = Some(rule_contexts.clone());
    }
    if let Some(ref filters) = overrides.filters {
        req.filters = Some(filters.clone());
    }
    if let Some(ref optional_filters) = overrides.optional_filters {
        req.optional_filters = Some(serde_json::Value::Array(
            optional_filters
                .iter()
                .cloned()
                .map(serde_json::Value::String)
                .collect(),
        ));
    }
    if let Some(ref remove_words_if_no_results) = overrides.remove_words_if_no_results {
        req.remove_words_if_no_results = Some(remove_words_if_no_results.clone());
    }

    if overrides.custom_ranking.is_some() {
        tracing::debug!("skipping custom_ranking query override (index-level only)");
    }
    if overrides.attribute_weights.is_some() {
        tracing::debug!("skipping attribute_weights query override (index-level only)");
    }
}

fn resolve_experiment_context(
    state: &AppState,
    index_name: &str,
    req: &mut SearchRequest,
    assignment_query_id: &str,
) -> (String, Option<ExperimentContext>) {
    let mut effective_index = index_name.to_string();
    let Some(store) = state.experiment_store.as_ref() else {
        return (effective_index, None);
    };
    let Some(experiment) = store.get_active_for_index(index_name) else {
        return (effective_index, None);
    };
    // get_active_for_index already filters for Running status

    let assignment = assignment::assign_variant(
        &experiment,
        req.user_token.as_deref(),
        None,
        assignment_query_id,
    );
    let (variant_id, arm) = if assignment.arm == "variant" {
        ("variant", &experiment.variant)
    } else {
        ("control", &experiment.control)
    };

    if let Some(ref overrides) = arm.query_overrides {
        apply_query_overrides(req, overrides);
    }
    if let Some(ref routed_index) = arm.index_name {
        effective_index = routed_index.clone();
    }

    (
        effective_index,
        Some(ExperimentContext {
            experiment_id: experiment.id,
            variant_id: variant_id.to_string(),
            assignment_method: assignment_method_str(&assignment.method).to_string(),
        }),
    )
}

fn build_search_event(
    req: &SearchRequest,
    query_id: Option<String>,
    index_name: String,
    nb_hits: usize,
    processing_time_ms: u32,
    page: usize,
    hits_per_page: usize,
    experiment_ctx: Option<&ExperimentContext>,
) -> flapjack::analytics::schema::SearchEvent {
    let analytics_tags = req.analytics_tags.as_ref().map(|tags| tags.join(","));
    let facets = req
        .facets
        .as_ref()
        .map(|facet_list| serde_json::to_string(facet_list).unwrap_or_default());
    flapjack::analytics::schema::SearchEvent {
        timestamp_ms: chrono::Utc::now().timestamp_millis(),
        query: req.query.clone(),
        query_id,
        index_name,
        nb_hits: nb_hits as u32,
        processing_time_ms,
        user_token: req.user_token.clone(),
        user_ip: req.user_ip.clone(),
        filters: req.filters.clone(),
        facets,
        analytics_tags,
        page: page as u32,
        hits_per_page: hits_per_page as u32,
        has_results: nb_hits > 0,
        country: None,
        region: None,
        experiment_id: experiment_ctx.map(|ctx| ctx.experiment_id.clone()),
        variant_id: experiment_ctx.map(|ctx| ctx.variant_id.clone()),
        assignment_method: experiment_ctx.map(|ctx| ctx.assignment_method.clone()),
    }
}

fn merge_secured_filters(
    req: &mut SearchRequest,
    restrictions: &crate::auth::SecuredKeyRestrictions,
) {
    if let Some(ref forced_filters) = restrictions.filters {
        match &req.filters {
            Some(existing) => {
                req.filters = Some(format!("({}) AND ({})", existing, forced_filters));
            }
            None => {
                req.filters = Some(forced_filters.clone());
            }
        }
    }
    if let Some(hpp) = restrictions.hits_per_page {
        if req.hits_per_page.is_none_or(|h| h > hpp) {
            req.hits_per_page = Some(hpp);
        }
    }
}

/// Batch search across multiple queries
#[utoipa::path(
    post,
    path = "/1/indexes/{indexName}/queries",
    tag = "search",
    params(
        ("indexName" = String, Path, description = "Index to search")
    ),
    request_body(content = serde_json::Value, description = "Batch search request with multiple queries"),
    responses(
        (status = 200, description = "Batch search results", body = serde_json::Value),
        (status = 404, description = "Index not found")
    ),
    security(
        ("api_key" = [])
    )
)]
pub async fn batch_search(
    State(state): State<Arc<AppState>>,
    request: axum::extract::Request,
) -> Result<Json<serde_json::Value>, FlapjackError> {
    let secured_restrictions = request
        .extensions()
        .get::<crate::auth::SecuredKeyRestrictions>()
        .cloned();
    let (user_token_header, user_ip) = extract_analytics_headers(request.headers());
    let body_bytes = axum::body::to_bytes(request.into_body(), 10_000_000)
        .await
        .map_err(|e| FlapjackError::InvalidQuery(format!("Failed to read body: {}", e)))?;
    let body: serde_json::Value = serde_json::from_slice(&body_bytes)
        .map_err(|e| FlapjackError::InvalidQuery(format!("Invalid JSON: {}", e)))?;
    #[derive(serde::Deserialize)]
    struct BatchSearchRequest {
        requests: Vec<SearchRequest>,
    }

    let batch: BatchSearchRequest = serde_json::from_value(body.clone()).map_err(|e| {
        tracing::error!("DESERIALIZATION FAILED on body: {}", body);
        FlapjackError::InvalidQuery(format!("Invalid batch search: {}", e))
    })?;

    // Validate all requests up front, then execute in parallel.
    let mut prepared: Vec<(usize, String, SearchRequest)> = Vec::new();
    for (i, mut req) in batch.requests.into_iter().enumerate() {
        req.apply_params_string();
        if req.user_token.is_none() {
            req.user_token = user_token_header.clone();
        }
        req.user_ip = user_ip.clone();
        if let Some(ref restrictions) = secured_restrictions {
            merge_secured_filters(&mut req, restrictions);
            if let Some(ref restrict_indices) = restrictions.restrict_indices {
                if let Some(ref idx) = req.index_name {
                    if !crate::auth::index_pattern_matches(restrict_indices, idx) {
                        return Err(FlapjackError::InvalidQuery("Index not allowed".to_string()));
                    }
                }
            }
        }
        let index_name = req
            .index_name
            .clone()
            .ok_or_else(|| FlapjackError::InvalidQuery("Missing indexName".to_string()))?;
        prepared.push((i, index_name, req));
    }

    let mut join_set = tokio::task::JoinSet::new();
    for (i, index_name, req) in prepared {
        let state = state.clone();
        // Route type=facet queries to the facet search handler
        if req.query_type.as_deref() == Some("facet") {
            let facet_name = req.facet.clone().unwrap_or_default();
            let facet_query = req.facet_query.clone().unwrap_or_default();
            let max_facet_hits = req.max_facet_hits.unwrap_or(10);
            let filters = req.filters.clone();
            join_set.spawn(async move {
                let result = super::facets::search_facet_values_inline(
                    state,
                    &index_name,
                    &facet_name,
                    &facet_query,
                    max_facet_hits,
                    filters.as_deref(),
                )
                .await?;
                Ok::<_, FlapjackError>((i, result))
            });
        } else {
            join_set.spawn(async move {
                let result = search_single(State(state), index_name, req).await?;
                Ok::<_, FlapjackError>((i, result.0))
            });
        }
    }

    let mut indexed_results: Vec<(usize, serde_json::Value)> = Vec::with_capacity(join_set.len());
    while let Some(join_result) = join_set.join_next().await {
        let result = join_result
            .map_err(|e| FlapjackError::InvalidQuery(format!("Task join error: {}", e)))?;
        indexed_results.push(result?);
    }
    indexed_results.sort_by_key(|(i, _)| *i);
    let results: Vec<serde_json::Value> = indexed_results.into_iter().map(|(_, v)| v).collect();

    Ok(Json(serde_json::json!({"results": results})))
}

pub async fn search_single(
    State(state): State<Arc<AppState>>,
    index_name: String,
    mut req: SearchRequest,
) -> Result<Json<serde_json::Value>, FlapjackError> {
    // Move CPU-bound search + highlighting + JSON serialization off the async
    // runtime. On t4g.micro (2 vCPUs) this prevents worker-thread starvation
    // when multiple searches run concurrently.
    let enqueue_time = Instant::now();

    // Generate queryID for click analytics correlation before assignment.
    let query_id = if req.click_analytics == Some(true) {
        Some(hex::encode(uuid::Uuid::new_v4().as_bytes()))
    } else {
        None
    };
    let assignment_query_id = query_id
        .clone()
        .unwrap_or_else(|| hex::encode(uuid::Uuid::new_v4().as_bytes()));
    let (effective_index, experiment_ctx) =
        resolve_experiment_context(&state, &index_name, &mut req, &assignment_query_id);

    // --- Hybrid search: resolve query vector before spawn_blocking ---
    #[cfg(feature = "vector-search")]
    let (query_vector, hybrid_params) = {
        use crate::dto::HybridSearchParams;
        use flapjack::index::settings::IndexMode;

        let mut qv: Option<Vec<f32>> = None;
        let mut hp: Option<HybridSearchParams> = None;

        // Determine if hybrid search is active
        let settings = state.manager.get_settings(&effective_index);
        let is_hybrid = if req.hybrid.is_some() {
            true
        } else if let Some(ref s) = settings {
            matches!(resolve_search_mode(&req.mode, s), IndexMode::NeuralSearch)
        } else {
            false
        };

        if is_hybrid {
            // Resolve hybrid params: explicit from request, or synthesized from neuralSearch mode
            let mut params = req.hybrid.clone().unwrap_or(HybridSearchParams {
                semantic_ratio: 0.5,
                embedder: "default".to_string(),
            });
            params.clamp_ratio();

            // Pure BM25 requested (ratio=0.0) — skip vector search entirely
            if params.semantic_ratio > 0.0 {
                let embedder_name = params.embedder.as_str();

                // Try query cache first
                if let Some(cached) = state
                    .embedder_store
                    .query_cache
                    .get(embedder_name, &req.query)
                {
                    qv = Some(cached);
                } else {
                    // Cache miss — try to embed the query
                    if let Some(ref s) = settings {
                        match state
                            .embedder_store
                            .get_or_create(&effective_index, embedder_name, s)
                        {
                            Ok(embedder) => match embedder.embed_query(&req.query).await {
                                Ok(vec) => {
                                    state.embedder_store.query_cache.insert(
                                        embedder_name,
                                        &req.query,
                                        vec.clone(),
                                    );
                                    qv = Some(vec);
                                }
                                Err(e) => {
                                    tracing::warn!(
                                        "hybrid search: embedding failed for '{}': {}",
                                        effective_index,
                                        e
                                    );
                                }
                            },
                            Err(e) => {
                                tracing::warn!(
                                    "hybrid search: embedder resolution failed for '{}': {}",
                                    effective_index,
                                    e
                                );
                            }
                        }
                    }
                }

                hp = Some(params);
            }
        }

        (qv, hp)
    };

    #[cfg(not(feature = "vector-search"))]
    let (query_vector, hybrid_params): (Option<Vec<f32>>, Option<()>) = (None, None);

    tokio::task::spawn_blocking(move || {
        search_single_sync(
            state,
            index_name,
            effective_index,
            req,
            enqueue_time,
            query_id,
            experiment_ctx,
            query_vector,
            hybrid_params,
        )
    })
    .await
    .map_err(|e| FlapjackError::InvalidQuery(format!("spawn_blocking join error: {}", e)))?
}

fn search_single_sync(
    state: Arc<AppState>,
    index_name: String,
    effective_index: String,
    req: SearchRequest,
    enqueue_time: Instant,
    query_id: Option<String>,
    experiment_ctx: Option<ExperimentContext>,
    #[cfg(feature = "vector-search")] query_vector: Option<Vec<f32>>,
    #[cfg(feature = "vector-search")] hybrid_params: Option<crate::dto::HybridSearchParams>,
    #[cfg(not(feature = "vector-search"))] _query_vector: Option<Vec<f32>>,
    #[cfg(not(feature = "vector-search"))] _hybrid_params: Option<()>,
) -> Result<Json<serde_json::Value>, FlapjackError> {
    let queue_wait = enqueue_time.elapsed();
    let start = Instant::now();

    let filter = req.build_combined_filter();

    let sort = if let Some(sort_specs) = &req.sort {
        if let Some(first) = sort_specs.first() {
            if first.ends_with(":asc") {
                let field = first.trim_end_matches(":asc").to_string();
                Some(Sort::ByField {
                    field,
                    order: SortOrder::Asc,
                })
            } else if first.ends_with(":desc") {
                let field = first.trim_end_matches(":desc").to_string();
                Some(Sort::ByField {
                    field,
                    order: SortOrder::Desc,
                })
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    };

    let loaded_settings = state.manager.get_settings(&effective_index);

    let facet_requests = req.facets.as_ref().and_then(|facets| {
        let allowed_facets = loaded_settings.as_ref().map(|s| s.facet_set());

        let effective_facets: Vec<String> = if facets.iter().any(|f| f == "*") {
            match &allowed_facets {
                Some(allowed) => allowed.iter().cloned().collect(),
                None => Vec::new(),
            }
        } else {
            facets
                .iter()
                .filter(|f| match &allowed_facets {
                    Some(allowed) => allowed.contains(f.as_str()),
                    None => true,
                })
                .cloned()
                .collect()
        };

        let filtered_facets: Vec<FacetRequest> = effective_facets
            .iter()
            .map(|f| FacetRequest {
                field: f.clone(),
                path: format!("/{}", f),
            })
            .collect();

        if filtered_facets.is_empty() {
            None
        } else {
            Some(filtered_facets)
        }
    });

    let distinct_count = match &req.distinct {
        Some(serde_json::Value::Bool(true)) => loaded_settings
            .as_ref()
            .and_then(|s| s.distinct.as_ref())
            .map(|d| d.as_count())
            .or(Some(1)),
        Some(serde_json::Value::Bool(false)) => Some(0),
        Some(serde_json::Value::Number(n)) => n.as_u64().map(|u| u as u32),
        _ => loaded_settings
            .as_ref()
            .and_then(|s| s.distinct.as_ref())
            .map(|d| d.as_count()),
    };

    let geo_params = req.build_geo_params();

    let hits_per_page = req.effective_hits_per_page();

    // For hybrid search, over-fetch BM25 results for RRF fusion (re-ranking
    // invalidates source-level pagination). Both BM25 and vector fetch this
    // many results; post-fusion pagination selects the requested window.
    #[cfg(feature = "vector-search")]
    let is_hybrid_active = query_vector.is_some();
    #[cfg(not(feature = "vector-search"))]
    let is_hybrid_active = false;

    let (fetch_limit, fetch_offset) = if is_hybrid_active {
        let limit = (hits_per_page * (req.page + 1) + 50).max(200);
        (limit, 0)
    } else if geo_params.has_geo_filter() {
        (
            (hits_per_page + req.page * hits_per_page)
                .saturating_mul(10)
                .max(1000),
            0,
        )
    } else {
        (hits_per_page, req.page * hits_per_page)
    };
    let typo_tolerance = match &req.typo_tolerance {
        Some(serde_json::Value::Bool(false)) => Some(false),
        Some(serde_json::Value::String(s)) if s == "false" => Some(false),
        _ => None,
    };
    let optional_filter_specs = req
        .optional_filters
        .as_ref()
        .map(crate::dto::parse_optional_filters)
        .filter(|v| !v.is_empty());

    let result = state.manager.search_full_with_stop_words(
        &effective_index,
        &req.query,
        filter.as_ref(),
        sort.as_ref(),
        fetch_limit,
        fetch_offset,
        facet_requests.as_deref(),
        distinct_count,
        req.max_values_per_facet,
        req.remove_stop_words.as_ref(),
        req.ignore_plurals.as_ref(),
        req.query_languages.as_ref(),
        req.query_type_prefix.as_deref(),
        typo_tolerance,
        req.advanced_syntax,
        req.remove_words_if_no_results.as_deref(),
        optional_filter_specs.as_deref(),
        req.enable_synonyms,
        req.enable_rules,
        req.rule_contexts.as_deref(),
        req.restrict_searchable_attributes.as_deref(),
    )?;

    // --- Hybrid search: RRF fusion with vector results ---
    #[allow(unused_mut)]
    let mut fallback_message: Option<String> = None;

    #[cfg(feature = "vector-search")]
    let mut result = result;

    #[cfg(feature = "vector-search")]
    if let (Some(qv), Some(ref hp)) = (&query_vector, &hybrid_params) {
        let vi_opt = state.manager.get_vector_index(&effective_index);
        match vi_opt {
            Some(vi_arc) => match vi_arc.read() {
                Ok(vi_guard) => {
                    if vi_guard.is_empty() {
                        fallback_message = Some(
                            "Hybrid search unavailable: vector index is empty. Falling back to keyword search.".to_string()
                        );
                    } else {
                        let vec_fetch_limit = (hits_per_page * (req.page + 1) + 50).max(200);
                        match vi_guard.search(qv, vec_fetch_limit) {
                            Ok(vector_results) => {
                                // Extract BM25 doc IDs in ranked order
                                let bm25_ids: Vec<String> = result
                                    .documents
                                    .iter()
                                    .map(|d| d.document.id.clone())
                                    .collect();

                                let fused = crate::fusion::rrf_fuse(
                                    &bm25_ids,
                                    &vector_results,
                                    hp.semantic_ratio,
                                    60,
                                );

                                // Build a lookup map from BM25 results
                                let mut bm25_map: HashMap<String, flapjack::types::ScoredDocument> =
                                    result
                                        .documents
                                        .drain(..)
                                        .map(|sd| (sd.document.id.clone(), sd))
                                        .collect();

                                // Build fused document list, fetching vector-only docs as needed
                                let mut fused_docs = Vec::new();
                                for fr in &fused {
                                    if let Some(sd) = bm25_map.remove(&fr.doc_id) {
                                        fused_docs.push(flapjack::types::ScoredDocument {
                                            document: sd.document,
                                            score: fr.fused_score as f32,
                                        });
                                    } else {
                                        // Vector-only doc: fetch from Tantivy by ID
                                        match state
                                            .manager
                                            .get_document(&effective_index, &fr.doc_id)
                                        {
                                            Ok(Some(doc)) => {
                                                fused_docs.push(flapjack::types::ScoredDocument {
                                                    document: doc,
                                                    score: fr.fused_score as f32,
                                                });
                                            }
                                            Ok(None) => {
                                                // Document was in vector index but not in Tantivy
                                                // (deleted but not yet removed from vector index).
                                                // Skip it silently.
                                            }
                                            Err(e) => {
                                                tracing::warn!(
                                                    "hybrid search: failed to fetch vector-only doc '{}': {}",
                                                    fr.doc_id,
                                                    e
                                                );
                                            }
                                        }
                                    }
                                }

                                let total_fused = fused_docs.len();

                                // Paginate the fused results to the requested window
                                let page_start = (req.page * hits_per_page).min(total_fused);
                                let page_end = (page_start + hits_per_page).min(total_fused);
                                result.documents = fused_docs[page_start..page_end].to_vec();
                                result.total = total_fused;
                            }
                            Err(e) => {
                                tracing::warn!(
                                    "hybrid search: vector search failed for '{}': {}",
                                    effective_index,
                                    e
                                );
                                fallback_message = Some(format!(
                                    "Hybrid search unavailable: vector search failed: {}. Falling back to keyword search.",
                                    e
                                ));
                            }
                        }
                    }
                }
                Err(e) => {
                    tracing::error!(
                        "vector index read lock poisoned for '{}': {}",
                        effective_index,
                        e
                    );
                    fallback_message = Some(
                        "Hybrid search unavailable: internal error. Falling back to keyword search.".to_string()
                    );
                }
            },
            None => {
                fallback_message = Some(
                    "Hybrid search unavailable: no vector index for this tenant. Falling back to keyword search.".to_string()
                );
            }
        }
    }

    #[cfg(feature = "vector-search")]
    if query_vector.is_none() && hybrid_params.is_some() {
        // Hybrid was requested but embedding failed (no query vector)
        if fallback_message.is_none() {
            fallback_message = Some(
                "Hybrid search unavailable: no embedders configured. Falling back to keyword search.".to_string()
            );
        }
    }

    let search_elapsed = start.elapsed();

    // Extract original query words for matchedWords (Algolia compatibility)
    let original_query_words = extract_query_words(&req.query);

    // For highlighting, expand to include synonyms to highlight all variant matches
    let mut query_words = original_query_words.clone();

    // Build synonym mapping for matchedWords translation
    let mut synonym_map: HashMap<String, HashSet<String>> = HashMap::new();

    // If synonyms are enabled, expand query words to highlight all synonym matches
    if req.enable_synonyms.unwrap_or(true) {
        if let Some(synonym_store) = state.manager.get_synonyms(&effective_index) {
            let expanded_queries = synonym_store.expand_query(&req.query);
            // Extract words from all expanded queries and add to query_words
            let mut all_words: std::collections::HashSet<String> =
                query_words.iter().cloned().collect();
            for expanded in &expanded_queries {
                for word in extract_query_words(expanded) {
                    all_words.insert(word);
                }
            }
            query_words = all_words.into_iter().collect();

            // Build synonym map: original_word -> set of synonyms (including original)
            for original in &original_query_words {
                let original_lower = original.to_lowercase();
                let mut synonyms = HashSet::new();
                synonyms.insert(original_lower.clone());

                // Add all words that are synonyms of this original word
                for word in &query_words {
                    let word_lower = word.to_lowercase();
                    // If this word appeared from synonym expansion and wasn't in original query
                    if !original_query_words
                        .iter()
                        .any(|o| o.to_lowercase() == word_lower)
                    {
                        // Check if it's related via synonym expansion
                        // We can identify this by checking if both words appear in expanded queries
                        synonyms.insert(word_lower);
                    }
                }

                synonym_map.insert(original_lower, synonyms);
            }
        }
    }

    let highlighter = match (&req.highlight_pre_tag, &req.highlight_post_tag) {
        (Some(pre), Some(post)) => Highlighter::new(pre.clone(), post.clone()),
        _ => Highlighter::default(),
    };

    let searchable_paths = loaded_settings
        .as_ref()
        .and_then(|s| s.searchable_attributes.as_ref())
        .cloned()
        .unwrap_or_default();

    let mut geo_distances: HashMap<String, (f64, f64, f64)> = HashMap::new();
    let mut automatic_radius: Option<u64> = None;

    let result = if geo_params.has_geo_filter() {
        let mut geo_docs: Vec<(flapjack::types::ScoredDocument, Option<f64>)> = result
            .documents
            .into_iter()
            .filter_map(|scored_doc| {
                let geoloc = scored_doc.document.fields.get("_geoloc");
                let points = extract_all_geolocs(geoloc);
                let (lat, lng) = best_geoloc_for_filter(&points, &geo_params)?;
                let dist = geo_params.distance_from_center(lat, lng);
                if let Some(d) = dist {
                    geo_distances.insert(scored_doc.document.id.clone(), (d, lat, lng));
                }
                Some((scored_doc, dist))
            })
            .collect();

        if geo_params.has_around() && geo_params.around_radius.is_none() {
            geo_docs.sort_by(|a, b| {
                let da = a.1.unwrap_or(f64::MAX);
                let db = b.1.unwrap_or(f64::MAX);
                da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
            });
            let target_count = 1000.min(geo_docs.len());
            let density_radius = if target_count > 0 && target_count < geo_docs.len() {
                geo_docs[target_count - 1].1.unwrap_or(0.0) as u64
            } else {
                geo_docs
                    .last()
                    .and_then(|d| d.1)
                    .map(|d| d as u64)
                    .unwrap_or(0)
            };
            let effective_radius = match geo_params.minimum_around_radius {
                Some(min_r) => density_radius.max(min_r),
                None => density_radius,
            };
            automatic_radius = Some(effective_radius);
            let effective_radius_f = effective_radius as f64;
            geo_docs.retain(|(_doc, dist)| {
                dist.map(|d| d <= effective_radius_f + 1.0).unwrap_or(false)
            });
        }

        if geo_params.has_around() {
            if geo_params.around_precision.fixed.is_some()
                || !geo_params.around_precision.ranges.is_empty()
            {
                geo_docs.sort_by(|a, b| {
                    let da = a.1.unwrap_or(f64::MAX);
                    let db = b.1.unwrap_or(f64::MAX);
                    let ba = geo_params.around_precision.bucket_distance(da);
                    let bb = geo_params.around_precision.bucket_distance(db);
                    ba.cmp(&bb)
                });
            } else {
                geo_docs.sort_by(|a, b| {
                    let da = a.1.unwrap_or(f64::MAX);
                    let db = b.1.unwrap_or(f64::MAX);
                    da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
                });
            }
        }

        let total_geo = geo_docs.len();
        let start = (req.page * hits_per_page).min(total_geo);
        let end = (start + hits_per_page).min(total_geo);
        let docs: Vec<flapjack::types::ScoredDocument> = geo_docs[start..end]
            .iter()
            .map(|(d, _)| d.clone())
            .collect();
        flapjack::types::SearchResult {
            documents: docs,
            total: total_geo,
            facets: result.facets,
            user_data: result.user_data,
            applied_rules: result.applied_rules,
        }
    } else {
        result
    };

    let highlight_start = Instant::now();
    let hits: Vec<serde_json::Value> = result
        .documents
        .iter()
        .map(|scored_doc| {
            let mut doc_map = serde_json::Map::new();
            doc_map.insert(
                "objectID".to_string(),
                serde_json::Value::String(scored_doc.document.id.clone()),
            );

            for (key, value) in &scored_doc.document.fields {
                if let Some(ref attrs) = req.attributes_to_retrieve {
                    if !attrs.contains(key) && !attrs.iter().any(|a| a == "*") {
                        continue;
                    }
                } else if let Some(ref settings) = loaded_settings {
                    if !settings.should_retrieve(key) {
                        continue;
                    }
                }
                doc_map.insert(key.clone(), field_value_to_json(value));
            }

            let skip_highlight =
                matches!(&req.attributes_to_highlight, Some(attrs) if attrs.is_empty());
            if !skip_highlight {
                let mut highlight_map = highlighter.highlight_document(
                    &scored_doc.document,
                    &query_words,
                    &searchable_paths,
                );

                // Map synonym matches back to original query terms (replaceSynonymsInHighlight=false)
                if !synonym_map.is_empty() {
                    highlight_map = highlight_map
                        .into_iter()
                        .map(|(k, v)| {
                            (
                                k,
                                map_synonym_matches(v, &original_query_words, &synonym_map),
                            )
                        })
                        .collect();
                }

                let highlight_json = highlight_value_map_to_json(&highlight_map);
                doc_map.insert("_highlightResult".to_string(), highlight_json);
            }

            // Snippet generation
            if let Some(ref snippet_attrs) = req.attributes_to_snippet {
                if !snippet_attrs.is_empty() {
                    let snippet_specs: Vec<(&str, usize)> = snippet_attrs
                        .iter()
                        .map(|s| parse_snippet_spec(s.as_str()))
                        .collect();
                    let snippet_map = highlighter.snippet_document(
                        &scored_doc.document,
                        &query_words,
                        &snippet_specs,
                    );
                    let snippet_json = snippet_value_map_to_json(&snippet_map);
                    doc_map.insert("_snippetResult".to_string(), snippet_json);
                }
            }

            if req.get_ranking_info == Some(true) {
                let mut ranking_info = serde_json::json!({
                    "nbTypos": 0,
                    "firstMatchedWord": 0,
                    "proximityDistance": 0,
                    "userScore": 0,
                    "geoDistance": 0,
                    "geoPrecision": 1,
                    "nbExactWords": 0,
                    "words": 0,
                    "filters": 0
                });
                if let Some(&(dist, lat, lng)) = geo_distances.get(&scored_doc.document.id) {
                    let precision = if geo_params.around_precision.fixed.is_some()
                        || !geo_params.around_precision.ranges.is_empty()
                    {
                        let bucket = geo_params.around_precision.bucket_distance(dist);
                        if bucket > 0 {
                            (dist as u64) / bucket
                        } else {
                            1
                        }
                    } else {
                        1
                    };
                    ranking_info["geoDistance"] =
                        serde_json::json!((dist as u64) / precision.max(1));
                    ranking_info["geoPrecision"] = serde_json::json!(precision);
                    ranking_info["matchedGeoLocation"] = serde_json::json!({
                        "lat": lat,
                        "lng": lng,
                        "distance": dist as u64
                    });
                }
                doc_map.insert("_rankingInfo".to_string(), ranking_info);
            }

            serde_json::Value::Object(doc_map)
        })
        .collect();
    let highlight_elapsed = highlight_start.elapsed();

    let facet_distribution = if req.facets.is_some() {
        if result.total == 0 {
            Some(std::collections::HashMap::new())
        } else if !result.facets.is_empty() {
            Some(
                result
                    .facets
                    .into_iter()
                    .map(|(field, counts)| {
                        let facet_map: serde_json::Map<String, serde_json::Value> = counts
                            .into_iter()
                            .map(|fc| (fc.path, serde_json::json!(fc.count)))
                            .collect();
                        (field, serde_json::Value::Object(facet_map))
                    })
                    .collect::<std::collections::HashMap<String, serde_json::Value>>(),
            )
        } else {
            Some(std::collections::HashMap::new())
        }
    } else {
        None
    };

    let page = req.page;
    let hits_per_page = req.effective_hits_per_page();
    let nb_pages = if result.total > 0 && hits_per_page > 0 {
        result.total.div_ceil(hits_per_page)
    } else {
        0
    };

    let params_str = {
        let mut params = Vec::new();
        if !req.query.is_empty() {
            params.push(format!("query={}", urlencoding::encode(&req.query)));
        }
        params.push(format!("hitsPerPage={}", hits_per_page));
        if page != 0 {
            params.push(format!("page={}", page));
        }
        if let Some(ref f) = req.filters {
            params.push(format!("filters={}", urlencoding::encode(f)));
        }
        if let Some(ref s) = req.sort {
            if !s.is_empty() {
                params.push(format!("sort={}", urlencoding::encode(&s.join(","))));
            }
        }
        if let Some(ref facets) = req.facets {
            if !facets.is_empty() {
                let facets_str = serde_json::to_string(facets).unwrap_or_default();
                params.push(format!("facets={}", urlencoding::encode(&facets_str)));
            }
        }
        params.join("&")
    };

    let mut exhaustive_obj = serde_json::json!({
        "nbHits": true,
        "typo": true
    });

    if req.facets.is_some() {
        exhaustive_obj["facetsCount"] = serde_json::json!(true);
    }

    let total_elapsed = start.elapsed();

    let mut response = serde_json::json!({
        "hits": hits,
        "nbHits": result.total,
        "page": page,
        "nbPages": nb_pages,
        "hitsPerPage": hits_per_page,
        "processingTimeMS": total_elapsed.as_millis() as u64,
        "serverTimeMS": total_elapsed.as_millis() as u64,
        "query": req.query,
        "params": params_str,
        "exhaustive": exhaustive_obj,
        "exhaustiveNbHits": true,
        "exhaustiveTypo": true,
        "index": index_name,
        "renderingContent": {},
        "processingTimingsMS": {
            "queue": queue_wait.as_micros() as u64,
            "search": search_elapsed.as_micros() as u64,
            "highlight": highlight_elapsed.as_micros() as u64,
            "total": total_elapsed.as_micros() as u64
        }
    });

    if req.facets.is_some() {
        response["exhaustiveFacetsCount"] = serde_json::json!(true);
    }

    match facet_distribution {
        Some(facets) if facets.is_empty() => {
            response["facets"] = serde_json::json!({});
        }
        Some(facets) => {
            response["facets"] = serde_json::Value::Object(facets.into_iter().collect());
        }
        None => {}
    }

    if !result.user_data.is_empty() {
        response["userData"] = serde_json::Value::Array(result.user_data);
    }

    if let Some(auto_r) = automatic_radius {
        response["automaticRadius"] = serde_json::json!(auto_r.to_string());
    }

    if !result.applied_rules.is_empty() {
        response["appliedRules"] = serde_json::Value::Array(
            result
                .applied_rules
                .into_iter()
                .map(|id| serde_json::json!({ "objectID": id }))
                .collect(),
        );
    }

    // Add queryID to response when clickAnalytics is enabled
    if let Some(ref qid) = query_id {
        response["queryID"] = serde_json::json!(qid);
    }

    // Add hybrid search fallback warning if applicable
    if let Some(ref msg) = fallback_message {
        response["message"] = serde_json::json!(msg);
    }

    if let Some(ref fields) = req.response_fields {
        if !fields.contains(&"*".to_string()) {
            let response_obj = response.as_object_mut().unwrap();
            let keys: Vec<String> = response_obj.keys().cloned().collect();
            for key in keys {
                if !fields.contains(&key) {
                    response_obj.remove(&key);
                }
            }
        }
    }

    if let Some(ref ctx) = experiment_ctx {
        response["abTestID"] = serde_json::json!(ctx.experiment_id);
        response["abTestVariantID"] = serde_json::json!(ctx.variant_id);
    }
    if effective_index != index_name {
        response["indexUsed"] = serde_json::json!(effective_index.clone());
    }

    // Increment usage counter: search_results_total
    {
        let entry = state
            .usage_counters
            .entry(effective_index.clone())
            .or_insert_with(crate::usage_middleware::TenantUsageCounters::new);
        entry
            .search_results_total
            .fetch_add(result.total as u64, std::sync::atomic::Ordering::Relaxed);
    }

    // Record analytics event (fire-and-forget, never blocks search response)
    if req.analytics != Some(false) {
        if let Some(collector) = flapjack::analytics::get_global_collector() {
            collector.record_search(build_search_event(
                &req,
                query_id.clone(),
                effective_index.clone(),
                result.total,
                total_elapsed.as_millis() as u32,
                page,
                hits_per_page,
                experiment_ctx.as_ref(),
            ));
        }
    }

    Ok(Json(response))
}

/// Search an index with full-text query and filters
#[utoipa::path(
    post,
    path = "/1/indexes/{indexName}/query",
    tag = "search",
    params(
        ("indexName" = String, Path, description = "Index to search")
    ),
    request_body(content = SearchRequest, description = "Search parameters including query, filters, facets, and pagination"),
    responses(
        (status = 200, description = "Search results with hits and facets", body = serde_json::Value),
        (status = 404, description = "Index not found")
    ),
    security(
        ("api_key" = [])
    )
)]
pub async fn search(
    State(state): State<Arc<AppState>>,
    Path(index_name): Path<String>,
    request: axum::extract::Request,
) -> Result<Json<serde_json::Value>, FlapjackError> {
    let secured_restrictions = request
        .extensions()
        .get::<crate::auth::SecuredKeyRestrictions>()
        .cloned();
    let (user_token_header, user_ip) = extract_analytics_headers(request.headers());
    let body_bytes = axum::body::to_bytes(request.into_body(), 10_000_000)
        .await
        .map_err(|e| FlapjackError::InvalidQuery(format!("Failed to read body: {}", e)))?;
    let mut req: SearchRequest = serde_json::from_slice(&body_bytes)
        .map_err(|e| FlapjackError::InvalidQuery(format!("Invalid JSON: {}", e)))?;
    if let Some(ref restrictions) = secured_restrictions {
        merge_secured_filters(&mut req, restrictions);
    }
    if req.user_token.is_none() {
        req.user_token = user_token_header;
    }
    req.user_ip = user_ip;
    search_single(State(state), index_name, req).await
}

/// Resolve the effective search mode from per-query override and index settings.
///
/// Priority: query mode > settings mode > KeywordSearch default.
/// Stage 6 will call this from the search handler to determine whether to run hybrid search.
pub fn resolve_search_mode(
    query_mode: &Option<flapjack::index::settings::IndexMode>,
    settings: &flapjack::index::settings::IndexSettings,
) -> flapjack::index::settings::IndexMode {
    use flapjack::index::settings::IndexMode;
    if let Some(mode) = query_mode {
        return mode.clone();
    }
    if let Some(mode) = &settings.mode {
        return mode.clone();
    }
    IndexMode::KeywordSearch
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handlers::metrics::MetricsState;
    use axum::{
        body::Body,
        http::{Method, Request, StatusCode},
        routing::post,
        Router,
    };
    use flapjack::experiments::{
        assignment::{self, AssignmentMethod},
        config::{Experiment, ExperimentArm, ExperimentStatus, PrimaryMetric, QueryOverrides},
        store::ExperimentStore,
    };
    use flapjack::types::Document;
    use flapjack::types::FieldValue;
    use flapjack::IndexManager;
    use serde_json::{json, Value};
    use std::collections::HashMap;
    use tempfile::TempDir;
    use tower::ServiceExt;

    // ── extract_analytics_headers ──

    #[test]
    fn analytics_headers_empty() {
        let headers = axum::http::HeaderMap::new();
        let (token, ip) = extract_analytics_headers(&headers);
        assert!(token.is_none());
        assert!(ip.is_none());
    }

    #[test]
    fn analytics_headers_user_token() {
        let mut headers = axum::http::HeaderMap::new();
        headers.insert("x-algolia-usertoken", "user123".parse().unwrap());
        let (token, _) = extract_analytics_headers(&headers);
        assert_eq!(token, Some("user123".to_string()));
    }

    #[test]
    fn analytics_headers_forwarded_for() {
        let mut headers = axum::http::HeaderMap::new();
        headers.insert("x-forwarded-for", "1.2.3.4, 5.6.7.8".parse().unwrap());
        let (_, ip) = extract_analytics_headers(&headers);
        assert_eq!(ip, Some("1.2.3.4".to_string()));
    }

    #[test]
    fn analytics_headers_real_ip_fallback() {
        let mut headers = axum::http::HeaderMap::new();
        headers.insert("x-real-ip", "10.0.0.1".parse().unwrap());
        let (_, ip) = extract_analytics_headers(&headers);
        assert_eq!(ip, Some("10.0.0.1".to_string()));
    }

    #[test]
    fn analytics_headers_forwarded_for_takes_priority() {
        let mut headers = axum::http::HeaderMap::new();
        headers.insert("x-forwarded-for", "1.2.3.4".parse().unwrap());
        headers.insert("x-real-ip", "10.0.0.1".parse().unwrap());
        let (_, ip) = extract_analytics_headers(&headers);
        assert_eq!(ip, Some("1.2.3.4".to_string()));
    }

    // ── extract_single_geoloc ──

    #[test]
    fn geoloc_from_float_object() {
        let mut map = HashMap::new();
        map.insert("lat".to_string(), FieldValue::Float(48.8566));
        map.insert("lng".to_string(), FieldValue::Float(2.3522));
        let val = FieldValue::Object(map);
        let result = extract_single_geoloc(&val);
        assert_eq!(result, Some((48.8566, 2.3522)));
    }

    #[test]
    fn geoloc_from_integer_object() {
        let mut map = HashMap::new();
        map.insert("lat".to_string(), FieldValue::Integer(48));
        map.insert("lng".to_string(), FieldValue::Integer(2));
        let val = FieldValue::Object(map);
        let result = extract_single_geoloc(&val);
        assert_eq!(result, Some((48.0, 2.0)));
    }

    #[test]
    fn geoloc_missing_lat() {
        let mut map = HashMap::new();
        map.insert("lng".to_string(), FieldValue::Float(2.0));
        let val = FieldValue::Object(map);
        assert_eq!(extract_single_geoloc(&val), None);
    }

    #[test]
    fn geoloc_missing_lng() {
        let mut map = HashMap::new();
        map.insert("lat".to_string(), FieldValue::Float(48.0));
        let val = FieldValue::Object(map);
        assert_eq!(extract_single_geoloc(&val), None);
    }

    #[test]
    fn geoloc_wrong_type() {
        let val = FieldValue::Text("not a geoloc".into());
        assert_eq!(extract_single_geoloc(&val), None);
    }

    #[test]
    fn geoloc_string_lat_returns_none() {
        let mut map = HashMap::new();
        map.insert("lat".to_string(), FieldValue::Text("48.0".into()));
        map.insert("lng".to_string(), FieldValue::Float(2.0));
        let val = FieldValue::Object(map);
        assert_eq!(extract_single_geoloc(&val), None);
    }

    // ── extract_all_geolocs ──

    #[test]
    fn all_geolocs_none() {
        assert!(extract_all_geolocs(None).is_empty());
    }

    #[test]
    fn all_geolocs_single_object() {
        let mut map = HashMap::new();
        map.insert("lat".to_string(), FieldValue::Float(48.0));
        map.insert("lng".to_string(), FieldValue::Float(2.0));
        let val = FieldValue::Object(map);
        let result = extract_all_geolocs(Some(&val));
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], (48.0, 2.0));
    }

    #[test]
    fn all_geolocs_array() {
        let mut m1 = HashMap::new();
        m1.insert("lat".to_string(), FieldValue::Float(48.0));
        m1.insert("lng".to_string(), FieldValue::Float(2.0));
        let mut m2 = HashMap::new();
        m2.insert("lat".to_string(), FieldValue::Float(40.7));
        m2.insert("lng".to_string(), FieldValue::Float(-74.0));
        let val = FieldValue::Array(vec![FieldValue::Object(m1), FieldValue::Object(m2)]);
        let result = extract_all_geolocs(Some(&val));
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn all_geolocs_non_object_value() {
        let val = FieldValue::Text("nope".into());
        assert!(extract_all_geolocs(Some(&val)).is_empty());
    }

    // ── merge_secured_filters ──

    #[test]
    fn merge_secured_filters_adds_to_empty() {
        let mut req = SearchRequest::default();
        let restrictions = crate::auth::SecuredKeyRestrictions {
            filters: Some("brand:Nike".to_string()),
            ..Default::default()
        };
        merge_secured_filters(&mut req, &restrictions);
        assert_eq!(req.filters, Some("brand:Nike".to_string()));
    }

    #[test]
    fn merge_secured_filters_combines_with_existing() {
        let mut req = SearchRequest::default();
        req.filters = Some("color:Red".to_string());
        let restrictions = crate::auth::SecuredKeyRestrictions {
            filters: Some("brand:Nike".to_string()),
            ..Default::default()
        };
        merge_secured_filters(&mut req, &restrictions);
        assert_eq!(
            req.filters,
            Some("(color:Red) AND (brand:Nike)".to_string())
        );
    }

    #[test]
    fn merge_secured_filters_no_filters() {
        let mut req = SearchRequest::default();
        let restrictions = crate::auth::SecuredKeyRestrictions::default();
        merge_secured_filters(&mut req, &restrictions);
        assert!(req.filters.is_none());
    }

    #[test]
    fn merge_secured_filters_caps_hits_per_page() {
        let mut req = SearchRequest::default();
        req.hits_per_page = Some(100);
        let restrictions = crate::auth::SecuredKeyRestrictions {
            hits_per_page: Some(20),
            ..Default::default()
        };
        merge_secured_filters(&mut req, &restrictions);
        assert_eq!(req.hits_per_page, Some(20));
    }

    #[test]
    fn merge_secured_filters_no_cap_when_lower() {
        let mut req = SearchRequest::default();
        req.hits_per_page = Some(10);
        let restrictions = crate::auth::SecuredKeyRestrictions {
            hits_per_page: Some(20),
            ..Default::default()
        };
        merge_secured_filters(&mut req, &restrictions);
        assert_eq!(req.hits_per_page, Some(10));
    }

    #[test]
    fn merge_secured_filters_applies_when_none() {
        let mut req = SearchRequest::default();
        assert!(req.hits_per_page.is_none()); // precondition
        let restrictions = crate::auth::SecuredKeyRestrictions {
            hits_per_page: Some(20),
            ..Default::default()
        };
        merge_secured_filters(&mut req, &restrictions);
        assert_eq!(req.hits_per_page, Some(20));
    }

    #[test]
    fn merge_secured_filters_empty_filter_string() {
        let mut req = SearchRequest::default();
        req.filters = Some("color:Red".to_string());
        let restrictions = crate::auth::SecuredKeyRestrictions {
            filters: Some("".to_string()),
            ..Default::default()
        };
        merge_secured_filters(&mut req, &restrictions);
        // Empty string still gets combined — caller should avoid passing empty
        assert_eq!(req.filters, Some("(color:Red) AND ()".to_string()));
    }

    #[test]
    fn merge_secured_filters_both_filters_and_hpp() {
        let mut req = SearchRequest::default();
        req.hits_per_page = Some(100);
        let restrictions = crate::auth::SecuredKeyRestrictions {
            filters: Some("brand:Nike".to_string()),
            hits_per_page: Some(20),
            ..Default::default()
        };
        merge_secured_filters(&mut req, &restrictions);
        assert_eq!(req.filters, Some("brand:Nike".to_string()));
        assert_eq!(req.hits_per_page, Some(20));
    }

    // ── resolve_search_mode ──

    #[test]
    fn test_resolve_search_mode_query_overrides_settings() {
        use flapjack::index::settings::{IndexMode, IndexSettings};
        let query_mode = Some(IndexMode::NeuralSearch);
        let settings = IndexSettings {
            mode: Some(IndexMode::KeywordSearch),
            ..Default::default()
        };
        let result = resolve_search_mode(&query_mode, &settings);
        assert_eq!(result, IndexMode::NeuralSearch);
    }

    #[test]
    fn test_resolve_search_mode_falls_back_to_settings() {
        use flapjack::index::settings::{IndexMode, IndexSettings};
        let query_mode = None;
        let settings = IndexSettings {
            mode: Some(IndexMode::NeuralSearch),
            ..Default::default()
        };
        let result = resolve_search_mode(&query_mode, &settings);
        assert_eq!(result, IndexMode::NeuralSearch);
    }

    #[test]
    fn test_resolve_search_mode_both_none_is_keyword() {
        use flapjack::index::settings::{IndexMode, IndexSettings};
        let query_mode = None;
        let settings = IndexSettings::default();
        let result = resolve_search_mode(&query_mode, &settings);
        assert_eq!(result, IndexMode::KeywordSearch);
    }

    #[test]
    fn test_resolve_search_mode_query_keyword_overrides_settings_neural() {
        // A per-query KeywordSearch must override an index-level NeuralSearch setting.
        // Critical: users must be able to opt out of hybrid search on a per-query basis.
        use flapjack::index::settings::{IndexMode, IndexSettings};
        let query_mode = Some(IndexMode::KeywordSearch);
        let settings = IndexSettings {
            mode: Some(IndexMode::NeuralSearch),
            ..Default::default()
        };
        let result = resolve_search_mode(&query_mode, &settings);
        assert_eq!(result, IndexMode::KeywordSearch);
    }

    #[test]
    fn test_resolve_search_mode_settings_keyword_propagates() {
        // Explicit KeywordSearch in settings should propagate (not be shadowed by default).
        use flapjack::index::settings::{IndexMode, IndexSettings};
        let query_mode = None;
        let settings = IndexSettings {
            mode: Some(IndexMode::KeywordSearch),
            ..Default::default()
        };
        let result = resolve_search_mode(&query_mode, &settings);
        assert_eq!(result, IndexMode::KeywordSearch);
    }

    // ── A6: apply_query_overrides ──

    #[test]
    fn apply_overrides_typo_tolerance() {
        let mut req = SearchRequest::default();
        let overrides = QueryOverrides {
            typo_tolerance: Some(json!(false)),
            ..Default::default()
        };
        apply_query_overrides(&mut req, &overrides);
        assert_eq!(req.typo_tolerance, Some(json!(false)));
    }

    #[test]
    fn apply_overrides_enable_synonyms() {
        let mut req = SearchRequest::default();
        let overrides = QueryOverrides {
            enable_synonyms: Some(false),
            ..Default::default()
        };
        apply_query_overrides(&mut req, &overrides);
        assert_eq!(req.enable_synonyms, Some(false));
    }

    #[test]
    fn apply_overrides_enable_rules() {
        let mut req = SearchRequest::default();
        let overrides = QueryOverrides {
            enable_rules: Some(false),
            ..Default::default()
        };
        apply_query_overrides(&mut req, &overrides);
        assert_eq!(req.enable_rules, Some(false));
    }

    #[test]
    fn apply_overrides_rule_contexts() {
        let mut req = SearchRequest::default();
        let overrides = QueryOverrides {
            rule_contexts: Some(vec!["sale".to_string()]),
            ..Default::default()
        };
        apply_query_overrides(&mut req, &overrides);
        assert_eq!(req.rule_contexts, Some(vec!["sale".to_string()]));
    }

    #[test]
    fn apply_overrides_filters() {
        let mut req = SearchRequest::default();
        let overrides = QueryOverrides {
            filters: Some("brand:Nike".to_string()),
            ..Default::default()
        };
        apply_query_overrides(&mut req, &overrides);
        assert_eq!(req.filters, Some("brand:Nike".to_string()));
    }

    #[test]
    fn apply_overrides_optional_filters() {
        let mut req = SearchRequest::default();
        let overrides = QueryOverrides {
            optional_filters: Some(vec!["brand:Nike".to_string()]),
            ..Default::default()
        };
        apply_query_overrides(&mut req, &overrides);
        assert_eq!(req.optional_filters, Some(json!(["brand:Nike"])));
    }

    #[test]
    fn apply_overrides_remove_words_if_no_results() {
        let mut req = SearchRequest::default();
        let overrides = QueryOverrides {
            remove_words_if_no_results: Some("lastWords".to_string()),
            ..Default::default()
        };
        apply_query_overrides(&mut req, &overrides);
        assert_eq!(
            req.remove_words_if_no_results,
            Some("lastWords".to_string())
        );
    }

    #[test]
    fn apply_overrides_skips_none_fields() {
        let mut req = SearchRequest::default();
        req.filters = Some("existing".to_string());
        req.enable_rules = Some(true);
        req.optional_filters = Some(json!(["old"]));
        let overrides = QueryOverrides::default();

        apply_query_overrides(&mut req, &overrides);

        assert_eq!(req.filters, Some("existing".to_string()));
        assert_eq!(req.enable_rules, Some(true));
        assert_eq!(req.optional_filters, Some(json!(["old"])));
    }

    #[test]
    fn apply_overrides_does_not_clobber_existing() {
        let mut req = SearchRequest::default();
        req.filters = Some("existing".to_string());
        let overrides = QueryOverrides {
            enable_synonyms: Some(false),
            ..Default::default()
        };

        apply_query_overrides(&mut req, &overrides);

        assert_eq!(req.enable_synonyms, Some(false));
        assert_eq!(req.filters, Some("existing".to_string()));
    }

    #[test]
    fn apply_overrides_skips_index_level_fields() {
        let mut req = SearchRequest::default();
        req.filters = Some("existing".to_string());
        req.enable_synonyms = Some(true);
        let overrides = QueryOverrides {
            custom_ranking: Some(vec!["desc(popularity)".to_string()]),
            attribute_weights: Some(std::iter::once(("title".to_string(), 10.0)).collect()),
            ..Default::default()
        };

        apply_query_overrides(&mut req, &overrides);

        assert_eq!(
            req.filters,
            Some("existing".to_string()),
            "index-level overrides must not mutate query request fields"
        );
        assert_eq!(req.enable_synonyms, Some(true));
    }

    // ── A6: assignment_method_str ──

    #[test]
    fn assignment_method_to_string_user_token() {
        assert_eq!(
            assignment_method_str(&AssignmentMethod::UserToken),
            "user_token"
        );
    }

    #[test]
    fn assignment_method_to_string_query_id() {
        assert_eq!(
            assignment_method_str(&AssignmentMethod::QueryId),
            "query_id"
        );
    }

    #[test]
    fn assignment_method_to_string_session_id() {
        assert_eq!(
            assignment_method_str(&AssignmentMethod::SessionId),
            "session_id"
        );
    }

    fn make_doc(id: &str, title: &str) -> Document {
        let mut fields = HashMap::new();
        fields.insert("title".to_string(), FieldValue::Text(title.to_string()));
        Document {
            id: id.to_string(),
            fields,
        }
    }

    fn mode_a_experiment(id: &str, index_name: &str) -> Experiment {
        Experiment {
            id: id.to_string(),
            name: format!("mode-a-{index_name}"),
            index_name: index_name.to_string(),
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
            created_at: chrono::Utc::now().timestamp_millis(),
            started_at: None,
            ended_at: None,
            minimum_days: 14,
            winsorization_cap: None,
            conclusion: None,
        }
    }

    fn mode_b_experiment(id: &str, index_name: &str, variant_index_name: &str) -> Experiment {
        Experiment {
            id: id.to_string(),
            name: format!("mode-b-{index_name}"),
            index_name: index_name.to_string(),
            status: ExperimentStatus::Draft,
            traffic_split: 0.5,
            control: ExperimentArm {
                name: "control".to_string(),
                query_overrides: None,
                index_name: None,
            },
            variant: ExperimentArm {
                name: "variant".to_string(),
                query_overrides: None,
                index_name: Some(variant_index_name.to_string()),
            },
            primary_metric: PrimaryMetric::Ctr,
            created_at: chrono::Utc::now().timestamp_millis(),
            started_at: None,
            ended_at: None,
            minimum_days: 14,
            winsorization_cap: None,
            conclusion: None,
        }
    }

    async fn make_search_experiment_state(tmp: &TempDir) -> Arc<AppState> {
        let experiment_store = Arc::new(ExperimentStore::new(tmp.path()).unwrap());
        let state = Arc::new(AppState {
            manager: IndexManager::new(tmp.path()),
            key_store: None,
            replication_manager: None,
            ssl_manager: None,
            analytics_engine: None,
            experiment_store: Some(experiment_store.clone()),
            metrics_state: Some(MetricsState::new()),
            usage_counters: Arc::new(dashmap::DashMap::new()),
            paused_indexes: crate::pause_registry::PausedIndexes::new(),
            start_time: std::time::Instant::now(),
            #[cfg(feature = "vector-search")]
            embedder_store: Arc::new(crate::embedder_store::EmbedderStore::new()),
        });

        state.manager.create_tenant("products").unwrap();
        state
            .manager
            .add_documents_sync(
                "products",
                vec![
                    make_doc("p1", "nike running shoe"),
                    make_doc("p2", "adidas trail shoe"),
                ],
            )
            .await
            .unwrap();

        state.manager.create_tenant("products_mode_b").unwrap();
        state
            .manager
            .add_documents_sync(
                "products_mode_b",
                vec![make_doc("m1", "control index document")],
            )
            .await
            .unwrap();

        state
            .manager
            .create_tenant("products_mode_b_variant")
            .unwrap();
        state
            .manager
            .add_documents_sync(
                "products_mode_b_variant",
                vec![make_doc("mv1", "variant index document")],
            )
            .await
            .unwrap();

        state
            .manager
            .create_tenant("products_no_experiment")
            .unwrap();
        state
            .manager
            .add_documents_sync(
                "products_no_experiment",
                vec![make_doc("n1", "plain index document")],
            )
            .await
            .unwrap();

        experiment_store
            .create(mode_a_experiment("exp-mode-a", "products"))
            .unwrap();
        experiment_store.start("exp-mode-a").unwrap();

        experiment_store
            .create(mode_b_experiment(
                "exp-mode-b",
                "products_mode_b",
                "products_mode_b_variant",
            ))
            .unwrap();
        experiment_store.start("exp-mode-b").unwrap();

        state
    }

    fn search_router(state: Arc<AppState>) -> Router {
        Router::new()
            .route("/1/indexes/:indexName/query", post(search))
            .with_state(state)
    }

    async fn post_search(
        app: &Router,
        index_name: &str,
        body: Value,
        user_token: Option<&str>,
    ) -> axum::http::Response<Body> {
        let mut builder = Request::builder()
            .method(Method::POST)
            .uri(format!("/1/indexes/{index_name}/query"))
            .header("content-type", "application/json");
        if let Some(token) = user_token {
            builder = builder.header("x-algolia-usertoken", token);
        }
        app.clone()
            .oneshot(builder.body(Body::from(body.to_string())).unwrap())
            .await
            .unwrap()
    }

    async fn post_batch_search(app: &Router, body: Value) -> axum::http::Response<Body> {
        app.clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/1/indexes/*/queries")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap()
    }

    async fn body_json(resp: axum::http::Response<Body>) -> Value {
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    fn find_user_token_for_arm(experiment: &Experiment, target_arm: &str) -> String {
        for i in 0..100_000 {
            let candidate = format!("tok-{i}");
            let assignment = assignment::assign_variant(experiment, Some(&candidate), None, "qid");
            if assignment.arm == target_arm {
                return candidate;
            }
        }
        panic!("failed to find user token for target arm: {target_arm}");
    }

    // ── A6 integration tests: search + experiments ──

    #[tokio::test]
    async fn search_with_active_experiment_is_annotated() {
        let tmp = TempDir::new().unwrap();
        let state = make_search_experiment_state(&tmp).await;
        let app = search_router(state);

        let resp = post_search(&app, "products", json!({ "query": "shoe" }), Some("user-a")).await;
        assert_eq!(resp.status(), StatusCode::OK);
        let body = body_json(resp).await;
        assert_eq!(
            body["abTestID"], "exp-mode-a",
            "abTestID must match the experiment ID"
        );
        let variant_id = body["abTestVariantID"].as_str().unwrap();
        assert!(
            variant_id == "control" || variant_id == "variant",
            "abTestVariantID must be 'control' or 'variant', got: {variant_id}"
        );
        assert_eq!(
            body["index"], "products",
            "response index must be the originally-requested index"
        );
        assert!(
            body.get("indexUsed").is_none(),
            "Mode A should not set indexUsed"
        );
    }

    #[tokio::test]
    async fn search_without_active_experiment_has_no_ab_fields() {
        let tmp = TempDir::new().unwrap();
        let state = make_search_experiment_state(&tmp).await;
        let app = search_router(state);

        let resp = post_search(
            &app,
            "products_no_experiment",
            json!({ "query": "plain" }),
            Some("user-a"),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::OK);
        let body = body_json(resp).await;
        assert!(body.get("abTestID").is_none());
        assert!(body.get("abTestVariantID").is_none());
    }

    #[tokio::test]
    async fn mode_b_variant_reroutes_shows_index_used() {
        let tmp = TempDir::new().unwrap();
        let state = make_search_experiment_state(&tmp).await;
        let experiment = state
            .experiment_store
            .as_ref()
            .unwrap()
            .get("exp-mode-b")
            .unwrap();
        let variant_token = find_user_token_for_arm(&experiment, "variant");
        let app = search_router(state);

        let resp = post_search(
            &app,
            "products_mode_b",
            json!({ "query": "document" }),
            Some(&variant_token),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::OK);
        let body = body_json(resp).await;
        assert_eq!(body["index"], "products_mode_b");
        assert_eq!(body["indexUsed"], "products_mode_b_variant");
        assert_eq!(body["hits"][0]["objectID"], "mv1");
    }

    #[tokio::test]
    async fn mode_b_control_stays_on_original_index() {
        let tmp = TempDir::new().unwrap();
        let state = make_search_experiment_state(&tmp).await;
        let experiment = state
            .experiment_store
            .as_ref()
            .unwrap()
            .get("exp-mode-b")
            .unwrap();
        let control_token = find_user_token_for_arm(&experiment, "control");
        let app = search_router(state);

        let resp = post_search(
            &app,
            "products_mode_b",
            json!({ "query": "document" }),
            Some(&control_token),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::OK);
        let body = body_json(resp).await;
        assert_eq!(
            body["index"], "products_mode_b",
            "control arm must stay on original index"
        );
        assert!(
            body.get("indexUsed").is_none(),
            "control arm must not set indexUsed"
        );
        assert_eq!(
            body["abTestID"], "exp-mode-b",
            "abTestID must be present for control arm"
        );
        assert_eq!(
            body["abTestVariantID"], "control",
            "control arm must report variant_id as 'control'"
        );
        assert_eq!(
            body["hits"][0]["objectID"], "m1",
            "control arm must serve documents from original index"
        );
    }

    #[tokio::test]
    async fn query_id_fallback_still_annotates_response() {
        let tmp = TempDir::new().unwrap();
        let state = make_search_experiment_state(&tmp).await;
        let app = search_router(state);

        let resp = post_search(&app, "products", json!({ "query": "shoe" }), None).await;
        assert_eq!(resp.status(), StatusCode::OK);
        let body = body_json(resp).await;
        assert_eq!(
            body["abTestID"], "exp-mode-a",
            "abTestID must match experiment ID even without user token"
        );
        let variant_id = body["abTestVariantID"].as_str().unwrap();
        assert!(
            variant_id == "control" || variant_id == "variant",
            "abTestVariantID must be 'control' or 'variant', got: {variant_id}"
        );
    }

    #[tokio::test]
    async fn query_id_fallback_without_click_analytics_varies_across_queries() {
        let tmp = TempDir::new().unwrap();
        let state = make_search_experiment_state(&tmp).await;
        let app = search_router(state);
        let mut seen_arms = std::collections::HashSet::new();

        for _ in 0..32 {
            let resp = post_search(
                &app,
                "products",
                json!({ "query": "shoe", "clickAnalytics": false }),
                None,
            )
            .await;
            assert_eq!(resp.status(), StatusCode::OK);
            let body = body_json(resp).await;
            assert!(body.get("queryID").is_none());
            let variant = body["abTestVariantID"].as_str().unwrap().to_string();
            seen_arms.insert(variant);
        }

        assert!(
            seen_arms.len() > 1,
            "query-id fallback should vary assignment across independent queries"
        );
    }

    #[tokio::test]
    async fn batch_search_with_active_experiment_is_annotated() {
        let tmp = TempDir::new().unwrap();
        let state = make_search_experiment_state(&tmp).await;
        let app = Router::new()
            .route("/1/indexes/:indexName/queries", post(batch_search))
            .with_state(state);

        let resp = post_batch_search(
            &app,
            json!({
                "requests": [
                    { "indexName": "products", "query": "shoe" }
                ]
            }),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::OK);
        let body = body_json(resp).await;
        assert_eq!(
            body["results"][0]["abTestID"], "exp-mode-a",
            "batch abTestID must match experiment ID"
        );
        let variant_id = body["results"][0]["abTestVariantID"].as_str().unwrap();
        assert!(
            variant_id == "control" || variant_id == "variant",
            "batch abTestVariantID must be 'control' or 'variant', got: {variant_id}"
        );
    }

    #[tokio::test]
    async fn batch_search_multiple_active_queries_each_annotated() {
        let tmp = TempDir::new().unwrap();
        let state = make_search_experiment_state(&tmp).await;
        let app = Router::new()
            .route("/1/indexes/:indexName/queries", post(batch_search))
            .with_state(state);

        let resp = post_batch_search(
            &app,
            json!({
                "requests": [
                    { "indexName": "products", "query": "shoe" },
                    { "indexName": "products", "query": "running" }
                ]
            }),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::OK);
        let body = body_json(resp).await;
        let results = body["results"]
            .as_array()
            .expect("batch response must contain a results array");
        assert_eq!(
            results.len(),
            2,
            "batch response must include one result object per request"
        );

        for result in results {
            assert_eq!(
                result["abTestID"], "exp-mode-a",
                "every active-experiment batch result must include abTestID"
            );
            let variant_id = result["abTestVariantID"]
                .as_str()
                .expect("abTestVariantID must be a string");
            assert!(
                variant_id == "control" || variant_id == "variant",
                "abTestVariantID must be 'control' or 'variant', got: {variant_id}"
            );
        }
    }

    #[test]
    fn search_event_includes_experiment_fields() {
        let req = SearchRequest {
            query: "shoe".to_string(),
            user_token: Some("user-a".to_string()),
            user_ip: Some("10.0.0.1".to_string()),
            analytics_tags: Some(vec!["ab".to_string()]),
            facets: Some(vec!["brand".to_string()]),
            ..Default::default()
        };
        let event = build_search_event(
            &req,
            Some("qid123".to_string()),
            "products".to_string(),
            4,
            8,
            0,
            20,
            Some(&ExperimentContext {
                experiment_id: "exp-123".to_string(),
                variant_id: "variant".to_string(),
                assignment_method: "user_token".to_string(),
            }),
        );

        assert_eq!(event.experiment_id.as_deref(), Some("exp-123"));
        assert_eq!(event.variant_id.as_deref(), Some("variant"));
        assert_eq!(event.assignment_method.as_deref(), Some("user_token"));
        assert_eq!(event.index_name, "products");
    }

    #[test]
    fn search_event_without_experiment_has_none_fields() {
        let req = SearchRequest {
            query: "shoe".to_string(),
            ..Default::default()
        };
        let event = build_search_event(
            &req,
            Some("qid456".to_string()),
            "products".to_string(),
            2,
            5,
            0,
            20,
            None,
        );

        assert!(
            event.experiment_id.is_none(),
            "no experiment should produce None experiment_id"
        );
        assert!(
            event.variant_id.is_none(),
            "no experiment should produce None variant_id"
        );
        assert!(
            event.assignment_method.is_none(),
            "no experiment should produce None assignment_method"
        );
    }

    // ── Hybrid search integration tests (6.17) ──
    // Behind vector-search feature flag. These exercise the full search_single path.

    #[cfg(feature = "vector-search")]
    mod hybrid_search_tests {
        use super::*;
        use crate::dto::HybridSearchParams;
        use crate::handlers::metrics::MetricsState;
        use flapjack::index::settings::{IndexMode, IndexSettings};
        use flapjack::types::{Document, FieldValue};
        use flapjack::vector::MetricKind;
        use flapjack::IndexManager;
        use std::collections::HashMap;
        use tempfile::TempDir;

        fn make_test_state(tmp: &TempDir) -> Arc<AppState> {
            Arc::new(AppState {
                manager: IndexManager::new(tmp.path()),
                key_store: None,
                replication_manager: None,
                ssl_manager: None,
                analytics_engine: None,
                experiment_store: None,
                metrics_state: Some(MetricsState::new()),
                usage_counters: Arc::new(dashmap::DashMap::new()),
                paused_indexes: crate::pause_registry::PausedIndexes::new(),
                start_time: std::time::Instant::now(),
                embedder_store: Arc::new(crate::embedder_store::EmbedderStore::new()),
            })
        }

        fn make_doc(id: &str, text: &str) -> Document {
            let mut fields = HashMap::new();
            fields.insert("title".to_string(), FieldValue::Text(text.to_string()));
            Document {
                id: id.to_string(),
                fields,
            }
        }

        /// Save settings JSON to the expected path for a tenant.
        fn save_settings(state: &Arc<AppState>, index_name: &str, settings: &IndexSettings) {
            let dir = state.manager.base_path.join(index_name);
            std::fs::create_dir_all(&dir).unwrap();
            settings.save(dir.join("settings.json")).unwrap();
        }

        /// Set up a VectorIndex with 3D cosine vectors for testing.
        /// Returns the query vector to use for search.
        fn setup_vector_index(state: &Arc<AppState>, index_name: &str) -> Vec<f32> {
            let mut vi = flapjack::vector::index::VectorIndex::new(3, MetricKind::Cos).unwrap();
            // doc1: very close to query [1.0, 0.0, 0.0]
            vi.add("doc1", &[0.99, 0.1, 0.0]).unwrap();
            // doc2: moderate distance
            vi.add("doc2", &[0.5, 0.5, 0.0]).unwrap();
            // doc3: very far from query
            vi.add("doc3", &[0.0, 0.1, 0.99]).unwrap();
            // doc4: close to query but different keyword content
            vi.add("doc4", &[0.95, 0.15, 0.0]).unwrap();
            state.manager.set_vector_index(index_name, vi);
            vec![1.0, 0.0, 0.0]
        }

        /// Settings with a UserProvided embedder (no HTTP calls needed).
        fn settings_with_embedder() -> IndexSettings {
            let mut embedders = HashMap::new();
            embedders.insert(
                "default".to_string(),
                serde_json::json!({
                    "source": "userProvided",
                    "dimensions": 3
                }),
            );
            IndexSettings {
                embedders: Some(embedders),
                ..Default::default()
            }
        }

        /// Pre-populate the query cache so the embedder is never actually called.
        fn cache_query_vector(
            state: &Arc<AppState>,
            embedder_name: &str,
            query: &str,
            vector: Vec<f32>,
        ) {
            state
                .embedder_store
                .query_cache
                .insert(embedder_name, query, vector);
        }

        #[tokio::test]
        async fn test_hybrid_search_pure_bm25() {
            let tmp = TempDir::new().unwrap();
            let state = make_test_state(&tmp);
            let idx = "test_pure_bm25";
            state.manager.create_tenant(idx).unwrap();

            let docs = vec![
                make_doc("doc1", "machine learning algorithms"),
                make_doc("doc2", "deep learning neural networks"),
                make_doc("doc3", "cooking recipes for beginners"),
            ];
            state.manager.add_documents_sync(idx, docs).await.unwrap();

            let settings = settings_with_embedder();
            save_settings(&state, idx, &settings);

            let query_vec = setup_vector_index(&state, idx);
            cache_query_vector(&state, "default", "learning", query_vec);

            // Search with hybrid semanticRatio=0.0 → pure BM25
            let req = SearchRequest {
                query: "learning".to_string(),
                hybrid: Some(HybridSearchParams {
                    semantic_ratio: 0.0,
                    embedder: "default".to_string(),
                }),
                ..Default::default()
            };
            let result = search_single(State(state.clone()), idx.to_string(), req)
                .await
                .unwrap();
            let hits = result.0["hits"].as_array().unwrap();

            // With ratio=0.0, only BM25 matters. doc1 and doc2 contain "learning".
            assert!(hits.len() >= 2, "Expected at least 2 BM25 hits");
            let ids: Vec<&str> = hits
                .iter()
                .map(|h| h["objectID"].as_str().unwrap())
                .collect();
            assert!(ids.contains(&"doc1"));
            assert!(ids.contains(&"doc2"));
            // doc3 ("cooking recipes") should NOT appear for query "learning"
            assert!(!ids.contains(&"doc3"));
        }

        #[tokio::test]
        async fn test_hybrid_search_pure_vector() {
            let tmp = TempDir::new().unwrap();
            let state = make_test_state(&tmp);
            let idx = "test_pure_vector";
            state.manager.create_tenant(idx).unwrap();

            let docs = vec![
                make_doc("doc1", "machine learning algorithms"),
                make_doc("doc2", "deep learning neural networks"),
                make_doc("doc3", "cooking recipes for beginners"),
                make_doc("doc4", "artificial intelligence research"),
            ];
            state.manager.add_documents_sync(idx, docs).await.unwrap();

            let settings = settings_with_embedder();
            save_settings(&state, idx, &settings);

            let query_vec = setup_vector_index(&state, idx);
            cache_query_vector(&state, "default", "learning", query_vec);

            // Search with hybrid semanticRatio=1.0 → pure vector
            let req = SearchRequest {
                query: "learning".to_string(),
                hybrid: Some(HybridSearchParams {
                    semantic_ratio: 1.0,
                    embedder: "default".to_string(),
                }),
                ..Default::default()
            };
            let result = search_single(State(state.clone()), idx.to_string(), req)
                .await
                .unwrap();
            let hits = result.0["hits"].as_array().unwrap();

            // With ratio=1.0, order should follow vector similarity.
            // Vector distances from [1,0,0]: doc1 closest, doc4 next, doc2, doc3 farthest.
            assert!(
                hits.len() >= 3,
                "Expected at least 3 hits from vector search"
            );
            let first_id = hits[0]["objectID"].as_str().unwrap();
            assert_eq!(first_id, "doc1", "doc1 should be first (closest vector)");

            // doc4 should appear even though "learning" has no keyword match in "artificial intelligence research"
            let ids: Vec<&str> = hits
                .iter()
                .map(|h| h["objectID"].as_str().unwrap())
                .collect();
            assert!(
                ids.contains(&"doc4"),
                "doc4 should appear via vector search even without keyword match"
            );
        }

        #[tokio::test]
        async fn test_hybrid_search_blended() {
            let tmp = TempDir::new().unwrap();
            let state = make_test_state(&tmp);
            let idx = "test_blended";
            state.manager.create_tenant(idx).unwrap();

            let docs = vec![
                make_doc("doc1", "machine learning algorithms"),
                make_doc("doc2", "deep learning neural networks"),
                make_doc("doc3", "cooking recipes for beginners"),
                make_doc("doc4", "artificial intelligence research"),
            ];
            state.manager.add_documents_sync(idx, docs).await.unwrap();

            let settings = settings_with_embedder();
            save_settings(&state, idx, &settings);

            let query_vec = setup_vector_index(&state, idx);
            cache_query_vector(&state, "default", "learning", query_vec);

            // Pure BM25 search (no hybrid)
            let bm25_req = SearchRequest {
                query: "learning".to_string(),
                ..Default::default()
            };
            let bm25_result = search_single(State(state.clone()), idx.to_string(), bm25_req)
                .await
                .unwrap();
            let bm25_ids: Vec<&str> = bm25_result.0["hits"]
                .as_array()
                .unwrap()
                .iter()
                .map(|h| h["objectID"].as_str().unwrap())
                .collect();

            // Blended search (ratio=0.5)
            let hybrid_req = SearchRequest {
                query: "learning".to_string(),
                hybrid: Some(HybridSearchParams {
                    semantic_ratio: 0.5,
                    embedder: "default".to_string(),
                }),
                ..Default::default()
            };
            let hybrid_result = search_single(State(state.clone()), idx.to_string(), hybrid_req)
                .await
                .unwrap();
            let hybrid_ids: Vec<&str> = hybrid_result.0["hits"]
                .as_array()
                .unwrap()
                .iter()
                .map(|h| h["objectID"].as_str().unwrap())
                .collect();

            // Blended results should include docs from both BM25 and vector.
            // doc4 only appears via vector (no "learning" keyword).
            assert!(
                hybrid_ids.contains(&"doc4"),
                "Blended search should include vector-only doc4, got {:?}",
                hybrid_ids
            );
            // BM25-only search should NOT include doc4
            assert!(
                !bm25_ids.contains(&"doc4"),
                "BM25 search should not include doc4"
            );
        }

        #[tokio::test]
        async fn test_hybrid_search_no_embedder_fallback() {
            let tmp = TempDir::new().unwrap();
            let state = make_test_state(&tmp);
            let idx = "test_no_embedder";
            state.manager.create_tenant(idx).unwrap();

            let docs = vec![
                make_doc("doc1", "machine learning algorithms"),
                make_doc("doc2", "deep learning neural networks"),
            ];
            state.manager.add_documents_sync(idx, docs).await.unwrap();

            // Settings with neuralSearch mode but NO embedders configured
            let settings = IndexSettings {
                mode: Some(IndexMode::NeuralSearch),
                ..Default::default()
            };
            save_settings(&state, idx, &settings);

            let req = SearchRequest {
                query: "learning".to_string(),
                mode: Some(IndexMode::NeuralSearch),
                ..Default::default()
            };
            let result = search_single(State(state.clone()), idx.to_string(), req)
                .await
                .unwrap();
            let response = &result.0;

            // Should still return BM25 results (graceful fallback)
            let hits = response["hits"].as_array().unwrap();
            assert!(!hits.is_empty(), "Should fall back to BM25 results");

            // Should include a warning message about fallback
            assert!(
                response.get("message").is_some(),
                "Response should include 'message' field with fallback warning"
            );
            let msg = response["message"].as_str().unwrap();
            assert!(
                msg.contains("Hybrid search unavailable"),
                "Message should indicate hybrid search unavailable, got: {}",
                msg
            );
        }

        #[tokio::test]
        async fn test_hybrid_search_neural_mode_default_params() {
            let tmp = TempDir::new().unwrap();
            let state = make_test_state(&tmp);
            let idx = "test_neural_mode";
            state.manager.create_tenant(idx).unwrap();

            let docs = vec![
                make_doc("doc1", "machine learning algorithms"),
                make_doc("doc2", "deep learning neural networks"),
                make_doc("doc3", "cooking recipes for beginners"),
                make_doc("doc4", "artificial intelligence research"),
            ];
            state.manager.add_documents_sync(idx, docs).await.unwrap();

            let mut settings = settings_with_embedder();
            settings.mode = Some(IndexMode::NeuralSearch);
            save_settings(&state, idx, &settings);

            let query_vec = setup_vector_index(&state, idx);
            // Cache with embedder "default" and query "learning"
            cache_query_vector(&state, "default", "learning", query_vec);

            // mode=neuralSearch with NO explicit hybrid param →
            // should synthesize hybrid with ratio=0.5, embedder="default"
            let req = SearchRequest {
                query: "learning".to_string(),
                mode: Some(IndexMode::NeuralSearch),
                ..Default::default()
            };
            let result = search_single(State(state.clone()), idx.to_string(), req)
                .await
                .unwrap();
            let hits = result.0["hits"].as_array().unwrap();
            let ids: Vec<&str> = hits
                .iter()
                .map(|h| h["objectID"].as_str().unwrap())
                .collect();

            // neuralSearch mode should trigger hybrid → doc4 should appear via vector
            assert!(
                ids.contains(&"doc4"),
                "neuralSearch mode should trigger hybrid and include vector-only doc4, got {:?}",
                ids
            );
        }

        #[tokio::test]
        async fn test_hybrid_search_empty_vector_index_fallback() {
            let tmp = TempDir::new().unwrap();
            let state = make_test_state(&tmp);
            let idx = "test_empty_vi";
            state.manager.create_tenant(idx).unwrap();

            let docs = vec![
                make_doc("doc1", "machine learning algorithms"),
                make_doc("doc2", "deep learning neural networks"),
            ];
            state.manager.add_documents_sync(idx, docs).await.unwrap();

            let settings = settings_with_embedder();
            save_settings(&state, idx, &settings);

            // Create an empty VectorIndex (no vectors added)
            let vi = flapjack::vector::index::VectorIndex::new(3, MetricKind::Cos).unwrap();
            state.manager.set_vector_index(idx, vi);

            cache_query_vector(&state, "default", "learning", vec![1.0, 0.0, 0.0]);

            let req = SearchRequest {
                query: "learning".to_string(),
                hybrid: Some(HybridSearchParams {
                    semantic_ratio: 0.5,
                    embedder: "default".to_string(),
                }),
                ..Default::default()
            };
            let result = search_single(State(state.clone()), idx.to_string(), req)
                .await
                .unwrap();
            let response = &result.0;

            // Should still return BM25 results
            let hits = response["hits"].as_array().unwrap();
            assert!(!hits.is_empty(), "Should return BM25 results");

            // Should include fallback message about empty vector index
            assert!(
                response.get("message").is_some(),
                "Response should include fallback message for empty vector index"
            );
        }

        #[tokio::test]
        async fn test_hybrid_search_vector_only_docs_fetched() {
            let tmp = TempDir::new().unwrap();
            let state = make_test_state(&tmp);
            let idx = "test_vector_only";
            state.manager.create_tenant(idx).unwrap();

            let docs = vec![
                make_doc("doc1", "machine learning algorithms"),
                make_doc("doc2", "deep learning neural networks"),
                make_doc("doc3", "cooking recipes for beginners"),
                make_doc("doc4", "artificial intelligence research"),
            ];
            state.manager.add_documents_sync(idx, docs).await.unwrap();

            let settings = settings_with_embedder();
            save_settings(&state, idx, &settings);

            let query_vec = setup_vector_index(&state, idx);
            cache_query_vector(&state, "default", "learning", query_vec);

            // ratio=0.7 weights vector heavily → doc4 should surface
            let req = SearchRequest {
                query: "learning".to_string(),
                hybrid: Some(HybridSearchParams {
                    semantic_ratio: 0.7,
                    embedder: "default".to_string(),
                }),
                ..Default::default()
            };
            let result = search_single(State(state.clone()), idx.to_string(), req)
                .await
                .unwrap();
            let hits = result.0["hits"].as_array().unwrap();
            let ids: Vec<&str> = hits
                .iter()
                .map(|h| h["objectID"].as_str().unwrap())
                .collect();

            // doc4 is NOT in BM25 results for "learning" but IS close in vector space.
            // It should be fetched via get_document and included in the fused results.
            assert!(
                ids.contains(&"doc4"),
                "Vector-only doc4 should be fetched and included, got {:?}",
                ids
            );
            // Verify doc4 has its full document data (title field)
            let doc4_hit = hits.iter().find(|h| h["objectID"] == "doc4").unwrap();
            assert_eq!(
                doc4_hit["title"].as_str().unwrap(),
                "artificial intelligence research",
                "Vector-only doc should have its full document fields"
            );
        }

        /// Ranking quality test: hybrid search surfaces semantically relevant docs
        /// that pure BM25 misses entirely, and ranks them above keyword-only partial matches.
        ///
        /// Scenario: user queries "comfortable office chair"
        /// - doc1 ("office chair with lumbar support"): keyword match + vector close → #1
        /// - doc2 ("ergonomic seating for better posture"): NO keyword match, vector closest → surfaced by hybrid
        /// - doc3 ("office desk organizer"): partial keyword ("office") + vector far
        /// - doc4 ("wooden dining chair set"): partial keyword ("chair") + vector far
        ///
        /// BM25 alone cannot find doc2 at all. Hybrid search must:
        /// 1. Surface doc2 (proves semantic recall)
        /// 2. Rank doc2 above doc3 and doc4 (proves ranking quality)
        #[tokio::test]
        async fn test_hybrid_ranking_quality_semantic_beats_keyword() {
            let tmp = TempDir::new().unwrap();
            let state = make_test_state(&tmp);
            let idx = "test_ranking_quality";
            state.manager.create_tenant(idx).unwrap();

            let docs = vec![
                make_doc("doc1", "office chair with lumbar support"),
                make_doc("doc2", "ergonomic seating for better posture"),
                make_doc("doc3", "office desk organizer"),
                make_doc("doc4", "wooden dining chair set"),
            ];
            state.manager.add_documents_sync(idx, docs).await.unwrap();

            let settings = settings_with_embedder();
            save_settings(&state, idx, &settings);

            // Vector space: query [1,0,0] represents "comfortable office seating" concept
            // doc2 is closest (same semantic concept, different words)
            // doc1 is close (semantic + keyword overlap)
            // doc3 and doc4 are far (different concepts)
            let mut vi = flapjack::vector::index::VectorIndex::new(3, MetricKind::Cos).unwrap();
            vi.add("doc1", &[0.92, 0.1, 0.0]).unwrap(); // close: office seating concept
            vi.add("doc2", &[0.98, 0.02, 0.0]).unwrap(); // closest: ergonomic seating
            vi.add("doc3", &[0.1, 0.95, 0.0]).unwrap(); // far: office supplies concept
            vi.add("doc4", &[0.15, 0.1, 0.9]).unwrap(); // far: dining furniture concept
            state.manager.set_vector_index(idx, vi);

            let query_vec = vec![1.0, 0.0, 0.0];
            cache_query_vector(&state, "default", "office chair", query_vec);

            // --- BM25 only (no hybrid) ---
            let bm25_req = SearchRequest {
                query: "office chair".to_string(),
                ..Default::default()
            };
            let bm25_result = search_single(State(state.clone()), idx.to_string(), bm25_req)
                .await
                .unwrap();
            let bm25_hits = bm25_result.0["hits"].as_array().unwrap();
            let bm25_ids: Vec<&str> = bm25_hits
                .iter()
                .map(|h| h["objectID"].as_str().unwrap())
                .collect();

            // BM25 should NOT find doc2 (no keyword overlap with "office chair")
            assert!(
                !bm25_ids.contains(&"doc2"),
                "BM25 should not find 'ergonomic seating' for query 'office chair', got {:?}",
                bm25_ids
            );
            // BM25 should find doc1 (contains both "office" and "chair")
            assert!(
                bm25_ids.contains(&"doc1"),
                "BM25 should find doc1 which contains 'office chair', got {:?}",
                bm25_ids
            );

            // --- Hybrid search (ratio=0.5) ---
            let hybrid_req = SearchRequest {
                query: "office chair".to_string(),
                hybrid: Some(HybridSearchParams {
                    semantic_ratio: 0.5,
                    embedder: "default".to_string(),
                }),
                ..Default::default()
            };
            let hybrid_result = search_single(State(state.clone()), idx.to_string(), hybrid_req)
                .await
                .unwrap();
            let hybrid_hits = hybrid_result.0["hits"].as_array().unwrap();
            let hybrid_ids: Vec<&str> = hybrid_hits
                .iter()
                .map(|h| h["objectID"].as_str().unwrap())
                .collect();

            // 1. Hybrid MUST surface doc2 (semantic recall)
            assert!(
                hybrid_ids.contains(&"doc2"),
                "Hybrid search must surface semantically relevant doc2 ('ergonomic seating'), got {:?}",
                hybrid_ids
            );

            // 2. doc1 should be #1 (strong in both BM25 and vector)
            assert_eq!(
                hybrid_ids[0], "doc1",
                "doc1 should be ranked #1 (keyword + semantic match), got {:?}",
                hybrid_ids
            );

            // 3. doc2 should rank above doc3 and doc4 (semantic relevance > partial keyword)
            let doc2_pos = hybrid_ids.iter().position(|&id| id == "doc2").unwrap();
            if let Some(doc3_pos) = hybrid_ids.iter().position(|&id| id == "doc3") {
                assert!(
                    doc2_pos < doc3_pos,
                    "doc2 (semantic match) should rank above doc3 (partial keyword only): doc2={}, doc3={}",
                    doc2_pos, doc3_pos
                );
            }
            if let Some(doc4_pos) = hybrid_ids.iter().position(|&id| id == "doc4") {
                assert!(
                    doc2_pos < doc4_pos,
                    "doc2 (semantic match) should rank above doc4 (partial keyword only): doc2={}, doc4={}",
                    doc2_pos, doc4_pos
                );
            }
        }

        #[tokio::test]
        async fn test_hybrid_search_algolia_compat_no_hybrid() {
            let tmp = TempDir::new().unwrap();
            let state = make_test_state(&tmp);
            let idx = "test_compat";
            state.manager.create_tenant(idx).unwrap();

            let docs = vec![
                make_doc("doc1", "machine learning algorithms"),
                make_doc("doc2", "deep learning neural networks"),
            ];
            state.manager.add_documents_sync(idx, docs).await.unwrap();

            // Standard Algolia search — no mode, no hybrid
            let req = SearchRequest {
                query: "learning".to_string(),
                ..Default::default()
            };
            let result = search_single(State(state.clone()), idx.to_string(), req)
                .await
                .unwrap();
            let response = &result.0;

            // Verify standard response shape
            assert!(response.get("hits").is_some());
            assert!(response.get("nbHits").is_some());
            assert!(response.get("page").is_some());
            assert!(response.get("hitsPerPage").is_some());
            assert!(response.get("query").is_some());

            // No message field for standard search
            assert!(
                response.get("message").is_none(),
                "Standard search should not have 'message' field"
            );

            let hits = response["hits"].as_array().unwrap();
            assert!(hits.len() >= 2);
        }

        #[tokio::test]
        async fn test_mode_b_hybrid_uses_variant_embedder_settings() {
            let tmp = TempDir::new().unwrap();
            let experiment_store = Arc::new(ExperimentStore::new(tmp.path()).unwrap());
            let state = Arc::new(AppState {
                manager: IndexManager::new(tmp.path()),
                key_store: None,
                replication_manager: None,
                ssl_manager: None,
                analytics_engine: None,
                experiment_store: Some(experiment_store.clone()),
                metrics_state: Some(MetricsState::new()),
                usage_counters: Arc::new(dashmap::DashMap::new()),
                paused_indexes: crate::pause_registry::PausedIndexes::new(),
                start_time: std::time::Instant::now(),
                embedder_store: Arc::new(crate::embedder_store::EmbedderStore::new()),
            });

            let original_index = "mode_b_hybrid_original";
            let variant_index = "mode_b_hybrid_variant";
            state.manager.create_tenant(original_index).unwrap();
            state.manager.create_tenant(variant_index).unwrap();
            state
                .manager
                .add_documents_sync(
                    original_index,
                    vec![make_doc("o1", "original keyword-only content")],
                )
                .await
                .unwrap();
            state
                .manager
                .add_documents_sync(
                    variant_index,
                    vec![make_doc("v1", "semantic-only document text")],
                )
                .await
                .unwrap();

            // Original index has no hybrid mode; variant index enables neural search.
            save_settings(&state, original_index, &IndexSettings::default());
            let mut variant_settings = settings_with_embedder();
            variant_settings.mode = Some(IndexMode::NeuralSearch);
            save_settings(&state, variant_index, &variant_settings);

            let mut vi = flapjack::vector::index::VectorIndex::new(3, MetricKind::Cos).unwrap();
            vi.add("v1", &[0.99, 0.1, 0.0]).unwrap();
            state.manager.set_vector_index(variant_index, vi);

            let query = "needle-query";
            cache_query_vector(&state, "default", query, vec![1.0, 0.0, 0.0]);

            let experiment = mode_b_experiment("exp-mode-b-hybrid", original_index, variant_index);
            experiment_store.create(experiment).unwrap();
            experiment_store.start("exp-mode-b-hybrid").unwrap();
            let running = experiment_store.get("exp-mode-b-hybrid").unwrap();
            let variant_token = find_user_token_for_arm(&running, "variant");

            let req = SearchRequest {
                query: query.to_string(),
                user_token: Some(variant_token),
                ..Default::default()
            };
            let result = search_single(State(state.clone()), original_index.to_string(), req)
                .await
                .unwrap();
            let response = result.0;
            let hits = response["hits"]
                .as_array()
                .expect("response must include hits array");

            assert_eq!(
                response["index"], original_index,
                "response index must remain the requested index"
            );
            assert_eq!(
                response["indexUsed"], variant_index,
                "Mode B variant must expose effective index"
            );
            assert!(
                !hits.is_empty(),
                "hybrid Mode B variant search should return vector hits from variant index"
            );
            assert_eq!(
                hits[0]["objectID"], "v1",
                "vector-ranked result must come from variant index"
            );
        }

        #[tokio::test]
        async fn test_mode_b_hybrid_control_stays_keyword_only() {
            let tmp = TempDir::new().unwrap();
            let experiment_store = Arc::new(ExperimentStore::new(tmp.path()).unwrap());
            let state = Arc::new(AppState {
                manager: IndexManager::new(tmp.path()),
                key_store: None,
                replication_manager: None,
                ssl_manager: None,
                analytics_engine: None,
                experiment_store: Some(experiment_store.clone()),
                metrics_state: Some(MetricsState::new()),
                usage_counters: Arc::new(dashmap::DashMap::new()),
                paused_indexes: crate::pause_registry::PausedIndexes::new(),
                start_time: std::time::Instant::now(),
                embedder_store: Arc::new(crate::embedder_store::EmbedderStore::new()),
            });

            let original_index = "mode_b_hybrid_control_original";
            let variant_index = "mode_b_hybrid_control_variant";
            state.manager.create_tenant(original_index).unwrap();
            state.manager.create_tenant(variant_index).unwrap();
            state
                .manager
                .add_documents_sync(
                    original_index,
                    vec![make_doc("o1", "needle-query control keyword hit")],
                )
                .await
                .unwrap();
            state
                .manager
                .add_documents_sync(
                    variant_index,
                    vec![make_doc("v1", "semantic-only document text")],
                )
                .await
                .unwrap();

            // Only the variant index is neural-enabled.
            save_settings(&state, original_index, &IndexSettings::default());
            let mut variant_settings = settings_with_embedder();
            variant_settings.mode = Some(IndexMode::NeuralSearch);
            save_settings(&state, variant_index, &variant_settings);

            let mut vi = flapjack::vector::index::VectorIndex::new(3, MetricKind::Cos).unwrap();
            vi.add("v1", &[0.99, 0.1, 0.0]).unwrap();
            state.manager.set_vector_index(variant_index, vi);

            let query = "needle-query";
            cache_query_vector(&state, "default", query, vec![1.0, 0.0, 0.0]);

            let experiment = mode_b_experiment(
                "exp-mode-b-hybrid-control",
                original_index,
                variant_index,
            );
            experiment_store.create(experiment).unwrap();
            experiment_store.start("exp-mode-b-hybrid-control").unwrap();
            let running = experiment_store.get("exp-mode-b-hybrid-control").unwrap();
            let control_token = find_user_token_for_arm(&running, "control");

            let req = SearchRequest {
                query: query.to_string(),
                user_token: Some(control_token),
                ..Default::default()
            };
            let result = search_single(State(state.clone()), original_index.to_string(), req)
                .await
                .unwrap();
            let response = result.0;
            let hits = response["hits"]
                .as_array()
                .expect("response must include hits array");

            assert_eq!(
                response["index"], original_index,
                "control arm must keep requested/original index"
            );
            assert!(
                response.get("indexUsed").is_none(),
                "control arm must not expose indexUsed when no reroute happens"
            );
            assert!(
                response.get("message").is_none(),
                "control arm should remain keyword-only and not emit hybrid fallback warnings"
            );
            assert!(!hits.is_empty(), "control arm should still return keyword hits");
            assert_eq!(
                hits[0]["objectID"], "o1",
                "control arm results must come from the original index"
            );
            assert_eq!(
                response["abTestVariantID"], "control",
                "control token must be annotated as control arm"
            );
        }
    }

    /// Integration tests: full write queue → auto-embedding → hybrid search pipeline.
    /// These tests exercise the end-to-end flow where documents are added via
    /// IndexManager, auto-embedded by the write queue, and then searchable via hybrid search.
    #[cfg(feature = "vector-search")]
    mod auto_embed_integration_tests {
        use super::*;
        use crate::dto::HybridSearchParams;
        use crate::handlers::metrics::MetricsState;
        use flapjack::index::settings::IndexSettings;
        use flapjack::types::{Document, FieldValue};
        use flapjack::IndexManager;
        use std::collections::HashMap;
        use tempfile::TempDir;
        use wiremock::matchers::{body_string_contains, method};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        fn make_test_state(tmp: &TempDir) -> Arc<AppState> {
            Arc::new(AppState {
                manager: IndexManager::new(tmp.path()),
                key_store: None,
                replication_manager: None,
                ssl_manager: None,
                analytics_engine: None,
                experiment_store: None,
                metrics_state: Some(MetricsState::new()),
                usage_counters: Arc::new(dashmap::DashMap::new()),
                paused_indexes: crate::pause_registry::PausedIndexes::new(),
                start_time: std::time::Instant::now(),
                embedder_store: Arc::new(crate::embedder_store::EmbedderStore::new()),
            })
        }

        fn make_doc(id: &str, text: &str) -> Document {
            let mut fields = HashMap::new();
            fields.insert("title".to_string(), FieldValue::Text(text.to_string()));
            Document {
                id: id.to_string(),
                fields,
            }
        }

        fn save_settings(state: &Arc<AppState>, index_name: &str, settings: &IndexSettings) {
            let dir = state.manager.base_path.join(index_name);
            std::fs::create_dir_all(&dir).unwrap();
            settings.save(dir.join("settings.json")).unwrap();
        }

        fn rest_embedder_settings(server_uri: &str) -> IndexSettings {
            let mut embedders = HashMap::new();
            embedders.insert(
                "default".to_string(),
                serde_json::json!({
                    "source": "rest",
                    "url": format!("{}/embed", server_uri),
                    "request": {"input": "{{text}}"},
                    "response": {"embedding": "{{embedding}}"},
                    "dimensions": 3
                }),
            );
            IndexSettings {
                embedders: Some(embedders),
                ..Default::default()
            }
        }

        /// Full pipeline: add documents → auto-embed via REST → hybrid search finds them.
        #[tokio::test]
        async fn test_add_documents_and_hybrid_search() {
            let server = MockServer::start().await;

            // Return different vectors based on document content
            Mock::given(method("POST"))
                .and(body_string_contains("machine"))
                .respond_with(
                    ResponseTemplate::new(200)
                        .set_body_json(serde_json::json!({"embedding": [0.9, 0.1, 0.0]})),
                )
                .mount(&server)
                .await;
            Mock::given(method("POST"))
                .and(body_string_contains("neural"))
                .respond_with(
                    ResponseTemplate::new(200)
                        .set_body_json(serde_json::json!({"embedding": [0.7, 0.3, 0.0]})),
                )
                .mount(&server)
                .await;
            Mock::given(method("POST"))
                .and(body_string_contains("cooking"))
                .respond_with(
                    ResponseTemplate::new(200)
                        .set_body_json(serde_json::json!({"embedding": [0.0, 0.1, 0.9]})),
                )
                .mount(&server)
                .await;

            let tmp = TempDir::new().unwrap();
            let state = make_test_state(&tmp);
            let idx = "int_add_hybrid";

            state.manager.create_tenant(idx).unwrap();
            save_settings(&state, idx, &rest_embedder_settings(&server.uri()));

            let docs = vec![
                make_doc("doc1", "machine learning algorithms"),
                make_doc("doc2", "neural networks deep learning"),
                make_doc("doc3", "cooking recipes for beginners"),
            ];
            state.manager.add_documents_sync(idx, docs).await.unwrap();

            // Vector index should be auto-created by write queue
            assert!(
                state.manager.get_vector_index(idx).is_some(),
                "VectorIndex should be auto-created after add_documents_sync"
            );

            // Cache query vector for search (close to doc1's [0.9, 0.1, 0.0])
            state
                .embedder_store
                .query_cache
                .insert("default", "machine", vec![1.0, 0.0, 0.0]);

            let req = SearchRequest {
                query: "machine".to_string(),
                hybrid: Some(HybridSearchParams {
                    semantic_ratio: 0.5,
                    embedder: "default".to_string(),
                }),
                ..Default::default()
            };
            let result = search_single(State(state.clone()), idx.to_string(), req)
                .await
                .unwrap();
            let hits = result.0["hits"].as_array().unwrap();

            assert!(!hits.is_empty(), "hybrid search should return results");
            // doc1 should rank high (BM25 match + closest vector)
            let ids: Vec<&str> = hits
                .iter()
                .map(|h| h["objectID"].as_str().unwrap())
                .collect();
            assert!(ids.contains(&"doc1"), "doc1 should be in results");
        }

        /// User-provided vectors: no embedding API calls, vectors stored directly.
        #[tokio::test]
        async fn test_add_documents_with_vectors_field_and_search() {
            let tmp = TempDir::new().unwrap();
            let state = make_test_state(&tmp);
            let idx = "int_vectors_field";

            state.manager.create_tenant(idx).unwrap();

            let mut embedders = HashMap::new();
            embedders.insert(
                "default".to_string(),
                serde_json::json!({
                    "source": "userProvided",
                    "dimensions": 3
                }),
            );
            let settings = IndexSettings {
                embedders: Some(embedders),
                ..Default::default()
            };
            save_settings(&state, idx, &settings);

            // Documents with _vectors field
            let doc1 = Document {
                id: "doc1".to_string(),
                fields: {
                    let mut f = HashMap::new();
                    f.insert(
                        "title".to_string(),
                        FieldValue::Text("machine learning".to_string()),
                    );
                    let mut vecs = HashMap::new();
                    vecs.insert(
                        "default".to_string(),
                        FieldValue::Array(vec![
                            FieldValue::Float(0.9),
                            FieldValue::Float(0.1),
                            FieldValue::Float(0.0),
                        ]),
                    );
                    f.insert("_vectors".to_string(), FieldValue::Object(vecs));
                    f
                },
            };
            let doc2 = Document {
                id: "doc2".to_string(),
                fields: {
                    let mut f = HashMap::new();
                    f.insert(
                        "title".to_string(),
                        FieldValue::Text("cooking recipes".to_string()),
                    );
                    let mut vecs = HashMap::new();
                    vecs.insert(
                        "default".to_string(),
                        FieldValue::Array(vec![
                            FieldValue::Float(0.0),
                            FieldValue::Float(0.1),
                            FieldValue::Float(0.9),
                        ]),
                    );
                    f.insert("_vectors".to_string(), FieldValue::Object(vecs));
                    f
                },
            };

            state
                .manager
                .add_documents_sync(idx, vec![doc1, doc2])
                .await
                .unwrap();

            // Vector index auto-created from _vectors
            assert!(state.manager.get_vector_index(idx).is_some());

            // Cache query vector close to doc1
            state
                .embedder_store
                .query_cache
                .insert("default", "machine", vec![1.0, 0.0, 0.0]);

            let req = SearchRequest {
                query: "machine".to_string(),
                hybrid: Some(HybridSearchParams {
                    semantic_ratio: 1.0, // Pure vector search
                    embedder: "default".to_string(),
                }),
                ..Default::default()
            };
            let result = search_single(State(state.clone()), idx.to_string(), req)
                .await
                .unwrap();
            let hits = result.0["hits"].as_array().unwrap();

            assert!(hits.len() >= 2, "should return both docs");
            // doc1 should be first (closest to query vector)
            assert_eq!(
                hits[0]["objectID"].as_str().unwrap(),
                "doc1",
                "doc1 should be closest to query vector [1,0,0]"
            );
        }

        /// Delete removes document from vector index — no longer found by hybrid search.
        #[tokio::test]
        async fn test_delete_document_removes_from_hybrid_search() {
            let server = MockServer::start().await;
            Mock::given(method("POST"))
                .respond_with(
                    ResponseTemplate::new(200)
                        .set_body_json(serde_json::json!({"embedding": [0.9, 0.1, 0.0]})),
                )
                .mount(&server)
                .await;

            let tmp = TempDir::new().unwrap();
            let state = make_test_state(&tmp);
            let idx = "int_delete";

            state.manager.create_tenant(idx).unwrap();
            save_settings(&state, idx, &rest_embedder_settings(&server.uri()));

            state
                .manager
                .add_documents_sync(idx, vec![make_doc("doc1", "machine learning")])
                .await
                .unwrap();

            // Verify vector index has the document
            {
                let vi_arc = state.manager.get_vector_index(idx).unwrap();
                let vi = vi_arc.read().unwrap();
                assert_eq!(vi.len(), 1);
            }

            // Delete the document
            state
                .manager
                .delete_documents_sync(idx, vec!["doc1".to_string()])
                .await
                .unwrap();

            // Vector index should be empty after delete
            {
                let vi_arc = state.manager.get_vector_index(idx).unwrap();
                let vi = vi_arc.read().unwrap();
                assert_eq!(vi.len(), 0, "vector index should be empty after delete");
            }

            // Hybrid search should return nothing
            state
                .embedder_store
                .query_cache
                .insert("default", "machine", vec![1.0, 0.0, 0.0]);

            let req = SearchRequest {
                query: "machine".to_string(),
                hybrid: Some(HybridSearchParams {
                    semantic_ratio: 1.0,
                    embedder: "default".to_string(),
                }),
                ..Default::default()
            };
            let result = search_single(State(state.clone()), idx.to_string(), req)
                .await
                .unwrap();
            let hits = result.0["hits"].as_array().unwrap();
            assert!(
                hits.is_empty(),
                "deleted doc should not appear in hybrid search"
            );
        }

        /// Upsert replaces the vector — hybrid search results change accordingly.
        #[tokio::test]
        async fn test_upsert_document_updates_vector() {
            let server = MockServer::start().await;

            // Different vectors based on content
            Mock::given(method("POST"))
                .and(body_string_contains("original"))
                .respond_with(
                    ResponseTemplate::new(200)
                        .set_body_json(serde_json::json!({"embedding": [1.0, 0.0, 0.0]})),
                )
                .mount(&server)
                .await;
            Mock::given(method("POST"))
                .and(body_string_contains("updated"))
                .respond_with(
                    ResponseTemplate::new(200)
                        .set_body_json(serde_json::json!({"embedding": [0.0, 0.0, 1.0]})),
                )
                .mount(&server)
                .await;

            let tmp = TempDir::new().unwrap();
            let state = make_test_state(&tmp);
            let idx = "int_upsert";

            state.manager.create_tenant(idx).unwrap();
            save_settings(&state, idx, &rest_embedder_settings(&server.uri()));

            // Add original document
            state
                .manager
                .add_documents_sync(idx, vec![make_doc("doc1", "original content")])
                .await
                .unwrap();

            // Verify initial vector is close to [1,0,0]
            {
                let vi_arc = state.manager.get_vector_index(idx).unwrap();
                let vi = vi_arc.read().unwrap();
                let results = vi.search(&[1.0, 0.0, 0.0], 1).unwrap();
                assert_eq!(results[0].doc_id, "doc1");
                assert!(
                    results[0].distance < 0.01,
                    "initial vector should be close to [1,0,0]"
                );
            }

            // Upsert with different content (add_documents uses upsert semantics)
            state
                .manager
                .add_documents_sync(idx, vec![make_doc("doc1", "updated content")])
                .await
                .unwrap();

            // Vector should now be close to [0,0,1]
            {
                let vi_arc = state.manager.get_vector_index(idx).unwrap();
                let vi = vi_arc.read().unwrap();
                let results = vi.search(&[0.0, 0.0, 1.0], 1).unwrap();
                assert_eq!(results[0].doc_id, "doc1");
                assert!(
                    results[0].distance < 0.01,
                    "upserted vector should be close to [0,0,1], distance={}",
                    results[0].distance
                );
            }

            // Hybrid search with query vector close to new embedding should find it
            state
                .embedder_store
                .query_cache
                .insert("default", "updated", vec![0.0, 0.0, 1.0]);

            let req = SearchRequest {
                query: "updated".to_string(),
                hybrid: Some(HybridSearchParams {
                    semantic_ratio: 1.0,
                    embedder: "default".to_string(),
                }),
                ..Default::default()
            };
            let result = search_single(State(state.clone()), idx.to_string(), req)
                .await
                .unwrap();
            let hits = result.0["hits"].as_array().unwrap();

            assert!(!hits.is_empty(), "upserted doc should be found");
            assert_eq!(hits[0]["objectID"].as_str().unwrap(), "doc1");
        }
    }

    /// FastEmbed integration tests: local ONNX embedding via fastembed.
    /// These tests use real ONNX model inference (no mock server needed).
    #[cfg(feature = "vector-search-local")]
    mod fastembed_integration_tests {
        use super::*;
        use crate::dto::HybridSearchParams;
        use crate::handlers::metrics::MetricsState;
        use flapjack::index::settings::IndexSettings;
        use flapjack::types::{Document, FieldValue};
        use flapjack::IndexManager;
        use std::collections::HashMap;
        use tempfile::TempDir;

        fn make_test_state(tmp: &TempDir) -> Arc<AppState> {
            Arc::new(AppState {
                manager: IndexManager::new(tmp.path()),
                key_store: None,
                replication_manager: None,
                ssl_manager: None,
                analytics_engine: None,
                experiment_store: None,
                metrics_state: Some(MetricsState::new()),
                usage_counters: Arc::new(dashmap::DashMap::new()),
                paused_indexes: crate::pause_registry::PausedIndexes::new(),
                start_time: std::time::Instant::now(),
                embedder_store: Arc::new(crate::embedder_store::EmbedderStore::new()),
            })
        }

        fn make_doc(id: &str, text: &str) -> Document {
            let mut fields = HashMap::new();
            fields.insert("title".to_string(), FieldValue::Text(text.to_string()));
            Document {
                id: id.to_string(),
                fields,
            }
        }

        fn save_settings(state: &Arc<AppState>, index_name: &str, settings: &IndexSettings) {
            let dir = state.manager.base_path.join(index_name);
            std::fs::create_dir_all(&dir).unwrap();
            settings.save(dir.join("settings.json")).unwrap();
        }

        fn fastembed_settings() -> IndexSettings {
            let mut embedders = HashMap::new();
            embedders.insert(
                "default".to_string(),
                serde_json::json!({ "source": "fastEmbed" }),
            );
            IndexSettings {
                embedders: Some(embedders),
                ..Default::default()
            }
        }

        /// Full pipeline: add documents → auto-embed via fastembed → hybrid search.
        #[tokio::test]
        async fn test_fastembed_hybrid_search_end_to_end() {
            let tmp = TempDir::new().unwrap();
            let state = make_test_state(&tmp);
            let idx = "fe_hybrid_e2e";

            state.manager.create_tenant(idx).unwrap();
            save_settings(&state, idx, &fastembed_settings());

            let docs = vec![
                make_doc("doc1", "machine learning algorithms for data science"),
                make_doc("doc2", "neural networks and deep learning models"),
                make_doc("doc3", "cooking recipes for Italian pasta dishes"),
            ];
            state.manager.add_documents_sync(idx, docs).await.unwrap();

            // Vector index should be auto-created with 384 dimensions (BGESmallENV15)
            let vi = state
                .manager
                .get_vector_index(idx)
                .expect("VectorIndex should be auto-created after fastembed add");
            let vi_read = vi.read().unwrap();
            assert_eq!(vi_read.len(), 3, "all 3 docs should be embedded");
            assert_eq!(
                vi_read.dimensions(),
                384,
                "BGESmallENV15 produces 384-dim vectors"
            );
            drop(vi_read);

            // Hybrid search — fastembed will embed the query at search time
            let req = SearchRequest {
                query: "machine learning".to_string(),
                hybrid: Some(HybridSearchParams {
                    semantic_ratio: 0.5,
                    embedder: "default".to_string(),
                }),
                ..Default::default()
            };
            let result = search_single(State(state.clone()), idx.to_string(), req)
                .await
                .unwrap();
            let hits = result.0["hits"].as_array().unwrap();

            assert!(!hits.is_empty(), "hybrid search should return results");
            // doc1 should rank high (BM25 match + semantic similarity)
            let ids: Vec<&str> = hits
                .iter()
                .map(|h| h["objectID"].as_str().unwrap())
                .collect();
            assert!(ids.contains(&"doc1"), "doc1 should be in results");
        }
    }

    /// Test that `source: "fastEmbed"` is rejected when `vector-search-local` is NOT compiled,
    /// but BM25 still works (documents are indexed in Tantivy).
    #[cfg(not(feature = "vector-search-local"))]
    #[cfg(feature = "vector-search")]
    mod fastembed_rejected_tests {
        use super::*;
        use crate::handlers::metrics::MetricsState;
        use flapjack::index::settings::IndexSettings;
        use flapjack::types::{Document, FieldValue};
        use flapjack::IndexManager;
        use std::collections::HashMap;
        use tempfile::TempDir;

        fn make_test_state(tmp: &TempDir) -> Arc<AppState> {
            Arc::new(AppState {
                manager: IndexManager::new(tmp.path()),
                key_store: None,
                replication_manager: None,
                ssl_manager: None,
                analytics_engine: None,
                experiment_store: None,
                metrics_state: Some(MetricsState::new()),
                usage_counters: Arc::new(dashmap::DashMap::new()),
                paused_indexes: crate::pause_registry::PausedIndexes::new(),
                start_time: std::time::Instant::now(),
                embedder_store: Arc::new(crate::embedder_store::EmbedderStore::new()),
            })
        }

        fn save_settings(state: &Arc<AppState>, index_name: &str, settings: &IndexSettings) {
            let dir = state.manager.base_path.join(index_name);
            std::fs::create_dir_all(&dir).unwrap();
            settings.save(dir.join("settings.json")).unwrap();
        }

        #[tokio::test]
        async fn test_fastembed_config_rejected_without_feature() {
            let tmp = TempDir::new().unwrap();
            let state = make_test_state(&tmp);
            let idx = "fe_rejected";

            state.manager.create_tenant(idx).unwrap();

            let mut embedders = HashMap::new();
            embedders.insert(
                "default".to_string(),
                serde_json::json!({ "source": "fastEmbed" }),
            );
            let settings = IndexSettings {
                embedders: Some(embedders),
                ..Default::default()
            };
            save_settings(&state, idx, &settings);

            // Add a document — embedding should fail, but BM25 indexing should succeed
            let mut fields = HashMap::new();
            fields.insert(
                "title".to_string(),
                FieldValue::Text("test document for rejected embed".to_string()),
            );
            let docs = vec![Document {
                id: "doc1".to_string(),
                fields,
            }];
            state.manager.add_documents_sync(idx, docs).await.unwrap();

            // BM25 search should still work — document was indexed in Tantivy
            let req = SearchRequest {
                query: "test document".to_string(),
                ..Default::default()
            };
            let result = search_single(State(state.clone()), idx.to_string(), req)
                .await
                .unwrap();
            let hits = result.0["hits"].as_array().unwrap();
            assert!(
                !hits.is_empty(),
                "BM25 search should work despite embedding failure"
            );
            assert_eq!(hits[0]["objectID"].as_str().unwrap(), "doc1");

            // Vector index should NOT exist (embedding failed)
            assert!(
                state.manager.get_vector_index(idx).is_none()
                    || state
                        .manager
                        .get_vector_index(idx)
                        .map(|vi| vi.read().unwrap().len() == 0)
                        .unwrap_or(true),
                "vector index should be empty — fastembed not available without feature"
            );
        }
    }
}
