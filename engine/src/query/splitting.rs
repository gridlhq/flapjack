use tantivy::schema::{Field, IndexRecordOption};
use tantivy::Searcher;

/// Check if a token exists as an exact word in any searchable path.
/// Uses `_json_exact` field (not `_json_search` which has edge-ngrams).
fn term_exists(
    searcher: &Searcher,
    json_exact_field: Field,
    searchable_paths: &[String],
    token: &str,
) -> bool {
    for segment_reader in searcher.segment_readers() {
        let inv_index = match segment_reader.inverted_index(json_exact_field) {
            Ok(idx) => idx,
            Err(_) => continue,
        };
        for path in searchable_paths.iter().take(3) {
            let term_text = format!("{}\0s{}", path, token);
            let term = tantivy::Term::from_field_text(json_exact_field, &term_text);
            if let Ok(Some(_)) = inv_index.read_postings(&term, IndexRecordOption::Basic) {
                return true;
            }
        }
    }
    false
}

/// Generate split alternatives for a query.
/// For each token >= 4 chars, tries all 2-part splits where both halves >= 2 chars
/// and both exist in the index. Takes the first valid split per token.
fn generate_split_alternatives(
    tokens: &[&str],
    searcher: &Searcher,
    json_exact_field: Field,
    searchable_paths: &[String],
) -> Vec<String> {
    let mut alternatives = Vec::new();

    for (i, token) in tokens.iter().enumerate() {
        let lower = token.to_lowercase();
        let chars: Vec<char> = lower.chars().collect();
        let char_count = chars.len();
        if char_count < 4 {
            continue;
        }

        let max_first = 12.min(char_count.saturating_sub(2));
        for split_pos in 2..=max_first {
            let first: String = chars[..split_pos].iter().collect();
            let second: String = chars[split_pos..].iter().collect();
            if second.chars().count() < 2 {
                continue;
            }

            if term_exists(searcher, json_exact_field, searchable_paths, &first)
                && term_exists(searcher, json_exact_field, searchable_paths, &second)
            {
                let mut alt_tokens: Vec<String> = Vec::with_capacity(tokens.len() + 1);
                for (j, t) in tokens.iter().enumerate() {
                    if j == i {
                        alt_tokens.push(first.clone());
                        alt_tokens.push(second.clone());
                    } else {
                        alt_tokens.push(t.to_lowercase());
                    }
                }
                alternatives.push(alt_tokens.join(" "));
                break;
            }
        }
    }

    alternatives
}

/// Generate concatenation alternatives for a query.
/// Bi-gram: concatenate adjacent pairs (first 5 tokens only).
/// All-word: concatenate all tokens if >= 3.
fn generate_concat_alternatives(tokens: &[&str]) -> Vec<String> {
    let mut alternatives = Vec::new();
    if tokens.len() < 2 {
        return alternatives;
    }

    // Bi-gram concatenation for first 5 tokens
    let bigram_limit = tokens.len().min(5);
    for i in 0..bigram_limit.saturating_sub(1) {
        let concat = format!(
            "{}{}",
            tokens[i].to_lowercase(),
            tokens[i + 1].to_lowercase()
        );
        let mut alt_tokens: Vec<String> = Vec::with_capacity(tokens.len());
        for (j, t) in tokens.iter().enumerate() {
            if j == i {
                alt_tokens.push(concat.clone());
            } else if j == i + 1 {
                continue;
            } else {
                alt_tokens.push(t.to_lowercase());
            }
        }
        alternatives.push(alt_tokens.join(" "));
    }

    // All-word concatenation if >= 3 tokens
    if tokens.len() >= 3 {
        let all_concat: String = tokens.iter().map(|t| t.to_lowercase()).collect();
        alternatives.push(all_concat);
    }

    alternatives
}

/// Generate all split and concat alternatives for a query string.
/// Returns alternative query strings (not including the original).
pub fn generate_alternatives(
    query_text: &str,
    searcher: &Searcher,
    json_exact_field: Field,
    searchable_paths: &[String],
) -> Vec<String> {
    let tokens: Vec<&str> = query_text.split_whitespace().collect();
    if tokens.is_empty() {
        return Vec::new();
    }

    let mut alternatives = Vec::new();
    alternatives.extend(generate_split_alternatives(
        &tokens,
        searcher,
        json_exact_field,
        searchable_paths,
    ));
    alternatives.extend(generate_concat_alternatives(&tokens));
    alternatives
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn concat_two_words() {
        let alts = generate_concat_alternatives(&["blue", "tooth"]);
        assert_eq!(alts, vec!["bluetooth"]);
    }

    #[test]
    fn concat_three_words() {
        let alts = generate_concat_alternatives(&["ice", "cream", "cone"]);
        assert!(alts.contains(&"icecream cone".to_string()));
        assert!(alts.contains(&"ice creamcone".to_string()));
        assert!(alts.contains(&"icecreamcone".to_string()));
    }

    #[test]
    fn concat_respects_5_token_limit() {
        let alts = generate_concat_alternatives(&["a", "b", "c", "d", "e", "f", "g"]);
        // 4 bi-grams (pairs within first 5 tokens) + 1 all-word
        assert_eq!(alts.len(), 5);
    }

    #[test]
    fn concat_single_token() {
        assert!(generate_concat_alternatives(&["hello"]).is_empty());
    }

    #[test]
    fn concat_empty() {
        assert!(generate_concat_alternatives(&[]).is_empty());
    }
}
