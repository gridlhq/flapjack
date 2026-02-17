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
