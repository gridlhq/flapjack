use crate::error::Result;
use crate::index::document::DocumentConverter;
use crate::index::settings::IndexSettings;
use crate::query::filter::FilterCompiler;
use crate::query::parser::ShortQueryPlaceholder;
use crate::types::{Filter, ScoredDocument, SearchResult};
use std::sync::Arc;
use tantivy::query::{BooleanQuery, BoostQuery, Occur, Query as TantivyQuery, TermQuery};
use tantivy::schema::IndexRecordOption;
use tantivy::Searcher;

/// Type alias: QueryExecutor stores settings as Arc to avoid cloning the
/// full IndexSettings struct on every search (it can be 1+ KB).
type SettingsRef = Option<Arc<IndexSettings>>;

mod facets;
mod relevance;
mod rules;
mod sorting;

pub struct QueryExecutor {
    pub(crate) converter: Arc<DocumentConverter>,
    pub(crate) filter_compiler: FilterCompiler,
    pub(crate) tantivy_schema: tantivy::schema::Schema,
    pub(crate) settings: SettingsRef,
    pub(crate) json_search_field: tantivy::schema::Field,
    pub(crate) searchable_paths: Vec<String>,
    pub(crate) query_text: String,
    pub(crate) max_values_per_facet: Option<usize>,
}

impl QueryExecutor {
    pub fn new(converter: Arc<DocumentConverter>, schema: tantivy::schema::Schema) -> Self {
        let json_search_field = schema
            .get_field("_json_search")
            .expect("_json_search field required");
        QueryExecutor {
            converter,
            filter_compiler: FilterCompiler::new(schema.clone()),
            tantivy_schema: schema,
            settings: None,
            json_search_field,
            searchable_paths: vec![],
            query_text: String::new(),
            max_values_per_facet: None,
        }
    }

    pub fn with_max_values_per_facet(mut self, max: Option<usize>) -> Self {
        self.max_values_per_facet = max;
        self
    }

    pub fn with_settings(mut self, settings: SettingsRef) -> Self {
        if let Some(ref s) = settings {
            if let Some(ref attrs) = s.searchable_attributes {
                self.searchable_paths = attrs.clone();
            }
        }
        self.settings = settings;
        self
    }

    pub fn with_query(mut self, query_text: String) -> Self {
        self.query_text = query_text;
        self
    }

    pub fn execute(
        &self,
        searcher: &Searcher,
        query: Box<dyn TantivyQuery>,
        filter: Option<&Filter>,
        limit: usize,
    ) -> Result<SearchResult> {
        self.execute_with_sort(searcher, query, filter, None, limit, false)
    }

    pub(crate) fn apply_filter(
        &self,
        query: Box<dyn TantivyQuery>,
        filter: Option<&Filter>,
    ) -> Result<Box<dyn TantivyQuery>> {
        if let Some(f) = filter {
            let filter_query = self.filter_compiler.compile(f, self.settings.as_deref())?;
            Ok(Box::new(BooleanQuery::new(vec![
                (Occur::Must, query),
                (Occur::Must, filter_query),
            ])))
        } else {
            Ok(query)
        }
    }

    /// Wraps the query with Should + BoostQuery clauses for optional filters.
    /// Documents matching the optional filters get a score boost; non-matching
    /// documents are NOT excluded from results.
    pub fn apply_optional_boosts(
        &self,
        query: Box<dyn TantivyQuery>,
        specs: &[(String, String, f32)],
    ) -> Result<Box<dyn TantivyQuery>> {
        if specs.is_empty() {
            return Ok(query);
        }
        let json_filter_field = self
            .tantivy_schema
            .get_field("_json_filter")
            .map_err(|_| crate::error::FlapjackError::FieldNotFound("_json_filter".to_string()))?;

        let mut clauses: Vec<(Occur, Box<dyn TantivyQuery>)> = vec![(Occur::Must, query)];

        for (field, value, score) in specs {
            // Build a term query on _json_filter.{field} for the value
            let term_text = format!("{}\0s{}", field, value.to_lowercase());
            let term = tantivy::Term::from_field_text(json_filter_field, &term_text);
            let term_query: Box<dyn TantivyQuery> =
                Box::new(TermQuery::new(term, IndexRecordOption::Basic));
            let boosted: Box<dyn TantivyQuery> = if *score != 1.0 {
                Box::new(BoostQuery::new(term_query, *score))
            } else {
                term_query
            };
            clauses.push((Occur::Should, boosted));
        }

        Ok(Box::new(BooleanQuery::new(clauses)))
    }

    // Expands short queries (≤2 chars) by enumerating matching terms from the index.
    // Recursively handles nested BooleanQueries containing ShortQueryPlaceholders.
    pub(crate) fn expand_short_query_with_searcher(
        &self,
        query: Box<dyn TantivyQuery>,
        searcher: &Searcher,
    ) -> Result<Box<dyn TantivyQuery>> {
        let query_any = query.as_any();

        if let Some(placeholder) = query_any.downcast_ref::<ShortQueryPlaceholder>() {
            return self.expand_placeholder(placeholder, searcher);
        }

        if let Some(bool_query) = query_any.downcast_ref::<BooleanQuery>() {
            let clauses = bool_query.clauses();
            let mut new_clauses: Vec<(Occur, Box<dyn TantivyQuery>)> = Vec::new();
            let mut changed = false;

            for (occur, sub_query) in clauses {
                if sub_query.as_any().is::<ShortQueryPlaceholder>() {
                    let placeholder = sub_query
                        .as_any()
                        .downcast_ref::<ShortQueryPlaceholder>()
                        .unwrap();
                    let expanded = self.expand_placeholder(placeholder, searcher)?;
                    new_clauses.push((*occur, expanded));
                    changed = true;
                } else if sub_query.as_any().is::<BooleanQuery>() {
                    let expanded = self.expand_short_query_with_searcher(
                        Box::new(
                            sub_query
                                .as_any()
                                .downcast_ref::<BooleanQuery>()
                                .unwrap()
                                .clone(),
                        ),
                        searcher,
                    )?;
                    new_clauses.push((*occur, expanded));
                    changed = true;
                } else {
                    new_clauses.push((*occur, sub_query.box_clone()));
                }
            }

            if changed {
                return Ok(Box::new(BooleanQuery::new(new_clauses)));
            }
        }

        Ok(query)
    }

    fn expand_placeholder(
        &self,
        placeholder: &ShortQueryPlaceholder,
        searcher: &Searcher,
    ) -> Result<Box<dyn TantivyQuery>> {
        let marker = &placeholder.marker;
        let mut term_queries: Vec<(Occur, Box<dyn TantivyQuery>)> = Vec::new();

        if let Some(segment) = searcher.segment_readers().first() {
            let inv_index = segment.inverted_index(marker.field)?;

            // Limit searchable paths and terms-per-path for short queries to
            // keep the resulting BooleanQuery manageable.  With edge_ngram
            // indexing, 1-char terms like "m" exist for every word starting
            // with 'm' — extremely high document frequency.  Use tighter caps
            // for 1-char queries (3 paths × 20 terms = 60 clauses) vs 2-char
            // (5 paths × 50 terms = 250 clauses).
            let is_single_char = marker.token.chars().count() == 1;
            let max_paths = if is_single_char { 3 } else { 5 }.min(marker.paths.len());
            let max_terms_per_path: usize = if is_single_char { 20 } else { 50 };
            for (path_idx, path) in marker.paths.iter().take(max_paths).enumerate() {
                let weight = marker.weights.get(path_idx).copied().unwrap_or(1.0);
                let prefix_bytes = format!("{}\0s{}", path, marker.token).into_bytes();
                let mut upper_bound = prefix_bytes.clone();
                upper_bound.push(0xFF);
                let mut terms = inv_index
                    .terms()
                    .range()
                    .ge(&prefix_bytes)
                    .lt(&upper_bound)
                    .into_stream()?;
                let mut count = 0;

                while terms.advance() && count < max_terms_per_path {
                    let term_bytes = terms.key();
                    let term = tantivy::Term::from_field_bytes(marker.field, term_bytes);
                    let term_query: Box<dyn TantivyQuery> = Box::new(TermQuery::new(
                        term,
                        IndexRecordOption::WithFreqsAndPositions,
                    ));
                    let boosted: Box<dyn TantivyQuery> = if weight != 1.0 {
                        Box::new(tantivy::query::BoostQuery::new(term_query, weight))
                    } else {
                        term_query
                    };
                    term_queries.push((Occur::Should, boosted));
                    count += 1;
                }
            }
        }

        if term_queries.is_empty() {
            Ok(Box::new(tantivy::query::EmptyQuery))
        } else {
            Ok(Box::new(BooleanQuery::new(term_queries)))
        }
    }

    pub(crate) fn reconstruct_documents(
        &self,
        searcher: &Searcher,
        doc_addresses: Vec<(f32, tantivy::DocAddress)>,
    ) -> Result<Vec<ScoredDocument>> {
        let mut documents = Vec::new();
        for (score, doc_address) in doc_addresses {
            let tantivy_doc = searcher.doc(doc_address)?;
            let document =
                self.converter
                    .from_tantivy(tantivy_doc, &self.tantivy_schema, String::new())?;
            documents.push(ScoredDocument { document, score });
        }
        Ok(documents)
    }

    pub(crate) fn build_result(
        &self,
        documents: Vec<ScoredDocument>,
        total: usize,
    ) -> SearchResult {
        SearchResult {
            documents,
            total,
            facets: std::collections::HashMap::new(),
            user_data: Vec::new(),
            applied_rules: Vec::new(),
        }
    }
}
