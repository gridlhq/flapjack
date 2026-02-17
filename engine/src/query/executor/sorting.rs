use super::QueryExecutor;
use crate::error::Result;
use crate::types::{ScoredDocument, SearchResult, Sort, SortOrder};
use tantivy::collector::{Count, TopDocs};
use tantivy::query::Query as TantivyQuery;
use tantivy::Searcher;

impl QueryExecutor {
    pub fn execute_with_sort(
        &self,
        searcher: &Searcher,
        query: Box<dyn TantivyQuery>,
        filter: Option<&crate::types::Filter>,
        sort: Option<&Sort>,
        limit: usize,
        has_text_query: bool,
    ) -> Result<SearchResult> {
        let final_query = self.apply_filter(query, filter)?;

        let (documents, total) = match sort {
            None | Some(Sort::ByRelevance) => {
                self.execute_relevance_sort(searcher, final_query, limit, 0)?
            }
            Some(Sort::ByField { field, order }) => {
                if has_text_query {
                    self.execute_relevance_first_sort(
                        searcher,
                        final_query,
                        field,
                        order,
                        limit,
                        0,
                    )?
                } else {
                    self.execute_pure_sort_fast(searcher, final_query, field, order, limit, 0)?
                }
            }
        };

        Ok(self.build_result(documents, total))
    }

    pub(crate) fn execute_relevance_first_sort(
        &self,
        searcher: &Searcher,
        query: Box<dyn TantivyQuery>,
        field: &str,
        order: &SortOrder,
        limit: usize,
        offset: usize,
    ) -> Result<(Vec<ScoredDocument>, usize)> {
        let prelim_limit = (limit + offset).saturating_mul(100).max(1000);
        let collector = TopDocs::with_limit(prelim_limit).and_offset(offset);
        let (total, prelim_results) = searcher.search(query.as_ref(), &(Count, collector))?;
        let sorted_docs =
            self.sort_docs_by_json_field(searcher, prelim_results, field, order, limit, 0)?;
        let documents = self.reconstruct_documents(searcher, sorted_docs)?;
        Ok((documents, total))
    }

    pub(crate) fn execute_pure_sort_fast(
        &self,
        searcher: &Searcher,
        query: Box<dyn TantivyQuery>,
        field: &str,
        order: &SortOrder,
        limit: usize,
        offset: usize,
    ) -> Result<(Vec<ScoredDocument>, usize)> {
        // Special case: objectID uses the _id FAST field
        if field == "objectID" {
            let is_ascending = matches!(order, SortOrder::Asc);

            let collector = TopDocs::with_limit(limit + offset).custom_score(
                move |segment_reader: &tantivy::SegmentReader| {
                    let ff = segment_reader.fast_fields();
                    // ff.str() takes &str, returns Result<Option<StrColumn>>
                    let str_col: Option<tantivy::columnar::StrColumn> =
                        ff.str("_id").ok().flatten();

                    move |doc_id: tantivy::DocId| {
                        if let Some(ref col) = str_col {
                            // Get term ordinal - ordinals are sorted lexicographically
                            let ord = col.term_ords(doc_id).next().unwrap_or(0);
                            if is_ascending {
                                -(ord as f32)
                            } else {
                                ord as f32
                            }
                        } else {
                            0.0f32
                        }
                    }
                },
            );

            let (total, top_docs) = searcher.search(query.as_ref(), &(Count, collector))?;

            let docs_to_skip = offset.min(top_docs.len());
            let doc_addresses: Vec<(f32, tantivy::DocAddress)> =
                top_docs.into_iter().skip(docs_to_skip).collect();

            let documents = self.reconstruct_documents(searcher, doc_addresses)?;
            return Ok((documents, total));
        }

        // Generic path for other fields
        let fast_field_path = format!("_json_filter.{}", field);
        let is_ascending = matches!(order, SortOrder::Asc);
        let path_clone = fast_field_path.clone();

        let has_fast_column = if let Some(segment) = searcher.segment_readers().first() {
            let ff = segment.fast_fields();
            let col: Option<tantivy::columnar::Column<f64>> =
                ff.column_opt(&path_clone).ok().flatten();
            col.is_some()
        } else {
            false
        };

        if !has_fast_column {
            tracing::warn!(
                "[SORT] No fast column for {}, using fallback",
                fast_field_path
            );
            return self.execute_pure_sort_fallback(searcher, query, field, order, limit, offset);
        }

        let collector = TopDocs::with_limit(limit + offset).custom_score(
            move |segment_reader: &tantivy::SegmentReader| {
                let ff = segment_reader.fast_fields();
                let col: Option<tantivy::columnar::Column<f64>> =
                    ff.column_opt(&path_clone).ok().flatten();

                move |doc_id: tantivy::DocId| {
                    if let Some(ref c) = col {
                        let val = c.first(doc_id).unwrap_or(if is_ascending {
                            f64::MAX
                        } else {
                            f64::MIN
                        });
                        if is_ascending {
                            -val as f32
                        } else {
                            val as f32
                        }
                    } else if is_ascending {
                        f32::MIN
                    } else {
                        f32::MAX
                    }
                }
            },
        );

        let (total, top_docs) = searcher.search(query.as_ref(), &(Count, collector))?;

        let docs_to_skip = offset.min(top_docs.len());
        let doc_addresses: Vec<(f32, tantivy::DocAddress)> =
            top_docs.into_iter().skip(docs_to_skip).collect();

        let documents = self.reconstruct_documents(searcher, doc_addresses)?;
        Ok((documents, total))
    }

    pub(crate) fn execute_pure_sort_fallback(
        &self,
        searcher: &Searcher,
        query: Box<dyn TantivyQuery>,
        field: &str,
        order: &SortOrder,
        limit: usize,
        offset: usize,
    ) -> Result<(Vec<ScoredDocument>, usize)> {
        let fetch_limit = (limit + offset).saturating_mul(3).max(100);
        let collector = TopDocs::with_limit(fetch_limit);
        let (total, prelim_results) = searcher.search(query.as_ref(), &(Count, collector))?;
        let sorted_docs =
            self.sort_docs_by_json_field(searcher, prelim_results, field, order, limit, offset)?;
        let documents = self.reconstruct_documents(searcher, sorted_docs)?;
        Ok((documents, total))
    }

    pub(crate) fn execute_pure_sort(
        &self,
        searcher: &Searcher,
        query: Box<dyn TantivyQuery>,
        field: &str,
        order: &SortOrder,
        limit: usize,
        offset: usize,
    ) -> Result<(Vec<ScoredDocument>, usize)> {
        self.execute_pure_sort_fast(searcher, query, field, order, limit, offset)
    }

    pub(crate) fn execute_pure_sort_internal(
        &self,
        searcher: &Searcher,
        query: Box<dyn TantivyQuery>,
        field: &str,
        order: &SortOrder,
        limit: usize,
        offset: usize,
        facet_collector: Option<tantivy::collector::FacetCollector>,
    ) -> Result<(Vec<ScoredDocument>, usize, tantivy::collector::FacetCounts)> {
        if field == "objectID" {
            let is_ascending = matches!(order, SortOrder::Asc);

            let collector = TopDocs::with_limit(limit + offset).custom_score(
                move |segment_reader: &tantivy::SegmentReader| {
                    let ff = segment_reader.fast_fields();
                    let str_col: Option<tantivy::columnar::StrColumn> =
                        ff.str("_id").ok().flatten();
                    move |doc_id: tantivy::DocId| {
                        if let Some(ref col) = str_col {
                            let ord = col.term_ords(doc_id).next().unwrap_or(0);
                            if is_ascending {
                                -(ord as f32)
                            } else {
                                ord as f32
                            }
                        } else {
                            0.0f32
                        }
                    }
                },
            );

            let (total, top_docs, facets) = if let Some(fc) = facet_collector {
                let (count, docs, f) = searcher.search(query.as_ref(), &(Count, collector, fc))?;
                (count, docs, f)
            } else {
                let (count, docs) = searcher.search(query.as_ref(), &(Count, collector))?;
                (count, docs, tantivy::collector::FacetCounts::default())
            };

            let docs_to_skip = offset.min(top_docs.len());
            let doc_addresses: Vec<(f32, tantivy::DocAddress)> =
                top_docs.into_iter().skip(docs_to_skip).collect();

            let documents = self.reconstruct_documents(searcher, doc_addresses)?;
            return Ok((documents, total, facets));
        }

        let fast_field_path = format!("_json_filter.{}", field);
        let is_ascending = matches!(order, SortOrder::Asc);
        let path_clone = fast_field_path.clone();

        let has_fast_column = if let Some(segment) = searcher.segment_readers().first() {
            let ff = segment.fast_fields();
            let col: Option<tantivy::columnar::Column<f64>> =
                ff.column_opt(&path_clone).ok().flatten();
            col.is_some()
        } else {
            false
        };

        if has_fast_column {
            let collector = TopDocs::with_limit(limit + offset).custom_score(
                move |segment_reader: &tantivy::SegmentReader| {
                    let ff = segment_reader.fast_fields();
                    let col: Option<tantivy::columnar::Column<f64>> =
                        ff.column_opt(&path_clone).ok().flatten();
                    move |doc_id: tantivy::DocId| {
                        if let Some(ref c) = col {
                            let val = c.first(doc_id).unwrap_or(if is_ascending {
                                f64::MAX
                            } else {
                                f64::MIN
                            });
                            if is_ascending {
                                -val as f32
                            } else {
                                val as f32
                            }
                        } else if is_ascending {
                            f32::MIN
                        } else {
                            f32::MAX
                        }
                    }
                },
            );

            let (total, top_docs, facets) = if let Some(fc) = facet_collector {
                let (count, docs, f) = searcher.search(query.as_ref(), &(Count, collector, fc))?;
                (count, docs, f)
            } else {
                let (count, docs) = searcher.search(query.as_ref(), &(Count, collector))?;
                (count, docs, tantivy::collector::FacetCounts::default())
            };

            let docs_to_skip = offset.min(top_docs.len());
            let doc_addresses: Vec<(f32, tantivy::DocAddress)> =
                top_docs.into_iter().skip(docs_to_skip).collect();

            let documents = self.reconstruct_documents(searcher, doc_addresses)?;
            return Ok((documents, total, facets));
        }

        tracing::warn!(
            "[SORT] No fast column for {}, using fallback in sort_internal",
            fast_field_path
        );
        let fetch_limit = (limit + offset).saturating_mul(3).max(100);
        let collector = TopDocs::with_limit(fetch_limit);

        let (total, prelim_results, facets) = if let Some(fc) = facet_collector {
            let (count, docs, f) = searcher.search(query.as_ref(), &(Count, collector, fc))?;
            (count, docs, f)
        } else {
            let (count, docs) = searcher.search(query.as_ref(), &(Count, collector))?;
            (count, docs, tantivy::collector::FacetCounts::default())
        };

        let sorted_docs =
            self.sort_docs_by_json_field(searcher, prelim_results, field, order, limit, offset)?;
        let documents = self.reconstruct_documents(searcher, sorted_docs)?;

        Ok((documents, total, facets))
    }

    pub(crate) fn sort_docs_by_json_field(
        &self,
        searcher: &Searcher,
        prelim_results: Vec<(tantivy::Score, tantivy::DocAddress)>,
        field: &str,
        order: &SortOrder,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<(f32, tantivy::DocAddress)>> {
        let json_filter_field = self
            .tantivy_schema
            .get_field("_json_filter")
            .map_err(|_| crate::error::FlapjackError::FieldNotFound("_json_filter".to_string()))?;

        let mut scored_docs: Vec<(SortValue, f32, tantivy::DocAddress)> = Vec::new();

        for (score, addr) in prelim_results {
            let doc: tantivy::TantivyDocument = searcher.doc(addr)?;
            let sort_value = if let Some(json_val) = doc.get_first(json_filter_field) {
                let owned: tantivy::schema::OwnedValue = json_val.into();
                self.extract_sort_value_from_json(&owned, field)
            } else {
                SortValue::Missing
            };
            if scored_docs.len() < 5 {
                tracing::trace!(
                    "[SORT_VALUE] objectID extraction: field={} value={:?}",
                    field,
                    sort_value
                );
            }
            scored_docs.push((sort_value, score, addr));
        }

        match order {
            SortOrder::Asc => scored_docs.sort_by(|a, b| a.0.cmp(&b.0)),
            SortOrder::Desc => scored_docs.sort_by(|a, b| b.0.cmp(&a.0)),
        }

        let start = offset.min(scored_docs.len());
        let end = (offset + limit).min(scored_docs.len());

        Ok(scored_docs[start..end]
            .iter()
            .map(|(_, score, addr)| (*score, *addr))
            .collect())
    }

    fn extract_sort_value_from_json(
        &self,
        json: &tantivy::schema::OwnedValue,
        field_path: &str,
    ) -> SortValue {
        let parts: Vec<&str> = field_path.split('.').collect();
        let force_string = field_path == "objectID";
        self.extract_nested_value(json, &parts, force_string)
    }

    fn extract_nested_value(
        &self,
        value: &tantivy::schema::OwnedValue,
        path: &[&str],
        force_string: bool,
    ) -> SortValue {
        if path.is_empty() {
            return match value {
                tantivy::schema::OwnedValue::I64(i) => {
                    if force_string {
                        SortValue::Text(i.to_string())
                    } else {
                        SortValue::Integer(*i)
                    }
                }
                tantivy::schema::OwnedValue::U64(u) => {
                    if force_string {
                        SortValue::Text(u.to_string())
                    } else {
                        SortValue::Integer(*u as i64)
                    }
                }
                tantivy::schema::OwnedValue::F64(f) => SortValue::Float(*f),
                tantivy::schema::OwnedValue::Str(s) => {
                    if force_string {
                        SortValue::Text(s.clone())
                    } else if let Ok(i) = s.parse::<i64>() {
                        SortValue::Integer(i)
                    } else {
                        SortValue::Text(s.clone())
                    }
                }
                tantivy::schema::OwnedValue::Date(dt) => {
                    SortValue::Integer(dt.into_timestamp_secs())
                }
                _ => SortValue::Missing,
            };
        }

        match value {
            tantivy::schema::OwnedValue::Object(pairs) => {
                for (key, val) in pairs {
                    if key == path[0] {
                        return self.extract_nested_value(val, &path[1..], force_string);
                    }
                }
                SortValue::Missing
            }
            _ => SortValue::Missing,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum SortValue {
    Integer(i64),
    Float(f64),
    Text(String),
    Missing,
}

impl Eq for SortValue {}

impl PartialOrd for SortValue {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SortValue {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match (self, other) {
            (SortValue::Missing, SortValue::Missing) => std::cmp::Ordering::Equal,
            (SortValue::Missing, _) => std::cmp::Ordering::Less,
            (_, SortValue::Missing) => std::cmp::Ordering::Greater,
            (SortValue::Integer(a), SortValue::Integer(b)) => a.cmp(b),
            (SortValue::Float(a), SortValue::Float(b)) => {
                a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal)
            }
            (SortValue::Text(a), SortValue::Text(b)) => a.cmp(b),
            (SortValue::Integer(_), _) => std::cmp::Ordering::Less,
            (SortValue::Float(_), SortValue::Text(_)) => std::cmp::Ordering::Less,
            (SortValue::Float(_), SortValue::Integer(_)) => std::cmp::Ordering::Greater,
            (SortValue::Text(_), _) => std::cmp::Ordering::Greater,
        }
    }
}
