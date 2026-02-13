use crate::types::{Document, FieldValue};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HighlightResult {
    pub value: String,
    pub match_level: MatchLevel,
    pub matched_words: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fully_highlighted: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MatchLevel {
    None,
    Partial,
    Full,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum HighlightValue {
    Single(HighlightResult),
    Array(Vec<HighlightResult>),
    Object(HashMap<String, HighlightValue>),
}

pub struct Highlighter {
    pre_tag: String,
    post_tag: String,
}

impl Default for Highlighter {
    fn default() -> Self {
        Self {
            pre_tag: "<em>".to_string(),
            post_tag: "</em>".to_string(),
        }
    }
}

impl Highlighter {
    pub fn new(pre_tag: String, post_tag: String) -> Self {
        Self { pre_tag, post_tag }
    }

    pub fn highlight_document(
        &self,
        doc: &Document,
        query_words: &[String],
        searchable_paths: &[String],
    ) -> HashMap<String, HighlightValue> {
        let mut result = HashMap::new();

        for (field_name, field_value) in &doc.fields {
            if field_name == "objectID" {
                continue;
            }
            // Algolia highlights ALL attributes with query words, not just
            // searchable ones.  The searchableAttributes setting only controls
            // which fields the *search* engine queries, not highlighting.
            result.insert(
                field_name.clone(),
                self.highlight_field_value(field_value, query_words, field_name, searchable_paths),
            );
        }

        result
    }

    fn highlight_field_value(
        &self,
        value: &FieldValue,
        query_words: &[String],
        field_path: &str,
        searchable_paths: &[String],
    ) -> HighlightValue {
        match value {
            FieldValue::Text(s) => HighlightValue::Single(self.highlight_text(s, query_words)),
            FieldValue::Array(items) => {
                let results: Vec<HighlightResult> = items
                    .iter()
                    .map(|item| match item {
                        FieldValue::Text(s) => self.highlight_text(s, query_words),
                        _ => self.no_match(self.field_value_to_string(item)),
                    })
                    .collect();
                HighlightValue::Array(results)
            }
            FieldValue::Object(map) => {
                let mut obj_result = HashMap::new();
                for (k, v) in map {
                    let nested_path = format!("{}.{}", field_path, k);
                    obj_result.insert(
                        k.clone(),
                        self.highlight_field_value(v, query_words, &nested_path, searchable_paths),
                    );
                }
                HighlightValue::Object(obj_result)
            }
            _ => HighlightValue::Single(self.no_match(self.field_value_to_string(value))),
        }
    }

    fn highlight_text(&self, text: &str, query_words: &[String]) -> HighlightResult {
        let text_lower = text.to_lowercase();
        let mut matched_words = Vec::new();
        let mut match_positions = Vec::new();

        // Pre-compute lowercased query words to avoid repeated allocation
        let query_words_lower: Vec<String> = query_words.iter().map(|w| w.to_lowercase()).collect();

        // 1. Exact substring matching for each query word
        for (qi, word_lower) in query_words_lower.iter().enumerate() {
            let mut start = 0;
            while let Some(pos) = text_lower[start..].find(word_lower.as_str()) {
                let absolute_pos = start + pos;
                matched_words.push(query_words[qi].clone());
                match_positions.push((absolute_pos, absolute_pos + word_lower.len()));
                start = absolute_pos + word_lower.len();
            }
        }

        // Check if exact matching already found all query words — if so,
        // skip the expensive split/concat/fuzzy matching.
        let unique_matched: std::collections::HashSet<&str> =
            matched_words.iter().map(|w| w.as_str()).collect();
        let all_found_exact = query_words
            .iter()
            .all(|qw| unique_matched.contains(qw.as_str()));

        if !all_found_exact {
            // 2. Split matching: for each query word >= 4 chars, try inserting a space
            //    at each position to match split forms (e.g., "hotdog" -> "hot dog")
            for (qi, word_lower) in query_words_lower.iter().enumerate() {
                let chars: Vec<char> = word_lower.chars().collect();
                if chars.len() < 4 {
                    continue;
                }
                for split_pos in 2..chars.len().saturating_sub(1) {
                    let first: String = chars[..split_pos].iter().collect();
                    let second: String = chars[split_pos..].iter().collect();
                    if second.len() < 2 {
                        continue;
                    }
                    let split_form = format!("{} {}", first, second);
                    let mut start = 0;
                    while let Some(pos) = text_lower[start..].find(&split_form) {
                        let absolute_pos = start + pos;
                        matched_words.push(query_words[qi].clone());
                        match_positions.push((absolute_pos, absolute_pos + split_form.len()));
                        start = absolute_pos + split_form.len();
                    }
                }
            }

            // 3. Concat matching: for adjacent query word pairs, try concatenated form
            //    (e.g., "ear" + "buds" -> try matching "earbuds" in text)
            if query_words_lower.len() >= 2 {
                for i in 0..query_words_lower.len() - 1 {
                    let concat = format!("{}{}", query_words_lower[i], query_words_lower[i + 1]);
                    let mut start = 0;
                    while let Some(pos) = text_lower[start..].find(&concat) {
                        let absolute_pos = start + pos;
                        matched_words.push(query_words[i].clone());
                        matched_words.push(query_words[i + 1].clone());
                        match_positions.push((absolute_pos, absolute_pos + concat.len()));
                        start = absolute_pos + concat.len();
                    }
                }
            }

            // 4. Fuzzy matching per word boundary (most expensive — only when needed)
            let text_words: Vec<(usize, &str)> = {
                let mut words = Vec::new();
                let mut current_start = 0;
                for (idx, ch) in text.char_indices() {
                    if !ch.is_alphanumeric() {
                        if current_start < idx {
                            words.push((current_start, &text[current_start..idx]));
                        }
                        current_start = idx + ch.len_utf8();
                    }
                }
                if current_start < text.len() {
                    words.push((current_start, &text[current_start..]));
                }
                words
            };

            for (word_start, text_word) in &text_words {
                let text_word_lower = text_word.to_lowercase();
                for (qi, query_lower) in query_words_lower.iter().enumerate() {
                    let ql_chars = query_lower.chars().count();
                    let twl_chars = text_word_lower.chars().count();
                    if ql_chars >= 4 && twl_chars >= 4 {
                        let distance = strsim::damerau_levenshtein(query_lower, &text_word_lower);
                        let max_distance = if ql_chars >= 8 { 2 } else { 1 };
                        if distance <= max_distance && distance > 0 {
                            matched_words.push(query_words[qi].clone());
                            let highlight_len = ql_chars.min(text_word.len());
                            match_positions.push((*word_start, word_start + highlight_len));
                        } else if twl_chars > ql_chars {
                            let prefix: String = text_word_lower.chars().take(ql_chars).collect();
                            let prefix_distance = strsim::damerau_levenshtein(query_lower, &prefix);
                            if prefix_distance <= max_distance {
                                matched_words.push(query_words[qi].clone());
                                let highlight_end = text_word
                                    .char_indices()
                                    .nth(ql_chars)
                                    .map(|(i, _)| word_start + i)
                                    .unwrap_or(word_start + text_word.len());
                                match_positions.push((*word_start, highlight_end));
                            }
                        }
                        if ql_chars >= 4 {
                            let query_suffix: String = query_lower.chars().skip(1).collect();
                            let suffix_len = query_suffix.chars().count();
                            if twl_chars >= suffix_len && suffix_len >= 3 {
                                let text_prefix: String =
                                    text_word_lower.chars().take(suffix_len).collect();
                                let suffix_distance =
                                    strsim::damerau_levenshtein(&query_suffix, &text_prefix);
                                if suffix_distance <= 1 {
                                    matched_words.push(query_words[qi].clone());
                                    let highlight_end = text_word
                                        .char_indices()
                                        .nth(suffix_len)
                                        .map(|(i, _)| word_start + i)
                                        .unwrap_or(word_start + text_word.len());
                                    match_positions.push((*word_start, highlight_end));
                                }
                            }
                        }
                    }
                }
            }
        }

        if matched_words.is_empty() {
            return self.no_match(text.to_string());
        }

        // Merge overlapping/adjacent positions into single spans
        match_positions.sort_by_key(|(start, _)| *start);
        match_positions.dedup();
        let match_positions = Self::merge_positions(match_positions);

        let highlighted = self.apply_highlights(text, &match_positions);

        let unique_matched: std::collections::HashSet<_> = matched_words.iter().collect();
        let match_level = if unique_matched.len() == query_words.len() {
            MatchLevel::Full
        } else {
            MatchLevel::Partial
        };

        let total_match_len: usize = match_positions.iter().map(|(s, e)| e - s).sum();
        let fully_highlighted = Some(total_match_len >= text.len());

        matched_words.sort();
        matched_words.dedup();

        HighlightResult {
            value: highlighted,
            match_level,
            matched_words,
            fully_highlighted,
        }
    }

    /// Merge overlapping or adjacent positions into single spans.
    fn merge_positions(positions: Vec<(usize, usize)>) -> Vec<(usize, usize)> {
        if positions.is_empty() {
            return positions;
        }
        let mut merged: Vec<(usize, usize)> = Vec::new();
        let mut current = positions[0];
        for &(start, end) in &positions[1..] {
            if start <= current.1 {
                current.1 = current.1.max(end);
            } else {
                merged.push(current);
                current = (start, end);
            }
        }
        merged.push(current);
        merged
    }

    fn apply_highlights(&self, text: &str, positions: &[(usize, usize)]) -> String {
        if positions.is_empty() {
            return text.to_string();
        }

        let mut result = String::new();
        let mut last_end = 0;

        for &(start, end) in positions {
            if start < last_end {
                continue;
            }

            result.push_str(&text[last_end..start]);
            result.push_str(&self.pre_tag);
            result.push_str(&text[start..end]);
            result.push_str(&self.post_tag);
            last_end = end;
        }

        result.push_str(&text[last_end..]);
        result
    }

    /// Generate a snippet for a document — truncated text around matches.
    pub fn snippet_document(
        &self,
        doc: &Document,
        query_words: &[String],
        snippet_specs: &[(&str, usize)],
    ) -> HashMap<String, SnippetValue> {
        let mut result = HashMap::new();
        for (attr, word_count) in snippet_specs {
            if *attr == "*" {
                // Snippet all text fields
                for (field_name, field_value) in &doc.fields {
                    if field_name == "objectID" {
                        continue;
                    }
                    result.insert(
                        field_name.clone(),
                        self.snippet_field_value(field_value, query_words, *word_count),
                    );
                }
            } else if let Some(field_value) = doc.fields.get(*attr) {
                result.insert(
                    attr.to_string(),
                    self.snippet_field_value(field_value, query_words, *word_count),
                );
            }
        }
        result
    }

    fn snippet_field_value(
        &self,
        value: &FieldValue,
        query_words: &[String],
        word_count: usize,
    ) -> SnippetValue {
        match value {
            FieldValue::Text(s) => {
                SnippetValue::Single(self.snippet_text(s, query_words, word_count))
            }
            FieldValue::Array(items) => {
                let results: Vec<SnippetResult> = items
                    .iter()
                    .map(|item| match item {
                        FieldValue::Text(s) => self.snippet_text(s, query_words, word_count),
                        _ => SnippetResult {
                            value: self.field_value_to_string(item),
                            match_level: MatchLevel::None,
                        },
                    })
                    .collect();
                SnippetValue::Array(results)
            }
            FieldValue::Object(map) => {
                let mut obj_result = HashMap::new();
                for (k, v) in map {
                    obj_result.insert(
                        k.clone(),
                        self.snippet_field_value(v, query_words, word_count),
                    );
                }
                SnippetValue::Object(obj_result)
            }
            _ => SnippetValue::Single(SnippetResult {
                value: self.field_value_to_string(value),
                match_level: MatchLevel::None,
            }),
        }
    }

    fn snippet_text(&self, text: &str, query_words: &[String], word_count: usize) -> SnippetResult {
        // First, get the full highlight result to find match positions
        let highlight = self.highlight_text(text, query_words);

        // If no match or text is short enough, return as-is (with highlight tags)
        let words: Vec<&str> = text.split_whitespace().collect();
        if words.len() <= word_count {
            return SnippetResult {
                value: highlight.value,
                match_level: highlight.match_level,
            };
        }

        if matches!(highlight.match_level, MatchLevel::None) {
            // No match — take first N words and add ellipsis
            let truncated: String = words[..word_count.min(words.len())].join(" ");
            return SnippetResult {
                value: format!("{}\u{2026}", truncated),
                match_level: MatchLevel::None,
            };
        }

        // Find the word index where the first match occurs
        let text_lower = text.to_lowercase();
        let query_words_lower: Vec<String> = query_words.iter().map(|w| w.to_lowercase()).collect();

        let first_match_byte = query_words_lower
            .iter()
            .filter_map(|qw| text_lower.find(qw.as_str()))
            .min()
            .unwrap_or(0);

        // Find which word index corresponds to this byte offset
        let mut match_word_idx = 0;
        let mut byte_pos = 0;
        for (i, word) in words.iter().enumerate() {
            if let Some(pos) = text[byte_pos..].find(word) {
                let word_start = byte_pos + pos;
                if word_start + word.len() > first_match_byte {
                    match_word_idx = i;
                    break;
                }
                byte_pos = word_start + word.len();
            }
        }

        // Center the window around the match
        let half = word_count / 2;
        let start = match_word_idx.saturating_sub(half);
        let end = (start + word_count).min(words.len());
        let start = if end == words.len() && end > word_count {
            end - word_count
        } else {
            start
        };

        // Extract the snippet window and highlight it
        let snippet_words: Vec<&str> = words[start..end].to_vec();
        let snippet_text = snippet_words.join(" ");
        let snippet_highlight = self.highlight_text(&snippet_text, query_words);

        let mut value = String::new();
        if start > 0 {
            value.push('\u{2026}');
        }
        value.push_str(&snippet_highlight.value);
        if end < words.len() {
            value.push('\u{2026}');
        }

        SnippetResult {
            value,
            match_level: snippet_highlight.match_level,
        }
    }

    fn no_match(&self, value: String) -> HighlightResult {
        HighlightResult {
            value,
            match_level: MatchLevel::None,
            matched_words: Vec::new(),
            fully_highlighted: None,
        }
    }

    fn field_value_to_string(&self, value: &FieldValue) -> String {
        match value {
            FieldValue::Text(s) => s.clone(),
            FieldValue::Integer(i) => i.to_string(),
            FieldValue::Float(f) => f.to_string(),
            FieldValue::Date(d) => d.to_string(),
            FieldValue::Facet(s) => s.clone(),
            FieldValue::Array(_) => "[]".to_string(),
            FieldValue::Object(_) => "{}".to_string(),
        }
    }
}

/// A snippet result — same shape as HighlightResult but `value` is truncated text.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnippetResult {
    pub value: String,
    pub match_level: MatchLevel,
}

/// Recursive snippet value (mirrors HighlightValue).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SnippetValue {
    Single(SnippetResult),
    Array(Vec<SnippetResult>),
    Object(HashMap<String, SnippetValue>),
}

/// Parse "attribute:N" snippet spec. Returns (attribute_name, word_count).
pub fn parse_snippet_spec(spec: &str) -> (&str, usize) {
    if let Some(colon) = spec.rfind(':') {
        let attr = &spec[..colon];
        let count = spec[colon + 1..].parse::<usize>().unwrap_or(10);
        (attr, count)
    } else {
        (spec, 10)
    }
}

pub fn extract_query_words(query_text: &str) -> Vec<String> {
    query_text
        .split_whitespace()
        .map(|s| s.to_lowercase())
        .collect()
}
