use crate::error::Result;
use tantivy::query::{BooleanQuery, FuzzyTermQuery, Occur, Query};
use tantivy::schema::Field;
use tantivy::Term;

pub struct FuzzyQueryBuilder {
    field: Field,
    term: String,
    distance: u8,
}

impl FuzzyQueryBuilder {
    pub fn new(field: Field, term: String) -> Self {
        let distance = Self::calculate_distance(&term);
        FuzzyQueryBuilder {
            field,
            term,
            distance,
        }
    }

    fn calculate_distance(term: &str) -> u8 {
        let len = term.chars().count();
        if len < 5 {
            0
        } else if len < 9 {
            1
        } else {
            2
        }
    }

    pub fn build(self) -> Box<dyn Query> {
        if self.distance == 0 {
            return Box::new(tantivy::query::TermQuery::new(
                Term::from_field_text(self.field, &self.term),
                tantivy::schema::IndexRecordOption::WithFreqsAndPositions,
            ));
        }

        Box::new(FuzzyTermQuery::new(
            Term::from_field_text(self.field, &self.term),
            self.distance,
            true,
        ))
    }
}

pub fn apply_fuzzy_to_terms(
    base_query: Box<dyn Query>,
    fields: &[Field],
    query_terms: &[String],
) -> Result<Box<dyn Query>> {
    let mut fuzzy_queries: Vec<(Occur, Box<dyn Query>)> = Vec::new();

    fuzzy_queries.push((Occur::Should, base_query));

    for term in query_terms {
        for &field in fields {
            let fuzzy = FuzzyQueryBuilder::new(field, term.clone()).build();
            fuzzy_queries.push((Occur::Should, fuzzy));
        }
    }

    Ok(Box::new(BooleanQuery::new(fuzzy_queries)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distance_short_words_zero() {
        // < 5 chars → distance 0 (exact match only)
        assert_eq!(FuzzyQueryBuilder::calculate_distance(""), 0);
        assert_eq!(FuzzyQueryBuilder::calculate_distance("a"), 0);
        assert_eq!(FuzzyQueryBuilder::calculate_distance("abcd"), 0);
    }

    #[test]
    fn distance_medium_words_one() {
        // 5-8 chars → distance 1
        assert_eq!(FuzzyQueryBuilder::calculate_distance("abcde"), 1);
        assert_eq!(FuzzyQueryBuilder::calculate_distance("abcdefgh"), 1);
    }

    #[test]
    fn distance_long_words_two() {
        // >= 9 chars → distance 2
        assert_eq!(FuzzyQueryBuilder::calculate_distance("abcdefghi"), 2);
        assert_eq!(FuzzyQueryBuilder::calculate_distance("international"), 2);
    }

    #[test]
    fn distance_unicode_counts_chars_not_bytes() {
        // "café" = 4 chars → distance 0
        assert_eq!(FuzzyQueryBuilder::calculate_distance("café"), 0);
        // "naïveté" = 7 chars → distance 1
        assert_eq!(FuzzyQueryBuilder::calculate_distance("naïveté"), 1);
    }

    #[test]
    fn distance_boundary_4_to_5() {
        assert_eq!(FuzzyQueryBuilder::calculate_distance("xxxx"), 0); // 4
        assert_eq!(FuzzyQueryBuilder::calculate_distance("xxxxx"), 1); // 5
    }

    #[test]
    fn distance_boundary_8_to_9() {
        assert_eq!(FuzzyQueryBuilder::calculate_distance("xxxxxxxx"), 1); // 8
        assert_eq!(FuzzyQueryBuilder::calculate_distance("xxxxxxxxx"), 2); // 9
    }

    #[test]
    fn builder_short_term_exact_match() {
        // distance=0 → should produce a TermQuery, not FuzzyTermQuery
        let schema_builder = tantivy::schema::SchemaBuilder::new();
        let schema = schema_builder.build();
        let field = schema.get_field("_json_search").unwrap_or_else(|_| {
            // Create a minimal schema for test
            let mut sb = tantivy::schema::Schema::builder();
            sb.add_text_field("test", tantivy::schema::TEXT)
        });
        let builder = FuzzyQueryBuilder::new(field, "cat".to_string());
        assert_eq!(builder.distance, 0);
        // build() should not panic
        let _query = builder.build();
    }

    #[test]
    fn builder_long_term_fuzzy() {
        let mut sb = tantivy::schema::Schema::builder();
        let field = sb.add_text_field("test", tantivy::schema::TEXT);
        let builder = FuzzyQueryBuilder::new(field, "international".to_string());
        assert_eq!(builder.distance, 2);
        let _query = builder.build();
    }
}
