use super::sorting::SortValue;
use super::QueryExecutor;
use crate::error::Result;
use crate::types::ScoredDocument;
use tantivy::collector::{Count, TopDocs};
use tantivy::postings::Postings;
use tantivy::query::Query as TantivyQuery;
use tantivy::schema::{document::ReferenceValue, Value};
use tantivy::{DocSet, Searcher, TERMINATED};

impl QueryExecutor {
    pub(crate) fn execute_relevance_sort(
        &self,
        searcher: &Searcher,
        query: Box<dyn TantivyQuery>,
        limit: usize,
        offset: usize,
    ) -> Result<(Vec<ScoredDocument>, usize)> {
        let tr0 = std::time::Instant::now();
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

        let (total, mut top_docs) =
            searcher.search(query.as_ref(), &(Count, TopDocs::with_limit(prelim_limit)))?;
        let tr1 = tr0.elapsed();

        let query_terms: Vec<String> = self
            .query_text
            .split_whitespace()
            .map(|s| s.to_lowercase())
            .collect();
        top_docs = self.apply_tier2_and_custom_ranking(searcher, top_docs, &query_terms)?;
        let tr2 = tr0.elapsed();

        let final_docs = top_docs.into_iter().skip(offset).take(limit).collect();
        let documents = self.reconstruct_documents(searcher, final_docs)?;
        tracing::debug!(
            "[REL] search={:?} tier2={:?} reconstruct={:?} total_hits={}",
            tr1,
            tr2.saturating_sub(tr1),
            tr0.elapsed().saturating_sub(tr2),
            total
        );
        Ok((documents, total))
    }

    pub(crate) fn apply_tier2_and_custom_ranking(
        &self,
        searcher: &Searcher,
        docs: Vec<(f32, tantivy::DocAddress)>,
        query_terms: &[String],
    ) -> Result<Vec<(f32, tantivy::DocAddress)>> {
        let custom_ranking = match &self.settings {
            Some(s) => match &s.custom_ranking {
                Some(cr) if !cr.is_empty() => cr,
                _ => return self.apply_tier2_only(searcher, docs, query_terms),
            },
            None => return self.apply_tier2_only(searcher, docs, query_terms),
        };

        let tantivy_schema = searcher.schema();
        let json_filter_field = tantivy_schema
            .get_field("_json_filter")
            .map_err(|_| crate::error::FlapjackError::FieldNotFound("_json_filter".to_string()))?;

        let mut ranking_specs: Vec<(String, bool)> = Vec::new();

        for spec in custom_ranking {
            let (direction, attr) = if let Some(attr) = spec.strip_prefix("desc(") {
                (false, attr.trim_end_matches(')'))
            } else if let Some(attr) = spec.strip_prefix("asc(") {
                (true, attr.trim_end_matches(')'))
            } else {
                continue;
            };

            ranking_specs.push((attr.to_string(), direction));
        }

        if ranking_specs.is_empty() {
            return self.apply_tier2_only(searcher, docs, query_terms);
        }

        let mut scored: Vec<(u32, Vec<SortValue>, f32, tantivy::DocAddress)> = Vec::new();

        for (score, addr) in docs {
            let doc: tantivy::TantivyDocument = searcher.doc(addr)?;

            let min_position = self.extract_min_position(searcher, addr, query_terms)?;

            let mut values = Vec::new();

            for (attr_name, _) in &ranking_specs {
                let val = if let Some(json_value) = doc.get_first(json_filter_field) {
                    if let ReferenceValue::Object(obj) = json_value.as_value() {
                        let mut found = SortValue::Missing;
                        for (path, compact_val) in obj {
                            if path == attr_name {
                                found = match compact_val.as_value() {
                                    ReferenceValue::Leaf(leaf) => match leaf {
                                        tantivy::schema::document::ReferenceValueLeaf::I64(i) => {
                                            SortValue::Integer(i)
                                        }
                                        tantivy::schema::document::ReferenceValueLeaf::U64(u) => {
                                            SortValue::Integer(u as i64)
                                        }
                                        tantivy::schema::document::ReferenceValueLeaf::F64(f) => {
                                            SortValue::Float(f)
                                        }
                                        tantivy::schema::document::ReferenceValueLeaf::Str(s) => {
                                            SortValue::Text(s.to_string())
                                        }
                                        _ => SortValue::Missing,
                                    },
                                    _ => SortValue::Missing,
                                };
                                break;
                            }
                        }
                        found
                    } else {
                        SortValue::Missing
                    }
                } else {
                    SortValue::Missing
                };
                values.push(val);
            }
            scored.push((min_position, values, score, addr));
        }

        scored.sort_by(|a, b| {
            let pos_cmp = a.0.cmp(&b.0);
            if pos_cmp != std::cmp::Ordering::Equal {
                return pos_cmp;
            }

            for (idx, (_, asc)) in ranking_specs.iter().enumerate() {
                let cmp = match (&a.1[idx], &b.1[idx]) {
                    (SortValue::Missing, SortValue::Missing) => std::cmp::Ordering::Equal,
                    (SortValue::Missing, _) => std::cmp::Ordering::Greater,
                    (_, SortValue::Missing) => std::cmp::Ordering::Less,
                    _ => {
                        if *asc {
                            a.1[idx].cmp(&b.1[idx])
                        } else {
                            b.1[idx].cmp(&a.1[idx])
                        }
                    }
                };
                if cmp != std::cmp::Ordering::Equal {
                    return cmp;
                }
            }
            a.3.doc_id.cmp(&b.3.doc_id)
        });

        Ok(scored
            .into_iter()
            .map(|(_, _, score, addr)| (score, addr))
            .collect())
    }

    pub(crate) fn apply_tier2_only(
        &self,
        searcher: &Searcher,
        docs: Vec<(f32, tantivy::DocAddress)>,
        query_terms: &[String],
    ) -> Result<Vec<(f32, tantivy::DocAddress)>> {
        use std::collections::HashMap;
        use tantivy::schema::IndexRecordOption;

        let mut doc_positions: HashMap<(u32, u32), u32> = HashMap::new();

        for doc in &docs {
            doc_positions.insert((doc.1.segment_ord, doc.1.doc_id), u32::MAX);
        }

        let top_paths: Vec<&String> = self.searchable_paths.iter().take(2).collect();

        for segment_ord in 0..searcher.segment_readers().len() {
            let segment_reader = searcher.segment_reader(segment_ord as u32);
            let inverted_index = segment_reader.inverted_index(self.json_search_field)?;

            for path in &top_paths {
                for term_text in query_terms {
                    let full_term = format!("{}\0s{}", path, term_text);
                    let term = tantivy::Term::from_field_text(self.json_search_field, &full_term);

                    if let Some(mut postings) = inverted_index
                        .read_postings(&term, IndexRecordOption::WithFreqsAndPositions)?
                    {
                        let mut doc_id = postings.doc();
                        while doc_id != TERMINATED {
                            let key = (segment_ord as u32, doc_id);
                            if let Some(min_pos) = doc_positions.get_mut(&key) {
                                let mut positions: Vec<u32> =
                                    Vec::with_capacity(postings.term_freq() as usize);
                                postings.positions(&mut positions);
                                if let Some(&first_pos) = positions.first() {
                                    *min_pos = (*min_pos).min(first_pos);
                                }
                            }
                            doc_id = postings.advance();
                        }
                    }
                }
            }
        }

        let mut scored: Vec<(u32, f32, tantivy::DocAddress)> = docs
            .into_iter()
            .map(|(score, addr)| {
                let pos = doc_positions
                    .get(&(addr.segment_ord, addr.doc_id))
                    .copied()
                    .unwrap_or(u32::MAX);
                (pos, score, addr)
            })
            .collect();

        scored.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.2.doc_id.cmp(&b.2.doc_id)));

        Ok(scored
            .into_iter()
            .map(|(_, score, addr)| (score, addr))
            .collect())
    }

    pub(crate) fn extract_min_position(
        &self,
        searcher: &Searcher,
        addr: tantivy::DocAddress,
        query_terms: &[String],
    ) -> Result<u32> {
        use tantivy::schema::IndexRecordOption;

        let segment_reader = searcher.segment_reader(addr.segment_ord);
        let inverted_index = segment_reader.inverted_index(self.json_search_field)?;

        let mut min_pos = u32::MAX;

        // Only check top 2 searchable paths (same as apply_tier2_only)
        for path in self.searchable_paths.iter().take(2) {
            for term_text in query_terms {
                let full_term = format!("{}\0s{}", path, term_text);
                let term = tantivy::Term::from_field_text(self.json_search_field, &full_term);

                if let Some(mut postings) =
                    inverted_index.read_postings(&term, IndexRecordOption::WithFreqsAndPositions)?
                {
                    // Use seek() to jump directly to target doc instead of
                    // walking through every doc in the posting list.
                    let doc_id = postings.seek(addr.doc_id);
                    if doc_id == addr.doc_id {
                        let mut positions: Vec<u32> =
                            Vec::with_capacity(postings.term_freq() as usize);
                        postings.positions(&mut positions);
                        if let Some(&first_pos) = positions.first() {
                            min_pos = min_pos.min(first_pos);
                        }
                    }
                }
            }
        }

        Ok(min_pos)
    }
}
