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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::schema::SchemaBuilder;
    use crate::types::FieldValue;

    fn make_compiler() -> FilterCompiler {
        let schema = SchemaBuilder::new().build();
        let tantivy = schema.to_tantivy();
        FilterCompiler::new(tantivy)
    }

    // ── count_clauses ───────────────────────────────────────────────────

    #[test]
    fn count_clauses_single_equals() {
        let c = make_compiler();
        let f = Filter::Equals {
            field: "x".into(),
            value: FieldValue::Integer(1),
        };
        assert_eq!(c.count_clauses(&f), 1);
    }

    #[test]
    fn count_clauses_and_of_three() {
        let c = make_compiler();
        let f = Filter::And(vec![
            Filter::Equals {
                field: "a".into(),
                value: FieldValue::Integer(1),
            },
            Filter::Equals {
                field: "b".into(),
                value: FieldValue::Integer(2),
            },
            Filter::Equals {
                field: "c".into(),
                value: FieldValue::Integer(3),
            },
        ]);
        assert_eq!(c.count_clauses(&f), 3);
    }

    #[test]
    fn count_clauses_nested() {
        let c = make_compiler();
        let f = Filter::And(vec![
            Filter::Or(vec![
                Filter::Equals {
                    field: "a".into(),
                    value: FieldValue::Integer(1),
                },
                Filter::Equals {
                    field: "b".into(),
                    value: FieldValue::Integer(2),
                },
            ]),
            Filter::Not(Box::new(Filter::Equals {
                field: "c".into(),
                value: FieldValue::Integer(3),
            })),
        ]);
        assert_eq!(c.count_clauses(&f), 3);
    }

    // ── has_not ─────────────────────────────────────────────────────────

    #[test]
    fn has_not_simple_equals_false() {
        let c = make_compiler();
        let f = Filter::Equals {
            field: "x".into(),
            value: FieldValue::Integer(1),
        };
        assert!(!c.has_not(&f));
    }

    #[test]
    fn has_not_with_not_true() {
        let c = make_compiler();
        let f = Filter::Not(Box::new(Filter::Equals {
            field: "x".into(),
            value: FieldValue::Integer(1),
        }));
        assert!(c.has_not(&f));
    }

    #[test]
    fn has_not_with_not_equals_true() {
        let c = make_compiler();
        let f = Filter::NotEquals {
            field: "x".into(),
            value: FieldValue::Integer(1),
        };
        assert!(c.has_not(&f));
    }

    #[test]
    fn has_not_nested_in_and() {
        let c = make_compiler();
        let f = Filter::And(vec![
            Filter::Equals {
                field: "a".into(),
                value: FieldValue::Integer(1),
            },
            Filter::NotEquals {
                field: "b".into(),
                value: FieldValue::Integer(2),
            },
        ]);
        assert!(c.has_not(&f));
    }

    // ── to_query_string ─────────────────────────────────────────────────

    #[test]
    fn query_string_integer_equals() {
        let c = make_compiler();
        let f = Filter::Equals {
            field: "price".into(),
            value: FieldValue::Integer(42),
        };
        let qs = c.to_query_string(&f).unwrap();
        assert_eq!(qs, "_json_filter.price:[42 TO 42]");
    }

    #[test]
    fn query_string_text_equals() {
        let c = make_compiler();
        let f = Filter::Equals {
            field: "color".into(),
            value: FieldValue::Text("red".into()),
        };
        let qs = c.to_query_string(&f).unwrap();
        assert_eq!(qs, "_json_filter.color:red");
    }

    #[test]
    fn query_string_text_with_space_quoted() {
        let c = make_compiler();
        let f = Filter::Equals {
            field: "color".into(),
            value: FieldValue::Text("dark red".into()),
        };
        let qs = c.to_query_string(&f).unwrap();
        assert_eq!(qs, "_json_filter.color:\"dark red\"");
    }

    #[test]
    fn query_string_range() {
        let c = make_compiler();
        let f = Filter::Range {
            field: "price".into(),
            min: 10.0,
            max: 100.0,
        };
        let qs = c.to_query_string(&f).unwrap();
        assert_eq!(qs, "_json_filter.price:[10 TO 100]");
    }

    #[test]
    fn query_string_gte() {
        let c = make_compiler();
        let f = Filter::GreaterThanOrEqual {
            field: "age".into(),
            value: FieldValue::Integer(18),
        };
        let qs = c.to_query_string(&f).unwrap();
        assert_eq!(qs, "_json_filter.age:[18 TO *]");
    }

    #[test]
    fn query_string_lte() {
        let c = make_compiler();
        let f = Filter::LessThanOrEqual {
            field: "age".into(),
            value: FieldValue::Integer(65),
        };
        let qs = c.to_query_string(&f).unwrap();
        assert_eq!(qs, "_json_filter.age:[* TO 65]");
    }

    #[test]
    fn query_string_and() {
        let c = make_compiler();
        let f = Filter::And(vec![
            Filter::Equals {
                field: "a".into(),
                value: FieldValue::Integer(1),
            },
            Filter::Equals {
                field: "b".into(),
                value: FieldValue::Integer(2),
            },
        ]);
        let qs = c.to_query_string(&f).unwrap();
        assert_eq!(qs, "(_json_filter.a:[1 TO 1] AND _json_filter.b:[2 TO 2])");
    }

    #[test]
    fn query_string_or() {
        let c = make_compiler();
        let f = Filter::Or(vec![
            Filter::Equals {
                field: "a".into(),
                value: FieldValue::Integer(1),
            },
            Filter::Equals {
                field: "a".into(),
                value: FieldValue::Integer(2),
            },
        ]);
        let qs = c.to_query_string(&f).unwrap();
        assert_eq!(qs, "(_json_filter.a:[1 TO 1] OR _json_filter.a:[2 TO 2])");
    }

    #[test]
    fn query_string_not_errors() {
        let c = make_compiler();
        let f = Filter::Not(Box::new(Filter::Equals {
            field: "x".into(),
            value: FieldValue::Integer(1),
        }));
        assert!(c.to_query_string(&f).is_err());
    }

    // ── compile ─────────────────────────────────────────────────────────

    #[test]
    fn compile_simple_succeeds() {
        let c = make_compiler();
        let f = Filter::Equals {
            field: "price".into(),
            value: FieldValue::Integer(10),
        };
        assert!(c.compile(&f, None).is_ok());
    }

    #[test]
    fn compile_with_not_succeeds() {
        let c = make_compiler();
        let f = Filter::Not(Box::new(Filter::Equals {
            field: "price".into(),
            value: FieldValue::Integer(10),
        }));
        assert!(c.compile(&f, None).is_ok());
    }

    #[test]
    fn compile_too_many_clauses_errors() {
        let c = make_compiler();
        let clauses: Vec<Filter> = (0..1001)
            .map(|i| Filter::Equals {
                field: "x".into(),
                value: FieldValue::Integer(i),
            })
            .collect();
        let f = Filter::And(clauses);
        assert!(c.compile(&f, None).is_err());
    }

    // ── format_value ────────────────────────────────────────────────────

    #[test]
    fn format_value_text_simple() {
        let c = make_compiler();
        assert_eq!(c.format_value(&FieldValue::Text("hello".into())), "hello");
    }

    #[test]
    fn format_value_text_with_spaces() {
        let c = make_compiler();
        assert_eq!(
            c.format_value(&FieldValue::Text("hello world".into())),
            "\"hello world\""
        );
    }

    #[test]
    fn format_value_integer() {
        let c = make_compiler();
        assert_eq!(c.format_value(&FieldValue::Integer(42)), "42");
    }

    #[test]
    fn format_value_float() {
        let c = make_compiler();
        assert_eq!(c.format_value(&FieldValue::Float(3.14)), "3.14");
    }

    #[test]
    fn format_value_facet() {
        let c = make_compiler();
        assert_eq!(c.format_value(&FieldValue::Facet("cat".into())), "\"cat\"");
    }

    // ── gt/lt edge cases ────────────────────────────────────────────────

    #[test]
    fn gt_float_unsupported() {
        let c = make_compiler();
        let f = Filter::GreaterThan {
            field: "price".into(),
            value: FieldValue::Float(10.5),
        };
        assert!(c.to_query_string(&f).is_err());
    }

    #[test]
    fn lt_float_unsupported() {
        let c = make_compiler();
        let f = Filter::LessThan {
            field: "price".into(),
            value: FieldValue::Float(10.5),
        };
        assert!(c.to_query_string(&f).is_err());
    }

    #[test]
    fn gt_integer_adds_one() {
        let c = make_compiler();
        let f = Filter::GreaterThan {
            field: "age".into(),
            value: FieldValue::Integer(18),
        };
        let qs = c.to_query_string(&f).unwrap();
        assert_eq!(qs, "_json_filter.age:[19 TO *]");
    }

    #[test]
    fn lt_integer_subtracts_one() {
        let c = make_compiler();
        let f = Filter::LessThan {
            field: "age".into(),
            value: FieldValue::Integer(65),
        };
        let qs = c.to_query_string(&f).unwrap();
        assert_eq!(qs, "_json_filter.age:[* TO 64]");
    }
}
