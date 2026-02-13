use axum::{
    extract::{Path, State},
    Json,
};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use flapjack::error::FlapjackError;

use super::AppState;
use crate::dto::SearchRequest;
use flapjack::query::highlighter::{
    extract_query_words, parse_snippet_spec, HighlightValue, Highlighter, SnippetValue,
};
use flapjack::types::{FacetRequest, FieldValue, Sort, SortOrder};

use super::field_value_to_json;

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
    req: SearchRequest,
) -> Result<Json<serde_json::Value>, FlapjackError> {
    // Move CPU-bound search + highlighting + JSON serialization off the async
    // runtime. On t4g.micro (2 vCPUs) this prevents worker-thread starvation
    // when multiple searches run concurrently.
    let enqueue_time = Instant::now();
    tokio::task::spawn_blocking(move || search_single_sync(state, index_name, req, enqueue_time))
        .await
        .map_err(|e| FlapjackError::InvalidQuery(format!("spawn_blocking join error: {}", e)))?
}

fn search_single_sync(
    state: Arc<AppState>,
    index_name: String,
    req: SearchRequest,
    enqueue_time: Instant,
) -> Result<Json<serde_json::Value>, FlapjackError> {
    let queue_wait = enqueue_time.elapsed();
    let start = Instant::now();

    // Generate queryID for click analytics correlation
    let query_id = if req.click_analytics == Some(true) {
        Some(hex::encode(uuid::Uuid::new_v4().as_bytes()))
    } else {
        None
    };

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

    let loaded_settings = state.manager.get_settings(&index_name);

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
    let (fetch_limit, fetch_offset) = if geo_params.has_geo_filter() {
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
        &index_name,
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

    let search_elapsed = start.elapsed();

    let query_words = extract_query_words(&req.query);
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
                let highlight_map = highlighter.highlight_document(
                    &scored_doc.document,
                    &query_words,
                    &searchable_paths,
                );
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

    // Record analytics event (fire-and-forget, never blocks search response)
    if req.analytics != Some(false) {
        if let Some(collector) = flapjack::analytics::get_global_collector() {
            let analytics_tags_str = req.analytics_tags.as_ref().map(|t| t.join(","));
            let facets_str = req
                .facets
                .as_ref()
                .map(|f| serde_json::to_string(f).unwrap_or_default());
            collector.record_search(flapjack::analytics::schema::SearchEvent {
                timestamp_ms: chrono::Utc::now().timestamp_millis(),
                query: req.query.clone(),
                query_id: query_id.clone(),
                index_name: index_name.clone(),
                nb_hits: result.total as u32,
                processing_time_ms: total_elapsed.as_millis() as u32,
                user_token: req.user_token.clone(),
                user_ip: req.user_ip.clone(),
                filters: req.filters.clone(),
                facets: facets_str,
                analytics_tags: analytics_tags_str,
                page: page as u32,
                hits_per_page: hits_per_page as u32,
                has_results: result.total > 0,
                country: None,
                region: None,
            });
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
