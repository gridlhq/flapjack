use axum::{
    extract::{Path, State},
    Json,
};
use base64::Engine;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use super::AppState;
use crate::filter_parser::parse_filter;
use flapjack::error::FlapjackError;

use super::field_value_to_json;

#[derive(Deserialize)]
pub struct BrowseRequest {
    #[serde(default)]
    pub cursor: Option<String>,

    #[serde(default)]
    pub filters: Option<String>,

    #[serde(default = "default_browse_hits_per_page")]
    #[serde(rename = "hitsPerPage")]
    pub hits_per_page: usize,
}

fn default_browse_hits_per_page() -> usize {
    1000
}

#[derive(Serialize, Deserialize)]
struct BrowseCursor {
    offset: usize,
    generation: u64,
}

/// Browse all documents in an index with pagination
#[utoipa::path(
    post,
    path = "/1/indexes/{indexName}/browse",
    tag = "documents",
    params(
        ("indexName" = String, Path, description = "Index name")
    ),
    request_body(content = serde_json::Value, description = "Browse request with optional cursor"),
    responses(
        (status = 200, description = "Documents page with cursor", body = serde_json::Value),
        (status = 404, description = "Index not found")
    ),
    security(
        ("api_key" = [])
    )
)]
pub async fn browse_index(
    State(state): State<Arc<AppState>>,
    Path(index_name): Path<String>,
    Json(req): Json<BrowseRequest>,
) -> Result<Json<serde_json::Value>, FlapjackError> {
    let index = state.manager.get_or_load(&index_name)?;
    let reader = index.reader();
    let searcher = reader.searcher();
    let current_generation = searcher
        .segment_readers()
        .iter()
        .map(|sr| sr.segment_id().uuid_string())
        .collect::<Vec<_>>()
        .join("-");
    let current_gen_hash = {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        current_generation.hash(&mut hasher);
        hasher.finish()
    };

    let (offset, _expected_gen) = if let Some(cursor_str) = &req.cursor {
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(cursor_str)
            .map_err(|_| FlapjackError::InvalidQuery("Invalid cursor".to_string()))?;
        let cursor_json = String::from_utf8(decoded)
            .map_err(|_| FlapjackError::InvalidQuery("Invalid cursor encoding".to_string()))?;
        let cursor: BrowseCursor = serde_json::from_str(&cursor_json)
            .map_err(|_| FlapjackError::InvalidQuery("Invalid cursor format".to_string()))?;

        if cursor.generation != current_gen_hash {
            return Err(FlapjackError::InvalidQuery(
                "Cursor is not valid anymore (index modified)".to_string(),
            ));
        }

        (cursor.offset, Some(cursor.generation))
    } else {
        (0, None)
    };

    let filter = if let Some(filter_str) = &req.filters {
        Some(
            parse_filter(filter_str)
                .map_err(|e| FlapjackError::InvalidQuery(format!("Filter parse error: {}", e)))?,
        )
    } else {
        None
    };

    let hits_per_page = req.hits_per_page.min(1000);

    let result = state.manager.search_with_facets(
        &index_name,
        "",
        filter.as_ref(),
        None,
        hits_per_page,
        offset,
        None,
    )?;

    let total = result.total;
    let page_docs = &result.documents;

    let hits: Vec<serde_json::Value> = page_docs
        .iter()
        .map(|scored_doc| {
            let mut doc_map = serde_json::Map::new();
            doc_map.insert(
                "objectID".to_string(),
                serde_json::Value::String(scored_doc.document.id.clone()),
            );

            for (key, value) in &scored_doc.document.fields {
                doc_map.insert(key.clone(), field_value_to_json(value));
            }

            serde_json::Value::Object(doc_map)
        })
        .collect();

    let next_offset = offset + hits.len();
    let next_cursor = if next_offset < total {
        let cursor = BrowseCursor {
            offset: next_offset,
            generation: current_gen_hash,
        };
        let cursor_json = serde_json::to_string(&cursor).unwrap();
        Some(base64::engine::general_purpose::STANDARD.encode(cursor_json.as_bytes()))
    } else {
        None
    };

    Ok(Json(serde_json::json!({
        "hits": hits,
        "cursor": next_cursor,
        "nbHits": total
    })))
}
