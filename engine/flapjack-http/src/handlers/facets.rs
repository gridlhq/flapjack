use super::AppState;
use crate::dto::{FacetHit, SearchFacetValuesRequest, SearchFacetValuesResponse};
use crate::filter_parser::parse_filter;
use axum::{
    extract::{Path, State},
    Json,
};
use flapjack::error::FlapjackError;
use flapjack::index::settings::IndexSettings;
use flapjack::types::FacetRequest;
use std::sync::Arc;
use std::time::Instant;

pub fn parse_facet_params(params_str: &str) -> SearchFacetValuesRequest {
    let mut facet_query = String::new();
    let mut filters = None;
    let mut max_facet_hits = 10usize;

    for (key, value) in url::form_urlencoded::parse(params_str.as_bytes()) {
        match key.as_ref() {
            "facetQuery" => facet_query = value.into_owned(),
            "filters" => filters = Some(value.into_owned()),
            "maxFacetHits" => max_facet_hits = value.parse().unwrap_or(10),
            _ => {}
        }
    }

    SearchFacetValuesRequest {
        facet_query,
        filters,
        max_facet_hits,
    }
}

fn highlight_facet_match(value: &str, query: &str) -> String {
    let value_lower = value.to_lowercase();
    let query_lower = query.to_lowercase();
    if let Some(pos) = value_lower.find(&query_lower) {
        let match_end = pos + query.len();
        let safe_end = match_end.min(value.len());
        format!(
            "{}<em>{}</em>{}",
            &value[..pos],
            &value[pos..safe_end],
            &value[safe_end..]
        )
    } else {
        value.to_string()
    }
}

/// Search for facet values from a multi-search `type: "facet"` query.
/// Called by the batch_search handler when a request has `type: "facet"`.
pub async fn search_facet_values_inline(
    state: Arc<AppState>,
    index_name: &str,
    facet_name: &str,
    facet_query: &str,
    max_facet_hits: usize,
    filters: Option<&str>,
) -> Result<serde_json::Value, FlapjackError> {
    let start = Instant::now();

    let settings_path = state
        .manager
        .base_path
        .join(index_name)
        .join("settings.json");
    let settings = if settings_path.exists() {
        IndexSettings::load(&settings_path)?
    } else {
        // Return empty facet hits for missing index (don't fail the batch)
        return Ok(serde_json::json!({
            "facetHits": [],
            "exhaustiveFacetsCount": true,
            "processingTimeMS": 0
        }));
    };

    let searchable_facets = settings.searchable_facet_set();
    if !searchable_facets.contains(facet_name) {
        return Ok(serde_json::json!({
            "facetHits": [],
            "exhaustiveFacetsCount": true,
            "processingTimeMS": 0
        }));
    }

    let filter = if let Some(filter_str) = filters {
        Some(
            parse_filter(filter_str)
                .map_err(|e| FlapjackError::InvalidQuery(format!("Filter parse error: {}", e)))?,
        )
    } else {
        None
    };

    let facet_request = FacetRequest {
        field: facet_name.to_string(),
        path: format!("/{}", facet_name),
    };

    let result = state.manager.search_full(
        index_name,
        "",
        filter.as_ref(),
        None,
        0,
        0,
        Some(&[facet_request]),
        None,
        Some(1000),
    )?;

    let facet_counts = result.facets.get(facet_name);
    let query_lower = facet_query.to_lowercase();
    let empty_vec = Vec::new();
    let counts = facet_counts.unwrap_or(&empty_vec);

    let mut matching: Vec<_> = counts
        .iter()
        .filter(|fc| {
            if query_lower.is_empty() {
                return true;
            }
            let leaf_value = fc.path.rsplit(" > ").next().unwrap_or(&fc.path);
            leaf_value.to_lowercase().contains(&query_lower)
        })
        .collect();

    matching.sort_by(|a, b| b.count.cmp(&a.count));

    let hits: Vec<serde_json::Value> = matching
        .into_iter()
        .take(max_facet_hits)
        .map(|fc| {
            let value = fc.path.clone();
            let highlighted = if facet_query.is_empty() {
                value.clone()
            } else {
                highlight_facet_match(&value, facet_query)
            };
            serde_json::json!({
                "value": value,
                "highlighted": highlighted,
                "count": fc.count
            })
        })
        .collect();

    Ok(serde_json::json!({
        "facetHits": hits,
        "exhaustiveFacetsCount": true,
        "processingTimeMS": start.elapsed().as_millis() as u64
    }))
}

/// Search for facet values with optional filtering
#[utoipa::path(
    post,
    path = "/1/indexes/{indexName}/facets/{facetName}/query",
    tag = "search",
    params(
        ("indexName" = String, Path, description = "Index name"),
        ("facetName" = String, Path, description = "Facet field name")
    ),
    request_body(content = SearchFacetValuesRequest, description = "Facet search parameters"),
    responses(
        (status = 200, description = "Facet values matching query", body = SearchFacetValuesResponse),
        (status = 404, description = "Index or facet not found")
    ),
    security(
        ("api_key" = [])
    )
)]
pub async fn search_facet_values(
    State(state): State<Arc<AppState>>,
    Path((index_name, facet_name)): Path<(String, String)>,
    body: axum::body::Bytes,
) -> Result<Json<SearchFacetValuesResponse>, FlapjackError> {
    let start = Instant::now();

    let body_str = String::from_utf8_lossy(&body);

    let req: SearchFacetValuesRequest = if body_str.is_empty() || body_str == "{}" {
        SearchFacetValuesRequest {
            facet_query: String::new(),
            filters: None,
            max_facet_hits: 10,
        }
    } else {
        let body_json: serde_json::Value = serde_json::from_str(&body_str)
            .map_err(|e| FlapjackError::InvalidQuery(format!("Invalid JSON: {}", e)))?;

        if let Some(params_val) = body_json.get("params") {
            if let Some(params_str) = params_val.as_str() {
                parse_facet_params(params_str)
            } else {
                return Err(FlapjackError::InvalidQuery(
                    "params must be a string".to_string(),
                ));
            }
        } else if let Ok(r) = serde_json::from_value::<SearchFacetValuesRequest>(body_json.clone())
        {
            r
        } else {
            SearchFacetValuesRequest {
                facet_query: body_json
                    .get("facetQuery")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                filters: body_json
                    .get("filters")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                max_facet_hits: body_json
                    .get("maxFacetHits")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(10) as usize,
            }
        }
    };

    let settings_path = state
        .manager
        .base_path
        .join(&index_name)
        .join("settings.json");
    let settings = if settings_path.exists() {
        IndexSettings::load(&settings_path)?
    } else {
        return Err(FlapjackError::InvalidQuery(
            format!("Cannot search in `{}` attribute, you need to add `searchable({})` to attributesForFaceting.", facet_name, facet_name)
        ));
    };

    let searchable_facets = settings.searchable_facet_set();
    if !searchable_facets.contains(&facet_name) {
        return Err(FlapjackError::InvalidQuery(
            format!("Cannot search in `{}` attribute, you need to add `searchable({})` to attributesForFaceting.", facet_name, facet_name)
        ));
    }

    let filter = if let Some(filter_str) = &req.filters {
        Some(
            parse_filter(filter_str)
                .map_err(|e| FlapjackError::InvalidQuery(format!("Filter parse error: {}", e)))?,
        )
    } else {
        None
    };

    let facet_request = FacetRequest {
        field: facet_name.clone(),
        path: format!("/{}", facet_name),
    };

    let result = state.manager.search_full(
        &index_name,
        "",
        filter.as_ref(),
        None,
        0,
        0,
        Some(&[facet_request]),
        None,
        Some(1000),
    )?;

    let facet_counts = result.facets.get(&facet_name);

    let query_lower = req.facet_query.to_lowercase();
    let empty_vec = Vec::new();
    let counts = facet_counts.unwrap_or(&empty_vec);

    let mut matching: Vec<_> = counts
        .iter()
        .filter(|fc| {
            if query_lower.is_empty() {
                return true;
            }
            let leaf_value = fc.path.rsplit(" > ").next().unwrap_or(&fc.path);
            leaf_value.to_lowercase().contains(&query_lower)
        })
        .collect();

    matching.sort_by(|a, b| b.count.cmp(&a.count));

    let hits: Vec<FacetHit> = matching
        .into_iter()
        .take(req.max_facet_hits)
        .map(|fc| {
            let value = fc.path.clone();
            let highlighted = if req.facet_query.is_empty() {
                value.clone()
            } else {
                highlight_facet_match(&value, &req.facet_query)
            };

            FacetHit {
                value,
                highlighted,
                count: fc.count,
            }
        })
        .collect();

    Ok(Json(SearchFacetValuesResponse {
        facet_hits: hits,
        exhaustive_facets_count: true,
        processing_time_ms: start.elapsed().as_millis() as u64,
    }))
}
