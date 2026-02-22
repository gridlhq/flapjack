use super::assignment::murmurhash3_128;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Team {
    A,
    B,
}

#[derive(Debug, Clone)]
pub struct InterleavedItem {
    pub doc_id: String,
    pub team: Team,
    pub position: usize,
}

/// Interleave two ranked result lists using the team draft algorithm.
///
/// The first-team coin flip is deterministic via MurmurHash3 on
/// `"{experiment_id}:{query_id}"`. Each team takes turns picking their
/// next-best unseen document. Returns exactly `k` items (or fewer if
/// both lists are exhausted).
pub fn team_draft_interleave(
    list_a: &[&str],
    list_b: &[&str],
    k: usize,
    experiment_id: &str,
    query_id: &str,
) -> Vec<InterleavedItem> {
    use std::collections::HashSet;

    // Deterministic coin flip for first team
    let key = format!("{}:{}", experiment_id, query_id);
    let (h1, _) = murmurhash3_128(key.as_bytes(), 0);
    let first_team = if h1 & 1 == 0 { Team::A } else { Team::B };

    let mut result = Vec::with_capacity(k);
    let mut seen = HashSet::with_capacity(k);
    let mut ptr_a = 0usize;
    let mut ptr_b = 0usize;

    // Alternate turns starting with first_team
    let mut current_team = first_team;

    while result.len() < k {
        let picked = match current_team {
            Team::A => pick_next(list_a, &mut ptr_a, &seen),
            Team::B => pick_next(list_b, &mut ptr_b, &seen),
        };

        if let Some(doc_id) = picked {
            seen.insert(doc_id.to_string());
            result.push(InterleavedItem {
                doc_id: doc_id.to_string(),
                team: current_team,
                position: result.len(),
            });
        } else {
            // Current team exhausted — try the other team
            let other = match current_team {
                Team::A => Team::B,
                Team::B => Team::A,
            };
            let fallback = match other {
                Team::A => pick_next(list_a, &mut ptr_a, &seen),
                Team::B => pick_next(list_b, &mut ptr_b, &seen),
            };
            if let Some(doc_id) = fallback {
                seen.insert(doc_id.to_string());
                result.push(InterleavedItem {
                    doc_id: doc_id.to_string(),
                    team: other,
                    position: result.len(),
                });
            } else {
                // Both lists exhausted
                break;
            }
        }

        // Alternate teams
        current_team = match current_team {
            Team::A => Team::B,
            Team::B => Team::A,
        };
    }

    result
}

/// Advance the pointer past already-seen docs and return the next unseen doc.
fn pick_next<'a>(
    list: &[&'a str],
    ptr: &mut usize,
    seen: &std::collections::HashSet<String>,
) -> Option<&'a str> {
    while *ptr < list.len() {
        let doc = list[*ptr];
        *ptr += 1;
        if !seen.contains(doc) {
            return Some(doc);
        }
    }
    None
}

/// Attribute a clicked document to the team that drafted it.
/// Returns `None` if the doc_id is not in the interleaved list.
pub fn attribute_click(doc_id: &str, interleaved: &[InterleavedItem]) -> Option<Team> {
    interleaved
        .iter()
        .find(|item| item.doc_id == doc_id)
        .map(|item| item.team)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn team_draft_produces_k_items() {
        let list_a = vec!["a1", "a2", "a3", "a4", "a5"];
        let list_b = vec!["b1", "b2", "b3", "b4", "b5"];
        let result = team_draft_interleave(&list_a, &list_b, 5, "exp-id", "query-id");
        assert_eq!(result.len(), 5);
    }

    #[test]
    fn team_draft_produces_no_duplicates() {
        let list_a = vec!["a1", "a2", "a3", "a4", "a5"];
        let list_b = vec!["b1", "a2", "b3", "a4", "b5"]; // overlapping docs
        let result = team_draft_interleave(&list_a, &list_b, 8, "exp-id", "query-id");
        let ids: Vec<_> = result.iter().map(|r| &r.doc_id).collect();
        let unique: HashSet<_> = ids.iter().collect();
        assert_eq!(ids.len(), unique.len(), "duplicates found: {:?}", ids);
    }

    #[test]
    fn team_draft_first_team_is_deterministic() {
        let list_a = vec!["a1", "a2"];
        let list_b = vec!["b1", "b2"];
        let r1 = team_draft_interleave(&list_a, &list_b, 2, "exp-abc", "query-xyz");
        let r2 = team_draft_interleave(&list_a, &list_b, 2, "exp-abc", "query-xyz");
        assert_eq!(r1[0].team, r2[0].team);
        assert_eq!(r1[0].doc_id, r2[0].doc_id);
    }

    #[test]
    fn team_draft_different_queries_get_different_first_teams_statistically() {
        let list_a = vec!["a1"];
        let list_b = vec!["b1"];
        let team_a_first = (0..1000)
            .filter(|i| {
                let r = team_draft_interleave(&list_a, &list_b, 1, "exp-id", &format!("q{}", i));
                r[0].team == Team::A
            })
            .count();
        let ratio = team_a_first as f64 / 1000.0;
        assert!(
            (ratio - 0.5).abs() < 0.05,
            "expected ~50% Team A first, got {}%",
            ratio * 100.0
        );
    }

    #[test]
    fn click_attribution_identifies_correct_team() {
        let list_a = vec!["a1", "a2", "a3"];
        let list_b = vec!["b1", "b2", "b3"];
        let result = team_draft_interleave(&list_a, &list_b, 4, "exp-id", "q1");
        let first = &result[0];
        let team = attribute_click(&first.doc_id, &result);
        assert_eq!(team, Some(first.team));
    }

    #[test]
    fn click_attribution_returns_none_for_unknown_doc() {
        let list_a = vec!["a1", "a2"];
        let list_b = vec!["b1", "b2"];
        let result = team_draft_interleave(&list_a, &list_b, 2, "exp-id", "q1");
        assert_eq!(attribute_click("unknown-doc", &result), None);
    }

    #[test]
    fn team_draft_respects_k_limit() {
        let list_a = vec!["a1", "a2", "a3", "a4", "a5"];
        let list_b = vec!["b1", "b2", "b3", "b4", "b5"];
        let result = team_draft_interleave(&list_a, &list_b, 3, "exp-id", "q1");
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn team_draft_handles_empty_lists() {
        let empty: Vec<&str> = vec![];
        let result = team_draft_interleave(&empty, &empty, 5, "exp-id", "q1");
        assert!(result.is_empty());
    }

    #[test]
    fn team_draft_handles_one_empty_list() {
        let list_a = vec!["a1", "a2", "a3"];
        let empty: Vec<&str> = vec![];
        let result = team_draft_interleave(&list_a, &empty, 3, "exp-id", "q1");
        assert_eq!(result.len(), 3);
        assert!(result.iter().all(|item| item.team == Team::A));
    }

    #[test]
    fn team_draft_handles_single_item_lists() {
        let list_a = vec!["a1"];
        let list_b = vec!["b1"];
        let result = team_draft_interleave(&list_a, &list_b, 2, "exp-id", "q1");
        assert_eq!(result.len(), 2);
        let teams: HashSet<_> = result.iter().map(|r| r.team).collect();
        assert_eq!(teams.len(), 2, "both teams should be represented");
    }

    #[test]
    fn team_draft_positions_are_sequential() {
        let list_a = vec!["a1", "a2", "a3"];
        let list_b = vec!["b1", "b2", "b3"];
        let result = team_draft_interleave(&list_a, &list_b, 4, "exp-id", "q1");
        for (i, item) in result.iter().enumerate() {
            assert_eq!(item.position, i, "position should be sequential");
        }
    }

    #[test]
    fn team_draft_exhausted_lists_returns_fewer_than_k() {
        let list_a = vec!["a1"];
        let list_b = vec!["b1"];
        let result = team_draft_interleave(&list_a, &list_b, 10, "exp-id", "q1");
        assert_eq!(result.len(), 2, "can't produce more items than unique docs");
    }
}
