use super::QueryExecutor;
use crate::error::Result;
use crate::index::rules::RuleEffects;
use crate::types::ScoredDocument;
use tantivy::Searcher;

impl QueryExecutor {
    pub(crate) fn apply_rules_to_results(
        &self,
        searcher: &Searcher,
        mut documents: Vec<ScoredDocument>,
        effects: &RuleEffects,
    ) -> Result<Vec<ScoredDocument>> {
        if !effects.hidden.is_empty() {
            documents.retain(|doc| !effects.hidden.contains(&doc.document.id));
        }

        if effects.pins.is_empty() {
            return Ok(documents);
        }

        let schema = searcher.schema();
        let id_field = schema
            .get_field("_id")
            .map_err(|_| crate::error::FlapjackError::FieldNotFound("_id".to_string()))?;

        let mut pinned_docs = Vec::new();
        let mut seen_ids = std::collections::HashSet::new();

        for (pin_id, target_pos) in &effects.pins {
            if seen_ids.contains(pin_id) {
                continue;
            }
            seen_ids.insert(pin_id.clone());

            if let Some(pos) = documents.iter().position(|d| &d.document.id == pin_id) {
                pinned_docs.push((documents.remove(pos), *target_pos));
            } else {
                let term = tantivy::Term::from_field_text(id_field, pin_id);
                let term_query =
                    tantivy::query::TermQuery::new(term, tantivy::schema::IndexRecordOption::Basic);
                let top_docs =
                    searcher.search(&term_query, &tantivy::collector::TopDocs::with_limit(1))?;

                if !top_docs.is_empty() {
                    let doc_address = top_docs[0].1;
                    let retrieved_doc = searcher.doc(doc_address)?;
                    let document =
                        self.converter
                            .from_tantivy(retrieved_doc, schema, pin_id.clone())?;
                    pinned_docs.push((
                        ScoredDocument {
                            document,
                            score: f32::MAX,
                        },
                        *target_pos,
                    ));
                }
            }
        }

        pinned_docs.sort_by(|a, b| {
            a.1.cmp(&b.1).then_with(|| {
                let a_idx = effects
                    .pins
                    .iter()
                    .position(|(id, _)| id == &a.0.document.id)
                    .unwrap_or(0);
                let b_idx = effects
                    .pins
                    .iter()
                    .position(|(id, _)| id == &b.0.document.id)
                    .unwrap_or(0);
                a_idx.cmp(&b_idx)
            })
        });

        let mut result = Vec::new();
        let mut doc_iter = documents.into_iter();
        let mut next_pin_idx = 0;

        for target_pos in 0..1000 {
            while next_pin_idx < pinned_docs.len() && pinned_docs[next_pin_idx].1 == target_pos {
                result.push(pinned_docs[next_pin_idx].0.clone());
                next_pin_idx += 1;
            }

            while result.len() == target_pos {
                if let Some(doc) = doc_iter.next() {
                    result.push(doc);
                } else {
                    break;
                }
            }

            if result.len() <= target_pos {
                break;
            }
        }

        while next_pin_idx < pinned_docs.len() {
            result.push(pinned_docs[next_pin_idx].0.clone());
            next_pin_idx += 1;
        }

        result.extend(doc_iter);

        Ok(result)
    }
}
