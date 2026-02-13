use crate::error::Result;
use crate::index::settings::IndexSettings;
use crate::types::Filter;
use std::collections::HashSet;
use tantivy::query::{AllQuery, BooleanQuery, Occur, Query};
use tantivy::schema::Schema;

pub struct FilterCompiler {
    #[allow(dead_code)]
    schema: Schema,
    query_parser: tantivy::query::QueryParser,
}

impl FilterCompiler {
    pub fn new(schema: Schema) -> Self {
        let json_filter_field = schema
            .get_field("_json_filter")
            .expect("_json_filter field must exist in schema");

        let query_parser = tantivy::query::QueryParser::new(
            schema.clone(),
            vec![json_filter_field],
            tantivy::tokenizer::TokenizerManager::default(),
        );

        FilterCompiler {
            schema,
            query_parser,
        }
    }

    const MAX_FILTER_DEPTH: usize = 10;
    const MAX_BOOLEAN_CLAUSES: usize = 1000;

    pub fn compile(
        &self,
        filter: &Filter,
        settings: Option<&IndexSettings>,
    ) -> Result<Box<dyn Query>> {
        let clause_count = self.count_clauses(filter);
        if clause_count > Self::MAX_BOOLEAN_CLAUSES {
            return Err(crate::error::FlapjackError::InvalidQuery(format!(
                "Filter has {} clauses, exceeds maximum {}",
                clause_count,
                Self::MAX_BOOLEAN_CLAUSES
            )));
        }

        let facet_set = settings.map(|s| s.facet_set()).unwrap_or_default();

        if !self.is_valid_for_facet_set(filter, &facet_set) {
            return Ok(Box::new(tantivy::query::EmptyQuery));
        }

        if self.has_not(filter) {
            self.compile_with_hybrid(filter, 0)
        } else {
            let query_string = self.to_query_string(filter)?;
            self.query_parser
                .parse_query(&query_string)
                .map_err(|e| crate::error::FlapjackError::InvalidQuery(e.to_string()))
        }
    }

    fn is_valid_for_facet_set(&self, filter: &Filter, facet_set: &HashSet<String>) -> bool {
        match filter {
            Filter::Equals { field, value } => {
                if matches!(value, crate::types::FieldValue::Text(_)) {
                    facet_set.contains(field)
                } else {
                    true
                }
            }
            Filter::And(filters) | Filter::Or(filters) => filters
                .iter()
                .all(|f| self.is_valid_for_facet_set(f, facet_set)),
            Filter::Not(inner) => self.is_valid_for_facet_set(inner, facet_set),
            _ => true,
        }
    }

    fn has_not(&self, filter: &Filter) -> bool {
        match filter {
            Filter::Not(_) | Filter::NotEquals { .. } => true,
            Filter::And(filters) | Filter::Or(filters) => filters.iter().any(|f| self.has_not(f)),
            _ => false,
        }
    }

    fn to_query_string(&self, filter: &Filter) -> Result<String> {
        match filter {
            Filter::Equals { field, value } => match value {
                crate::types::FieldValue::Text(_s) => Ok(format!(
                    "_json_filter.{}:{}",
                    field,
                    self.format_value(value)
                )),
                crate::types::FieldValue::Integer(i) => {
                    Ok(format!("_json_filter.{}:[{} TO {}]", field, i, i))
                }
                crate::types::FieldValue::Float(f) => {
                    Ok(format!("_json_filter.{}:[{} TO {}]", field, f, f))
                }
                crate::types::FieldValue::Date(d) => {
                    Ok(format!("_json_filter.{}:[{} TO {}]", field, d, d))
                }
                _ => Err(crate::error::FlapjackError::InvalidQuery(
                    "Equals only supports text, integer, float, or date values".to_string(),
                )),
            },
            Filter::Range { field, min, max } => {
                Ok(format!("_json_filter.{}:[{} TO {}]", field, min, max))
            }
            Filter::GreaterThan { field, value } => match value {
                crate::types::FieldValue::Integer(i) => {
                    Ok(format!("_json_filter.{}:[{} TO *]", field, i + 1))
                }
                crate::types::FieldValue::Float(_) => {
                    Err(crate::error::FlapjackError::InvalidQuery(format!(
                        "Exclusive '>' not supported on floats for field '{}'. Use '>='",
                        field
                    )))
                }
                crate::types::FieldValue::Date(d) => {
                    Ok(format!("_json_filter.{}:[{} TO *]", field, d + 1))
                }
                _ => Err(crate::error::FlapjackError::InvalidQuery(
                    "GreaterThan only supports integer/date values".to_string(),
                )),
            },
            Filter::GreaterThanOrEqual { field, value } => Ok(format!(
                "_json_filter.{}:[{} TO *]",
                field,
                self.format_range_value(value)
            )),
            Filter::LessThan { field, value } => match value {
                crate::types::FieldValue::Integer(i) => {
                    Ok(format!("_json_filter.{}:[* TO {}]", field, i - 1))
                }
                crate::types::FieldValue::Float(_) => {
                    Err(crate::error::FlapjackError::InvalidQuery(format!(
                        "Exclusive '<' not supported on floats for field '{}'. Use '<='",
                        field
                    )))
                }
                crate::types::FieldValue::Date(d) => {
                    Ok(format!("_json_filter.{}:[* TO {}]", field, d - 1))
                }
                _ => Err(crate::error::FlapjackError::InvalidQuery(
                    "LessThan only supports integer/date values".to_string(),
                )),
            },
            Filter::LessThanOrEqual { field, value } => Ok(format!(
                "_json_filter.{}:[* TO {}]",
                field,
                self.format_range_value(value)
            )),
            Filter::And(filters) => {
                let parts: Result<Vec<_>> =
                    filters.iter().map(|f| self.to_query_string(f)).collect();
                Ok(format!("({})", parts?.join(" AND ")))
            }
            Filter::Or(filters) => {
                let parts: Result<Vec<_>> =
                    filters.iter().map(|f| self.to_query_string(f)).collect();
                Ok(format!("({})", parts?.join(" OR ")))
            }
            Filter::Not(_) | Filter::NotEquals { .. } => {
                Err(crate::error::FlapjackError::InvalidQuery(
                    "NOT filters must use hybrid compilation".to_string(),
                ))
            }
        }
    }

    fn format_value(&self, value: &crate::types::FieldValue) -> String {
        match value {
            crate::types::FieldValue::Object(_) => {
                panic!("Object values cannot be used in filters directly")
            }
            crate::types::FieldValue::Array(_) => {
                panic!("Array values cannot be used in filters directly")
            }
            crate::types::FieldValue::Text(s) => {
                if s.contains(' ') || s.contains(':') || s.contains('%') {
                    format!("\"{}\"", s.replace('"', "\\\""))
                } else {
                    s.clone()
                }
            }
            crate::types::FieldValue::Integer(i) => i.to_string(),
            crate::types::FieldValue::Float(f) => f.to_string(),
            crate::types::FieldValue::Date(d) => d.to_string(),
            crate::types::FieldValue::Facet(s) => format!("\"{}\"", s),
        }
    }

    fn format_range_value(&self, value: &crate::types::FieldValue) -> String {
        match value {
            crate::types::FieldValue::Integer(i) => i.to_string(),
            crate::types::FieldValue::Float(f) => f.to_string(),
            crate::types::FieldValue::Date(d) => d.to_string(),
            _ => panic!("Range queries only support numeric/date values"),
        }
    }

    fn count_clauses(&self, filter: &Filter) -> usize {
        fn count_recursive(filter: &Filter) -> usize {
            match filter {
                Filter::Equals { .. }
                | Filter::NotEquals { .. }
                | Filter::GreaterThan { .. }
                | Filter::GreaterThanOrEqual { .. }
                | Filter::LessThan { .. }
                | Filter::LessThanOrEqual { .. }
                | Filter::Range { .. } => 1,
                Filter::Not(inner) => count_recursive(inner),
                Filter::And(filters) | Filter::Or(filters) => {
                    filters.iter().map(count_recursive).sum()
                }
            }
        }
        count_recursive(filter)
    }

    fn compile_with_hybrid(&self, filter: &Filter, depth: usize) -> Result<Box<dyn Query>> {
        if depth > Self::MAX_FILTER_DEPTH {
            return Err(crate::error::FlapjackError::InvalidQuery(format!(
                "Filter nesting exceeds {} levels",
                Self::MAX_FILTER_DEPTH
            )));
        }

        match filter {
            Filter::Not(inner) => {
                let inner_query = self.compile_with_hybrid(inner, depth + 1)?;
                Ok(Box::new(BooleanQuery::new(vec![
                    (Occur::Must, Box::new(AllQuery) as Box<dyn Query>),
                    (Occur::MustNot, inner_query),
                ])))
            }
            Filter::NotEquals { field, value } => {
                let equals_str = format!("_json_filter.{}:{}", field, self.format_value(value));
                let equals_query = self
                    .query_parser
                    .parse_query(&equals_str)
                    .map_err(|e| crate::error::FlapjackError::InvalidQuery(e.to_string()))?;
                Ok(Box::new(BooleanQuery::new(vec![
                    (Occur::Must, Box::new(AllQuery) as Box<dyn Query>),
                    (Occur::MustNot, equals_query),
                ])))
            }
            Filter::And(filters) => {
                let mut subqueries = Vec::new();
                for f in filters {
                    subqueries.push((Occur::Must, self.compile_with_hybrid(f, depth + 1)?));
                }
                Ok(Box::new(BooleanQuery::new(subqueries)))
            }
            Filter::Or(filters) => {
                let mut subqueries = Vec::new();
                for f in filters {
                    subqueries.push((Occur::Should, self.compile_with_hybrid(f, depth + 1)?));
                }
                Ok(Box::new(BooleanQuery::new(subqueries)))
            }
            _ => {
                let query_str = self.to_query_string(filter)?;
                self.query_parser
                    .parse_query(&query_str)
                    .map_err(|e| crate::error::FlapjackError::InvalidQuery(e.to_string()))
            }
        }
    }
}
