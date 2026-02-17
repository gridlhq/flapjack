use super::QueryExecutor;
use crate::error::Result;
use crate::types::{FacetCount, FacetRequest, SearchResult, Sort};
use std::collections::HashMap;
use tantivy::collector::{Count, FacetCollector, TopDocs};
use tantivy::query::Query as TantivyQuery;
use tantivy::Searcher;

impl QueryExecutor {
    pub fn execute_with_facets(
        &self,
        searcher: &Searcher,
        query: Box<dyn TantivyQuery>,
        filter: Option<&crate::types::Filter>,
        sort: Option<&Sort>,
        limit: usize,
        offset: usize,
        has_text_query: bool,
        facet_requests: Option<&[FacetRequest]>,
    ) -> Result<SearchResult> {
        self.execute_with_facets_and_distinct(
            searcher,
            query,
            filter,
            sort,
            limit,
            offset,
            has_text_query,
            facet_requests,
            None,
        )
    }

    pub fn execute_with_facets_and_distinct(
        &self,
        searcher: &Searcher,
        query: Box<dyn TantivyQuery>,
        filter: Option<&crate::types::Filter>,
        sort: Option<&Sort>,
        limit: usize,
        offset: usize,
        has_text_query: bool,
        facet_requests: Option<&[FacetRequest]>,
        distinct_count: Option<u32>,
    ) -> Result<SearchResult> {
        let tf0 = std::time::Instant::now();
        let final_query = self.apply_filter(query, filter)?;
        let tf1 = tf0.elapsed();

        if let Some(facets) = facet_requests {
            if facets.is_empty() {
                return Err(crate::error::FlapjackError::InvalidQuery(
                    "Empty facet request array".to_string(),
                ));
            }

            return self.execute_with_facets_internal(
                searcher,
                final_query,
                sort,
                limit,
                offset,
                has_text_query,
                facets,
                distinct_count,
            );
        }

        let tf2 = tf0.elapsed();
        let (documents, total) = match sort {
            None | Some(Sort::ByRelevance) => {
                let (docs, count) =
                    self.execute_relevance_sort(searcher, final_query, limit, offset)?;
                tracing::debug!(
                    "[EXEC] filter={:?} relevance_sort={:?}",
                    tf1,
                    tf0.elapsed().saturating_sub(tf2)
                );
                (docs, count)
            }
            Some(Sort::ByField { field, order }) => {
                if has_text_query {
                    let (docs, count) = self.execute_relevance_first_sort(
                        searcher,
                        final_query,
                        field,
                        order,
                        limit,
                        offset,
                    )?;
                    (docs, count)
                } else {
                    let (docs, count) =
                        self.execute_pure_sort(searcher, final_query, field, order, limit, offset)?;
                    (docs, count)
                }
            }
        };

        let (documents, total) = if let Some(distinct) = distinct_count {
            if distinct > 0 {
                self.apply_distinct(documents, total, distinct)?
            } else {
                (documents, total)
            }
        } else {
            (documents, total)
        };

        Ok(SearchResult {
            documents,
            total,
            facets: HashMap::new(),
            user_data: Vec::new(),
            applied_rules: Vec::new(),
        })
    }

    pub(crate) fn execute_with_facets_internal(
        &self,
        searcher: &Searcher,
        query: Box<dyn TantivyQuery>,
        sort: Option<&Sort>,
        limit: usize,
        offset: usize,
        has_text_query: bool,
        facet_requests: &[FacetRequest],
        distinct_count: Option<u32>,
    ) -> Result<SearchResult> {
        let mut facet_collector = FacetCollector::for_field("_facets");
        for req in facet_requests {
            facet_collector.add_facet(&req.path);
        }

        tracing::debug!(
            "[FACET_SORT] sort={:?} has_text_query={} limit={} offset={}",
            sort,
            has_text_query,
            limit,
            offset
        );

        if limit == 0 && offset == 0 {
            let (count, facets) = searcher.search(query.as_ref(), &(Count, facet_collector))?;
            let (documents, total) = if let Some(distinct) = distinct_count {
                if distinct > 0 {
                    self.apply_distinct(Vec::new(), count, distinct)?
                } else {
                    (Vec::new(), count)
                }
            } else {
                (Vec::new(), count)
            };
            return Ok(SearchResult {
                documents,
                total,
                facets: self.extract_facet_counts(facets, facet_requests),
                user_data: Vec::new(),
                applied_rules: Vec::new(),
            });
        }

        let (documents, total, facet_counts) = match sort {
            None | Some(Sort::ByRelevance) => {
                let fi0 = std::time::Instant::now();
                let prelim_limit = if self
                    .settings
                    .as_ref()
                    .and_then(|s| s.custom_ranking.as_ref())
                    .is_some()
                {
                    (limit + offset).saturating_mul(3).max(50)
                } else {
                    limit + offset
                };
                let top_collector = TopDocs::with_limit(prelim_limit);
                let (count, mut top_docs, facets) =
                    searcher.search(query.as_ref(), &(Count, top_collector, facet_collector))?;
                let fi1 = fi0.elapsed();
                let query_terms: Vec<String> = self
                    .query_text
                    .split_whitespace()
                    .map(|s| s.to_lowercase())
                    .collect();
                top_docs = self.apply_tier2_and_custom_ranking(searcher, top_docs, &query_terms)?;
                let fi2 = fi0.elapsed();
                let final_docs = top_docs.into_iter().skip(offset).take(limit).collect();
                let docs = self.reconstruct_documents(searcher, final_docs)?;
                tracing::debug!(
                    "[FACET_INT] search={:?} tier2={:?} reconstruct={:?} prelim_limit={} count={}",
                    fi1,
                    fi2.saturating_sub(fi1),
                    fi0.elapsed().saturating_sub(fi2),
                    prelim_limit,
                    count
                );
                (docs, count, facets)
            }
            Some(Sort::ByField { field, order }) => {
                if has_text_query {
                    let prelim_limit = (limit + offset).saturating_mul(3).max(50);
                    let top_collector = TopDocs::with_limit(prelim_limit).and_offset(offset);
                    let (count, prelim, facets) = searcher
                        .search(query.as_ref(), &(Count, top_collector, facet_collector))?;
                    let sorted =
                        self.sort_docs_by_json_field(searcher, prelim, field, order, limit, 0)?;
                    (self.reconstruct_documents(searcher, sorted)?, count, facets)
                } else {
                    self.execute_pure_sort_internal(
                        searcher,
                        query,
                        field,
                        order,
                        limit,
                        offset,
                        Some(facet_collector),
                    )?
                }
            }
        };

        let (documents, total) = if let Some(distinct) = distinct_count {
            if distinct > 0 {
                self.apply_distinct(documents, total, distinct)?
            } else {
                (documents, total)
            }
        } else {
            (documents, total)
        };

        Ok(SearchResult {
            documents,
            total,
            facets: self.extract_facet_counts(facet_counts, facet_requests),
            user_data: Vec::new(),
            applied_rules: Vec::new(),
        })
    }

    pub(crate) fn apply_distinct(
        &self,
        documents: Vec<crate::types::ScoredDocument>,
        original_total: usize,
        distinct_count: u32,
    ) -> Result<(Vec<crate::types::ScoredDocument>, usize)> {
        let attr_name = match &self.settings {
            Some(s) => match &s.attribute_for_distinct {
                Some(attr) => attr,
                None => return Ok((documents, original_total)),
            },
            None => return Ok((documents, original_total)),
        };

        let mut seen: HashMap<String, u32> = HashMap::new();
        let mut deduplicated = Vec::new();

        for doc in documents {
            let key = match doc.document.fields.get(attr_name) {
                Some(crate::types::FieldValue::Text(s)) => s.clone(),
                Some(crate::types::FieldValue::Integer(i)) => i.to_string(),
                Some(crate::types::FieldValue::Float(f)) => f.round().to_string(),
                _ => continue,
            };

            let count = seen.entry(key.clone()).or_insert(0);
            if *count < distinct_count {
                *count += 1;
                deduplicated.push(doc);
            }
        }

        let group_count = if deduplicated.is_empty() {
            0
        } else {
            seen.len()
        };
        Ok((deduplicated, group_count))
    }

    pub(crate) fn trim_facet_counts(
        &self,
        facets: HashMap<String, Vec<FacetCount>>,
        _requests: &[FacetRequest],
    ) -> HashMap<String, Vec<FacetCount>> {
        let limit = self
            .max_values_per_facet
            .or_else(|| {
                self.settings
                    .as_ref()
                    .map(|s| s.max_values_per_facet as usize)
            })
            .unwrap_or(100)
            .min(1000);
        facets
            .into_iter()
            .map(|(k, v)| (k, v.into_iter().take(limit).collect()))
            .collect()
    }

    pub(crate) fn extract_facet_counts(
        &self,
        facet_counts: tantivy::collector::FacetCounts,
        requests: &[FacetRequest],
    ) -> HashMap<String, Vec<FacetCount>> {
        let limit = self
            .max_values_per_facet
            .or_else(|| {
                self.settings
                    .as_ref()
                    .map(|s| s.max_values_per_facet as usize)
            })
            .unwrap_or(100)
            .min(1000);

        let mut result = HashMap::new();

        for req in requests {
            let counts: Vec<FacetCount> = facet_counts
                .top_k(&req.path, limit)
                .into_iter()
                .map(|(facet, count)| {
                    let path_str = facet.to_path_string();
                    let trimmed = path_str.trim_start_matches('/');
                    let prefix = format!("{}/", req.path.trim_start_matches('/'));
                    let clean_path = trimmed.strip_prefix(&prefix).unwrap_or(trimmed).to_string();

                    FacetCount {
                        path: clean_path,
                        count,
                    }
                })
                .collect();

            let mut counts = counts;
            counts.sort_by(|a, b| b.count.cmp(&a.count));
            result
                .entry(req.field.clone())
                .or_insert_with(Vec::new)
                .extend(counts);
        }

        result
    }
}
