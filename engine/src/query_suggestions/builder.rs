use super::config::{BuildStatus, LogEntry, QsConfig, QsConfigStore};
use crate::analytics::AnalyticsQueryEngine;
use crate::types::{Document, FieldValue};
use crate::IndexManager;
use std::collections::HashMap;
use std::sync::Arc;

fn log_info(msg: &str) -> LogEntry {
    LogEntry {
        timestamp: chrono::Utc::now().to_rfc3339(),
        level: "INFO".to_string(),
        message: msg.to_string(),
        context_level: 1,
    }
}

fn log_skip(msg: &str) -> LogEntry {
    LogEntry {
        timestamp: chrono::Utc::now().to_rfc3339(),
        level: "SKIP".to_string(),
        message: msg.to_string(),
        context_level: 2,
    }
}

fn log_error(msg: &str) -> LogEntry {
    LogEntry {
        timestamp: chrono::Utc::now().to_rfc3339(),
        level: "ERROR".to_string(),
        message: msg.to_string(),
        context_level: 0,
    }
}

/// Build a suggestions index from analytics data.
///
/// Uses an atomic swap: builds into `{indexName}__building`, then renames
/// to `{indexName}` so the live index is never empty during a rebuild.
///
/// Returns the number of suggestions written.
pub async fn build_suggestions_index(
    config: &QsConfig,
    store: &QsConfigStore,
    manager: &Arc<IndexManager>,
    analytics_engine: &Arc<AnalyticsQueryEngine>,
) -> Result<usize, String> {
    let staging_name = format!("{}__building", config.index_name);

    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let thirty_days_ago = (chrono::Utc::now() - chrono::Duration::days(30))
        .format("%Y-%m-%d")
        .to_string();

    let mut log_entries: Vec<LogEntry> = vec![];
    log_entries.push(log_info(&format!(
        "Starting build for '{}' (sources: {})",
        config.index_name,
        config
            .source_indices
            .iter()
            .map(|s| s.index_name.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    )));

    // Set up staging index: delete if it exists, then recreate fresh
    if manager.get_or_load(&staging_name).is_ok() {
        manager
            .delete_tenant(&staging_name)
            .await
            .map_err(|e| e.to_string())?;
    } else {
        // May exist on disk but not loaded — delete_tenant handles both cases
        let staging_path = manager.base_path.join(&staging_name);
        if staging_path.exists() {
            manager
                .delete_tenant(&staging_name)
                .await
                .map_err(|e| e.to_string())?;
        }
    }
    manager
        .create_tenant(&staging_name)
        .map_err(|e| e.to_string())?;

    // Aggregate suggestions across all source indices.
    // Key: query string → max popularity across sources
    let mut query_map: HashMap<String, u64> = HashMap::new();

    for source in &config.source_indices {
        let tags_opt: Option<String> = if source.analytics_tags.is_empty() {
            None
        } else {
            Some(source.analytics_tags.join(","))
        };

        let result = analytics_engine
            .top_searches(
                &source.index_name,
                &thirty_days_ago,
                &today,
                10_000,
                false,
                None,
                tags_opt.as_deref(),
            )
            .await;

        let searches = match result {
            Ok(val) => val["searches"].as_array().cloned().unwrap_or_default(),
            Err(e) => {
                log_entries.push(log_error(&format!(
                    "Analytics query failed for '{}': {}",
                    source.index_name, e
                )));
                vec![]
            }
        };

        log_entries.push(log_info(&format!(
            "Got {} raw searches from analytics for '{}'",
            searches.len(),
            source.index_name
        )));

        for item in &searches {
            // top_searches returns {search: "...", count: N, nbHits: M}
            // count = how many times searched (→ popularity)
            // nbHits = average result count per search (Algolia minHits filters on this)
            let query = match item["search"].as_str() {
                Some(q) if !q.is_empty() => q.to_string(),
                _ => continue,
            };
            let count = item["count"].as_u64().unwrap_or(0);
            // nbHits: Algolia stores this as integer; DataFusion returns it as integer too.
            let nb_hits = item["nbHits"].as_u64().unwrap_or(0);

            // Filter: min_letters (character count, not byte count)
            let char_count = query.chars().count();
            if char_count < source.min_letters {
                log_entries.push(log_skip(&format!(
                    "Skipping '{}': {} chars < minLetters {}",
                    query, char_count, source.min_letters
                )));
                continue;
            }

            // Filter: min_hits — Algolia parity: filters on result count (nbHits),
            // not search frequency. A query that returns 0 results is never a useful
            // suggestion, even if users searched for it many times.
            if nb_hits < source.min_hits {
                log_entries.push(log_skip(&format!(
                    "Skipping '{}': avg nbHits {} < minHits {}",
                    query, nb_hits, source.min_hits
                )));
                continue;
            }

            // Filter: exclude list (case-insensitive exact match)
            let lower = query.to_lowercase();
            if config.exclude.iter().any(|e| lower == e.to_lowercase()) {
                log_entries.push(log_skip(&format!("Skipping '{}': in exclude list", query)));
                continue;
            }

            // Take the max count across sources for deduplication
            let entry = query_map.entry(query).or_insert(0);
            *entry = (*entry).max(count);
        }

        if !source.generate.is_empty() {
            log_entries.push(log_info(
                "generate field present — facet-value suggestions deferred to v2",
            ));
        }
        if !source.facets.is_empty() {
            log_entries.push(log_info(
                "facets field present — facet enrichment deferred to v2",
            ));
        }
    }

    // Build Document list
    let mut docs: Vec<Document> = Vec::with_capacity(query_map.len());
    for (query, popularity) in &query_map {
        let nb_words = query.split_whitespace().count() as i64;
        let mut fields = HashMap::new();
        fields.insert("query".to_string(), FieldValue::Text(query.clone()));
        fields.insert("nb_words".to_string(), FieldValue::Integer(nb_words));
        fields.insert(
            "popularity".to_string(),
            FieldValue::Integer(*popularity as i64),
        );
        // Stub exact_nb_hits per source index (v2: run a search per suggestion)
        for source in &config.source_indices {
            fields.insert(
                source.index_name.clone(),
                FieldValue::Object(HashMap::from([(
                    "exact_nb_hits".to_string(),
                    FieldValue::Integer(0),
                )])),
            );
        }
        docs.push(Document {
            id: query.clone(),
            fields,
        });
    }

    let doc_count = docs.len();
    log_entries.push(log_info(&format!(
        "Writing {} suggestions to staging index '{}'",
        doc_count, staging_name
    )));

    if !docs.is_empty() {
        manager
            .add_documents_sync(&staging_name, docs)
            .await
            .map_err(|e| e.to_string())?;
    }

    // Atomic swap: move staging over live index
    manager
        .move_index(&staging_name, &config.index_name)
        .await
        .map_err(|e| e.to_string())?;

    let now = chrono::Utc::now().to_rfc3339();
    log_entries.push(log_info(&format!(
        "Build complete: {} suggestions written to '{}'",
        doc_count, config.index_name
    )));

    store.append_log(&config.index_name, &log_entries).ok();
    store.truncate_log(&config.index_name, 1000).ok();

    let status = BuildStatus {
        index_name: config.index_name.clone(),
        is_running: false,
        last_built_at: Some(now.clone()),
        last_successful_built_at: Some(now),
    };
    store.save_status(&status).ok();

    Ok(doc_count)
}
