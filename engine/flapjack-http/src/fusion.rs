use std::collections::HashMap;

use flapjack::vector::VectorSearchResult;

/// A single document in the fused result set.
#[derive(Debug, Clone)]
pub struct FusedResult {
    pub doc_id: String,
    pub fused_score: f64,
    /// Raw cosine similarity (1.0 - distance) from vector search.
    /// None if document only appeared in BM25 results.
    /// Internal — NOT exposed in HTTP response in stage 6.
    pub semantic_score: Option<f32>,
}

/// Reciprocal Rank Fusion: merge BM25 and vector results with weighting.
///
/// Formula: score(d) = (1-ratio) * 1/(k + bm25_rank) + ratio * 1/(k + vec_rank)
/// k=60 is the standard RRF constant (empirically validated).
///
/// - `bm25_doc_ids`: ranked by BM25 score, best first
/// - `vector_results`: ranked by similarity, closest first
/// - `semantic_ratio`: 0.0 = pure BM25, 1.0 = pure vector
/// - `k`: RRF constant, use 60
pub fn rrf_fuse(
    bm25_doc_ids: &[String],
    vector_results: &[VectorSearchResult],
    semantic_ratio: f64,
    k: u32,
) -> Vec<FusedResult> {
    let bm25_weight = 1.0 - semantic_ratio;
    let vec_weight = semantic_ratio;
    let k_f64 = k as f64;

    // Accumulate scores per document
    let mut scores: HashMap<String, (f64, Option<f32>)> = HashMap::new();

    // BM25 contributions
    for (rank, doc_id) in bm25_doc_ids.iter().enumerate() {
        let rrf_score = bm25_weight / (k_f64 + rank as f64 + 1.0);
        let entry = scores.entry(doc_id.clone()).or_insert((0.0, None));
        entry.0 += rrf_score;
    }

    // Vector contributions
    for (rank, vsr) in vector_results.iter().enumerate() {
        let rrf_score = vec_weight / (k_f64 + rank as f64 + 1.0);
        let similarity = 1.0 - vsr.distance;
        let entry = scores.entry(vsr.doc_id.clone()).or_insert((0.0, None));
        entry.0 += rrf_score;
        entry.1 = Some(similarity);
    }

    let mut results: Vec<FusedResult> = scores
        .into_iter()
        .map(|(doc_id, (fused_score, semantic_score))| FusedResult {
            doc_id,
            fused_score,
            semantic_score,
        })
        .collect();

    // Sort by fused_score descending, stable
    results.sort_by(|a, b| {
        b.fused_score
            .partial_cmp(&a.fused_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    results
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vsr(doc_id: &str, distance: f32) -> VectorSearchResult {
        VectorSearchResult {
            doc_id: doc_id.to_string(),
            distance,
        }
    }

    #[test]
    fn test_rrf_pure_bm25() {
        let bm25 = vec!["A".to_string(), "B".to_string(), "C".to_string()];
        let vector: Vec<VectorSearchResult> = vec![];
        let results = rrf_fuse(&bm25, &vector, 0.0, 60);

        // With semantic_ratio=0.0, output order should match BM25 exactly
        assert_eq!(results[0].doc_id, "A");
        assert_eq!(results[1].doc_id, "B");
        assert_eq!(results[2].doc_id, "C");
    }

    #[test]
    fn test_rrf_pure_vector() {
        let bm25: Vec<String> = vec![];
        let vector = vec![vsr("X", 0.1), vsr("Y", 0.2), vsr("Z", 0.3)];
        let results = rrf_fuse(&bm25, &vector, 1.0, 60);

        // With semantic_ratio=1.0, output order should match vector order
        assert_eq!(results[0].doc_id, "X");
        assert_eq!(results[1].doc_id, "Y");
        assert_eq!(results[2].doc_id, "Z");
    }

    #[test]
    fn test_rrf_equal_blend() {
        // BM25: [A, B, C], Vector: [C, A, D]
        let bm25 = vec!["A".to_string(), "B".to_string(), "C".to_string()];
        let vector = vec![vsr("C", 0.1), vsr("A", 0.2), vsr("D", 0.3)];
        let results = rrf_fuse(&bm25, &vector, 0.5, 60);

        assert_eq!(results.len(), 4); // A, B, C, D

        // A and C appear in both lists, so they MUST both rank above B and D
        let top_two: Vec<&str> = results.iter().take(2).map(|r| r.doc_id.as_str()).collect();
        assert!(
            top_two.contains(&"A") && top_two.contains(&"C"),
            "Both A and C should be in top 2 since they appear in both lists, got {:?}",
            top_two
        );

        // A should beat C: A is BM25 rank 1 + vector rank 2, C is BM25 rank 3 + vector rank 1
        // A: 0.5/61 + 0.5/62 ≈ 0.01626, C: 0.5/63 + 0.5/61 ≈ 0.01613
        assert_eq!(results[0].doc_id, "A");
        assert_eq!(results[1].doc_id, "C");
    }

    #[test]
    fn test_rrf_document_in_one_source_only() {
        let bm25 = vec!["A".to_string(), "B".to_string()];
        let vector = vec![vsr("C", 0.1)];
        let results = rrf_fuse(&bm25, &vector, 0.5, 60);

        // All 3 docs should appear
        assert_eq!(results.len(), 3);
        let ids: Vec<&str> = results.iter().map(|r| r.doc_id.as_str()).collect();
        assert!(ids.contains(&"A"));
        assert!(ids.contains(&"B"));
        assert!(ids.contains(&"C"));
    }

    #[test]
    fn test_rrf_empty_inputs() {
        // Both empty
        let results = rrf_fuse(&[], &[], 0.5, 60);
        assert!(results.is_empty());

        // Only BM25
        let bm25 = vec!["A".to_string()];
        let results = rrf_fuse(&bm25, &[], 0.5, 60);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].doc_id, "A");

        // Only vector
        let vector = vec![vsr("X", 0.1)];
        let results = rrf_fuse(&[], &vector, 0.5, 60);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].doc_id, "X");
    }

    #[test]
    fn test_rrf_scores_monotonically_decrease() {
        let bm25: Vec<String> = (0..20).map(|i| format!("doc{}", i)).collect();
        let vector: Vec<VectorSearchResult> = (0..20)
            .map(|i| vsr(&format!("doc{}", 19 - i), i as f32 * 0.05))
            .collect();
        let results = rrf_fuse(&bm25, &vector, 0.5, 60);

        for window in results.windows(2) {
            assert!(
                window[0].fused_score >= window[1].fused_score,
                "Scores should be monotonically decreasing: {} >= {}",
                window[0].fused_score,
                window[1].fused_score
            );
        }
    }

    #[test]
    fn test_rrf_fused_result_includes_vector_similarity() {
        let bm25 = vec!["A".to_string(), "B".to_string()];
        let vector = vec![vsr("A", 0.2), vsr("C", 0.3)];
        let results = rrf_fuse(&bm25, &vector, 0.5, 60);

        // A is in both — should have semantic_score
        let a = results.iter().find(|r| r.doc_id == "A").unwrap();
        assert!(a.semantic_score.is_some());
        assert!((a.semantic_score.unwrap() - 0.8).abs() < 0.001); // 1.0 - 0.2 distance

        // B is BM25-only — no semantic_score
        let b = results.iter().find(|r| r.doc_id == "B").unwrap();
        assert!(b.semantic_score.is_none());

        // C is vector-only — has semantic_score
        let c = results.iter().find(|r| r.doc_id == "C").unwrap();
        assert!(c.semantic_score.is_some());
        assert!((c.semantic_score.unwrap() - 0.7).abs() < 0.001); // 1.0 - 0.3 distance
    }
}
