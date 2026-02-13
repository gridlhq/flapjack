use crate::error::Result;
use crate::types::Query;

fn is_cjk(c: char) -> bool {
    matches!(c,
        '\u{4E00}'..='\u{9FFF}' |
        '\u{3400}'..='\u{4DBF}' |
        '\u{F900}'..='\u{FAFF}' |
        '\u{2E80}'..='\u{2EFF}' |
        '\u{3000}'..='\u{303F}' |
        '\u{3040}'..='\u{309F}' |
        '\u{30A0}'..='\u{30FF}' |
        '\u{31F0}'..='\u{31FF}' |
        '\u{AC00}'..='\u{D7AF}' |
        '\u{1100}'..='\u{11FF}' |
        '\u{20000}'..='\u{2A6DF}' |
        '\u{2A700}'..='\u{2B73F}' |
        '\u{2B740}'..='\u{2B81F}' |
        '\u{2B820}'..='\u{2CEAF}'
    )
}

fn split_cjk_aware(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    for c in text.chars() {
        if is_cjk(c) {
            if !current.is_empty() {
                tokens.push(std::mem::take(&mut current));
            }
            tokens.push(c.to_string());
        } else if c.is_alphanumeric() {
            current.push(c);
        } else if !current.is_empty() {
            tokens.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}
use tantivy::query::{Query as TantivyQuery, Scorer, Weight};
use tantivy::schema::Schema as TantivySchema;
use tantivy::DocSet;

#[derive(Debug, Clone)]
pub struct ShortQueryPlaceholder {
    pub marker: ShortQueryMarker,
}

impl TantivyQuery for ShortQueryPlaceholder {
    fn weight(
        &self,
        _enable_scoring: tantivy::query::EnableScoring,
    ) -> tantivy::Result<Box<dyn Weight>> {
        Ok(Box::new(ShortQueryWeight))
    }
}

struct ShortQueryWeight;

impl Weight for ShortQueryWeight {
    fn scorer(
        &self,
        _reader: &tantivy::SegmentReader,
        _boost: tantivy::Score,
    ) -> tantivy::Result<Box<dyn Scorer>> {
        Ok(Box::new(EmptyScorer))
    }

    fn explain(
        &self,
        _reader: &tantivy::SegmentReader,
        _doc: tantivy::DocId,
    ) -> tantivy::Result<tantivy::query::Explanation> {
        Ok(tantivy::query::Explanation::new(
            "ShortQueryPlaceholder",
            0.0,
        ))
    }
}

struct EmptyScorer;

impl DocSet for EmptyScorer {
    fn advance(&mut self) -> tantivy::DocId {
        tantivy::TERMINATED
    }

    fn doc(&self) -> tantivy::DocId {
        tantivy::TERMINATED
    }

    fn size_hint(&self) -> u32 {
        0
    }
}

impl Scorer for EmptyScorer {
    fn score(&mut self) -> tantivy::Score {
        0.0
    }
}

pub struct QueryParser {
    fields: Vec<tantivy::schema::Field>,
    json_exact_field: Option<tantivy::schema::Field>,
    weights: Vec<f32>,
    searchable_paths: Vec<String>,
    query_type: String,
    plural_map: Option<std::collections::HashMap<String, Vec<String>>>,
    typo_tolerance: bool,
    min_word_size_for_1_typo: usize,
    advanced_syntax: bool,
}

#[derive(Debug, Clone)]
pub struct ShortQueryMarker {
    pub token: String,
    pub paths: Vec<String>,
    pub weights: Vec<f32>,
    pub field: tantivy::schema::Field,
}

impl QueryParser {
    pub fn new(_schema: &TantivySchema, default_fields: Vec<tantivy::schema::Field>) -> Self {
        let weights = vec![1.0; default_fields.len()];
        QueryParser {
            fields: default_fields,
            json_exact_field: None,
            weights,
            searchable_paths: vec![],
            query_type: "prefixLast".to_string(),
            plural_map: None,
            typo_tolerance: true,
            min_word_size_for_1_typo: 4,
            advanced_syntax: false,
        }
    }

    pub fn new_with_weights(
        _schema: &TantivySchema,
        fields: Vec<tantivy::schema::Field>,
        weights: Vec<f32>,
        searchable_paths: Vec<String>,
    ) -> Self {
        assert_eq!(
            weights.len(),
            searchable_paths.len(),
            "Weights and searchable_paths must match"
        );
        QueryParser {
            fields,
            json_exact_field: None,
            weights,
            searchable_paths,
            query_type: "prefixLast".to_string(),
            plural_map: None,
            typo_tolerance: true,
            min_word_size_for_1_typo: 4,
            advanced_syntax: false,
        }
    }

    pub fn with_exact_field(mut self, field: tantivy::schema::Field) -> Self {
        self.json_exact_field = Some(field);
        self
    }

    pub fn with_query_type(mut self, query_type: &str) -> Self {
        self.query_type = query_type.to_string();
        self
    }

    pub fn with_typo_tolerance(mut self, enabled: bool) -> Self {
        self.typo_tolerance = enabled;
        self
    }

    pub fn with_min_word_size_for_1_typo(mut self, size: usize) -> Self {
        self.min_word_size_for_1_typo = size;
        self
    }

    pub fn with_advanced_syntax(mut self, enabled: bool) -> Self {
        self.advanced_syntax = enabled;
        self
    }

    pub fn with_plural_map(
        mut self,
        plural_map: Option<std::collections::HashMap<String, Vec<String>>>,
    ) -> Self {
        self.plural_map = plural_map;
        self
    }

    pub fn parse(&self, query: &Query) -> Result<Box<dyn TantivyQuery>> {
        // Advanced syntax: extract "phrases" and -exclusions before normal parsing
        if self.advanced_syntax {
            let (phrases, exclusions, remaining) = Self::preprocess_advanced_syntax(&query.text);
            if !phrases.is_empty() || !exclusions.is_empty() {
                return self.parse_with_advanced_syntax(
                    &remaining,
                    &phrases,
                    &exclusions,
                    query.text.ends_with(' '),
                );
            }
        }

        let has_trailing_space = query.text.ends_with(' ');
        let text = query.text.to_lowercase().trim_end_matches('*').to_string();
        let tokens: Vec<String> = split_cjk_aware(&text);

        tracing::trace!(
            "[PARSER] parse() called: query='{}', tokens={:?}, searchable_paths={:?}",
            query.text,
            tokens,
            self.searchable_paths
        );

        if tokens.is_empty() {
            return Ok(Box::new(tantivy::query::AllQuery));
        }

        // SHORT QUERY PATH: Single token ≤2 chars uses prefix enumeration
        // BUT: if trailing space, treat as exact match (no prefix)
        if tokens.len() == 1 && tokens[0].chars().count() <= 2 {
            tracing::trace!(
                "[PARSER] Short query detected: token={}, char_count={}, has_trailing_space={}",
                tokens[0],
                tokens[0].chars().count(),
                has_trailing_space
            );

            if has_trailing_space {
                // Trailing space = exact match required, use _json_exact field
                // For short tokens with trailing space, this likely returns 0 results (intended)
                let target_field = self.json_exact_field.unwrap_or(self.fields[0]);
                let mut field_queries: Vec<(tantivy::query::Occur, Box<dyn TantivyQuery>)> =
                    Vec::new();

                for (path_idx, path) in self.searchable_paths.iter().enumerate() {
                    let term_text = format!("{}\0s{}", path, tokens[0]);
                    let term = tantivy::Term::from_field_text(target_field, &term_text);
                    let token_query: Box<dyn TantivyQuery> =
                        Box::new(tantivy::query::TermQuery::new(
                            term,
                            tantivy::schema::IndexRecordOption::WithFreqsAndPositions,
                        ));
                    let weight = if path_idx < self.weights.len() {
                        self.weights[path_idx]
                    } else {
                        1.0
                    };
                    let boosted_query: Box<dyn TantivyQuery> = if weight != 1.0 {
                        Box::new(tantivy::query::BoostQuery::new(token_query, weight))
                    } else {
                        token_query
                    };
                    field_queries.push((tantivy::query::Occur::Should, boosted_query));
                }

                return Ok(Box::new(tantivy::query::BooleanQuery::new(field_queries)));
            }

            let marker = ShortQueryMarker {
                token: tokens[0].to_string(),
                paths: self.searchable_paths.clone(),
                weights: self.weights.clone(),
                field: self.fields[0],
            };
            tracing::trace!(
                "[PARSER] Creating placeholder with {} paths",
                self.searchable_paths.len()
            );
            return Ok(Box::new(ShortQueryPlaceholder { marker }));
        }

        tracing::trace!(
            "QueryParser: query='{}', searchable_paths={:?}, weights={:?}, tokens={:?}",
            query.text,
            self.searchable_paths,
            self.weights,
            tokens
        );

        let json_search_field = self.fields[0];
        let mut word_queries: Vec<(tantivy::query::Occur, Box<dyn TantivyQuery>)> = Vec::new();

        // Limit fuzzy matching to top N paths for multi-word queries to keep
        // the total number of expensive FuzzyTermQuery evaluations manageable.
        // Cap fuzzy matching to a small number of top searchable paths.
        // FuzzyTermQuery builds a Levenshtein automaton per term×path — expensive.
        // For single-word queries like "action", running fuzzy on all 10+ paths
        // was causing 179ms vs Algolia's 83ms.
        let max_fuzzy_paths = if tokens.len() >= 3 {
            2.min(self.searchable_paths.len())
        } else {
            4.min(self.searchable_paths.len())
        };

        let last_idx = tokens.len() - 1;
        for (token_idx, token) in tokens.iter().enumerate() {
            let is_last = token_idx == last_idx;
            let is_prefix = match self.query_type.as_str() {
                "prefixAll" => true,
                "prefixNone" => false,
                _ => is_last && !has_trailing_space,
            };

            tracing::trace!(
                "[PARSER] token='{}' len={} is_last={} is_prefix={} query_type={}",
                token,
                token.len(),
                is_last,
                is_prefix,
                self.query_type
            );

            if token.chars().count() <= 2 && is_prefix {
                let marker = ShortQueryMarker {
                    token: token.to_string(),
                    paths: self.searchable_paths.clone(),
                    weights: self.weights.clone(),
                    field: self.fields[0],
                };
                word_queries.push((
                    tantivy::query::Occur::Must,
                    Box::new(ShortQueryPlaceholder { marker }),
                ));
                continue;
            }

            let mut field_queries: Vec<(tantivy::query::Occur, Box<dyn TantivyQuery>)> = Vec::new();

            let target_field = if is_prefix {
                json_search_field
            } else {
                self.json_exact_field.unwrap_or(json_search_field)
            };

            let plural_forms: Vec<String> = self
                .plural_map
                .as_ref()
                .and_then(|m| m.get(token.as_str()))
                .map(|forms| {
                    forms
                        .iter()
                        .filter(|f| f.as_str() != token.as_str())
                        .cloned()
                        .collect()
                })
                .unwrap_or_default();

            for (path_idx, path) in self.searchable_paths.iter().enumerate() {
                let term_text = format!("{}\0s{}", path, token);
                let term = tantivy::Term::from_field_text(target_field, &term_text);

                let distance = if self.typo_tolerance
                    && token.len() >= self.min_word_size_for_1_typo
                    && path_idx < max_fuzzy_paths
                {
                    1
                } else {
                    0
                };
                tracing::trace!(
                    "[PARSER] token='{}' path='{}' is_prefix={} field={:?}",
                    token,
                    path,
                    is_prefix,
                    if is_prefix { "search" } else { "exact" }
                );

                let token_query: Box<dyn TantivyQuery> = if distance > 0 {
                    let exact = Box::new(tantivy::query::TermQuery::new(
                        term.clone(),
                        tantivy::schema::IndexRecordOption::WithFreqsAndPositions,
                    ));
                    // For prefix queries, run fuzzy on _json_exact (simple tokenizer)
                    // instead of _json_search (edge_ngram). The prefix match is already
                    // handled by the TermQuery above on the n-gram index. Running the
                    // Levenshtein automaton on the n-gram index traverses ~8x more terms
                    // (every word generates ~8 n-gram terms) for no benefit.
                    let fuzzy_term = if is_prefix {
                        let fuzzy_field = self.json_exact_field.unwrap_or(target_field);
                        tantivy::Term::from_field_text(fuzzy_field, &term_text)
                    } else {
                        term
                    };
                    let fuzzy = Box::new(tantivy::query::FuzzyTermQuery::new(
                        fuzzy_term, distance, true,
                    ));
                    let mut clauses: Vec<(tantivy::query::Occur, Box<dyn TantivyQuery>)> = vec![
                        (
                            tantivy::query::Occur::Should,
                            exact as Box<dyn TantivyQuery>,
                        ),
                        (
                            tantivy::query::Occur::Should,
                            fuzzy as Box<dyn TantivyQuery>,
                        ),
                    ];
                    // First-char error fallback: strip the first character and
                    // prefix-match the remainder on the n-gram index.  Algolia
                    // handles first-character typos this way — e.g. "lsha" →
                    // "sha" matches "shades".
                    if is_prefix && token.len() >= 4 {
                        let stripped =
                            &token[token.char_indices().nth(1).map(|(i, _)| i).unwrap_or(1)..];
                        if stripped.len() >= 3 {
                            let stripped_term_text = format!("{}\0s{}", path, stripped);
                            let stripped_term = tantivy::Term::from_field_text(
                                json_search_field,
                                &stripped_term_text,
                            );
                            let stripped_q: Box<dyn TantivyQuery> =
                                Box::new(tantivy::query::TermQuery::new(
                                    stripped_term,
                                    tantivy::schema::IndexRecordOption::WithFreqsAndPositions,
                                ));
                            clauses.push((tantivy::query::Occur::Should, stripped_q));
                        }
                    }
                    Box::new(tantivy::query::BooleanQuery::new(clauses))
                } else {
                    Box::new(tantivy::query::TermQuery::new(
                        term,
                        tantivy::schema::IndexRecordOption::WithFreqsAndPositions,
                    ))
                };

                let token_query: Box<dyn TantivyQuery> = if !plural_forms.is_empty() {
                    let mut plural_clauses: Vec<(tantivy::query::Occur, Box<dyn TantivyQuery>)> =
                        vec![(tantivy::query::Occur::Should, token_query)];
                    for plural in &plural_forms {
                        let plural_term_text = format!("{}\0s{}", path, plural);
                        let plural_term =
                            tantivy::Term::from_field_text(target_field, &plural_term_text);
                        let plural_q: Box<dyn TantivyQuery> =
                            Box::new(tantivy::query::TermQuery::new(
                                plural_term,
                                tantivy::schema::IndexRecordOption::WithFreqsAndPositions,
                            ));
                        plural_clauses.push((tantivy::query::Occur::Should, plural_q));
                    }
                    Box::new(tantivy::query::BooleanQuery::new(plural_clauses))
                } else {
                    token_query
                };

                let weight = if path_idx < self.weights.len() {
                    self.weights[path_idx]
                } else {
                    1.0
                };
                let boosted_query: Box<dyn TantivyQuery> = if weight != 1.0 {
                    Box::new(tantivy::query::BoostQuery::new(token_query, weight))
                } else {
                    token_query
                };

                field_queries.push((tantivy::query::Occur::Should, boosted_query));
            }

            word_queries.push((
                tantivy::query::Occur::Must,
                Box::new(tantivy::query::BooleanQuery::new(field_queries)),
            ));
        }

        Ok(Box::new(tantivy::query::BooleanQuery::new(word_queries)))
    }

    pub fn fields(&self) -> &[tantivy::schema::Field] {
        &self.fields
    }

    pub fn extract_terms(&self, query: &Query) -> Vec<String> {
        split_cjk_aware(&query.text.to_lowercase())
            .into_iter()
            .map(|s| s.trim_matches(|c: char| !c.is_alphanumeric()).to_string())
            .filter(|s| !s.is_empty())
            .collect()
    }

    /// Extract "quoted phrases" and -exclusion terms from query text.
    fn preprocess_advanced_syntax(text: &str) -> (Vec<String>, Vec<String>, String) {
        let mut phrases = Vec::new();
        let mut exclusions = Vec::new();
        let mut remaining = String::new();

        let mut chars = text.chars().peekable();
        while let Some(&c) = chars.peek() {
            if c == '"' {
                chars.next();
                let mut phrase = String::new();
                while let Some(&nc) = chars.peek() {
                    if nc == '"' {
                        chars.next();
                        break;
                    }
                    phrase.push(nc);
                    chars.next();
                }
                let trimmed = phrase.trim().to_string();
                if !trimmed.is_empty() {
                    phrases.push(trimmed);
                }
            } else if c == '-' && (remaining.is_empty() || remaining.ends_with(' ')) {
                chars.next();
                let mut word = String::new();
                while let Some(&nc) = chars.peek() {
                    if nc.is_whitespace() {
                        break;
                    }
                    word.push(nc);
                    chars.next();
                }
                if !word.is_empty() {
                    exclusions.push(word);
                }
            } else {
                remaining.push(c);
                chars.next();
            }
        }
        (phrases, exclusions, remaining.trim().to_string())
    }

    /// Build a query combining phrases (Must), exclusions (MustNot), and remaining text.
    fn parse_with_advanced_syntax(
        &self,
        remaining_text: &str,
        phrases: &[String],
        exclusions: &[String],
        _has_trailing_space: bool,
    ) -> Result<Box<dyn TantivyQuery>> {
        let json_search_field = self.fields[0];
        let exact_field = self.json_exact_field.unwrap_or(json_search_field);

        let mut clauses: Vec<(tantivy::query::Occur, Box<dyn TantivyQuery>)> = Vec::new();

        // Parse remaining text as a normal query
        if !remaining_text.trim().is_empty() {
            let sub_query = Query {
                text: remaining_text.to_string(),
            };
            // Temporarily disable advanced_syntax to avoid recursion
            let normal_parser = QueryParser {
                advanced_syntax: false,
                ..self.clone_parser()
            };
            if let Ok(q) = normal_parser.parse(&sub_query) {
                clauses.push((tantivy::query::Occur::Must, q));
            }
        }

        // Phrase queries: all words in the phrase must match (exact, on same field paths)
        for phrase in phrases {
            let words: Vec<String> = phrase
                .to_lowercase()
                .split_whitespace()
                .map(|s| s.to_string())
                .collect();
            if words.is_empty() {
                continue;
            }
            let mut phrase_clauses: Vec<(tantivy::query::Occur, Box<dyn TantivyQuery>)> =
                Vec::new();
            for word in &words {
                let mut field_queries: Vec<(tantivy::query::Occur, Box<dyn TantivyQuery>)> =
                    Vec::new();
                for (path_idx, path) in self.searchable_paths.iter().enumerate() {
                    let term_text = format!("{}\0s{}", path, word);
                    let term = tantivy::Term::from_field_text(exact_field, &term_text);
                    let tq: Box<dyn TantivyQuery> = Box::new(tantivy::query::TermQuery::new(
                        term,
                        tantivy::schema::IndexRecordOption::WithFreqs,
                    ));
                    let weight = self.weights.get(path_idx).copied().unwrap_or(1.0);
                    field_queries.push((
                        tantivy::query::Occur::Should,
                        Box::new(tantivy::query::BoostQuery::new(tq, weight)),
                    ));
                }
                phrase_clauses.push((
                    tantivy::query::Occur::Must,
                    Box::new(tantivy::query::BooleanQuery::new(field_queries)),
                ));
            }
            clauses.push((
                tantivy::query::Occur::Must,
                Box::new(tantivy::query::BooleanQuery::new(phrase_clauses)),
            ));
        }

        // Exclusion queries: MustNot for each excluded term
        for exclusion in exclusions {
            let word = exclusion.to_lowercase();
            let mut field_queries: Vec<(tantivy::query::Occur, Box<dyn TantivyQuery>)> = Vec::new();
            for path in &self.searchable_paths {
                let term_text = format!("{}\0s{}", path, word);
                let term = tantivy::Term::from_field_text(exact_field, &term_text);
                let tq: Box<dyn TantivyQuery> = Box::new(tantivy::query::TermQuery::new(
                    term,
                    tantivy::schema::IndexRecordOption::WithFreqs,
                ));
                field_queries.push((tantivy::query::Occur::Should, tq));
            }
            clauses.push((
                tantivy::query::Occur::MustNot,
                Box::new(tantivy::query::BooleanQuery::new(field_queries)),
            ));
        }

        if clauses.is_empty() {
            return Ok(Box::new(tantivy::query::AllQuery));
        }

        Ok(Box::new(tantivy::query::BooleanQuery::new(clauses)))
    }

    /// Clone parser fields without the Clone trait (for recursion avoidance)
    fn clone_parser(&self) -> QueryParser {
        QueryParser {
            fields: self.fields.clone(),
            json_exact_field: self.json_exact_field,
            weights: self.weights.clone(),
            searchable_paths: self.searchable_paths.clone(),
            query_type: self.query_type.clone(),
            plural_map: self.plural_map.clone(),
            typo_tolerance: self.typo_tolerance,
            min_word_size_for_1_typo: self.min_word_size_for_1_typo,
            advanced_syntax: self.advanced_syntax,
        }
    }
}
