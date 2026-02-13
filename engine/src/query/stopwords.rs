use std::collections::HashSet;

pub fn english_stop_words() -> HashSet<&'static str> {
    [
        "a",
        "an",
        "the",
        "and",
        "or",
        "but",
        "nor",
        "not",
        "is",
        "are",
        "was",
        "were",
        "be",
        "been",
        "being",
        "have",
        "has",
        "had",
        "having",
        "do",
        "does",
        "did",
        "doing",
        "will",
        "would",
        "shall",
        "should",
        "may",
        "might",
        "must",
        "can",
        "could",
        "i",
        "me",
        "my",
        "myself",
        "we",
        "our",
        "ours",
        "ourselves",
        "you",
        "your",
        "yours",
        "yourself",
        "yourselves",
        "he",
        "him",
        "his",
        "himself",
        "she",
        "her",
        "hers",
        "herself",
        "it",
        "its",
        "itself",
        "they",
        "them",
        "their",
        "theirs",
        "themselves",
        "what",
        "which",
        "who",
        "whom",
        "this",
        "that",
        "these",
        "those",
        "am",
        "if",
        "then",
        "else",
        "when",
        "where",
        "why",
        "how",
        "all",
        "each",
        "every",
        "both",
        "few",
        "more",
        "most",
        "other",
        "some",
        "such",
        "no",
        "only",
        "own",
        "same",
        "so",
        "than",
        "too",
        "very",
        "of",
        "in",
        "to",
        "for",
        "with",
        "on",
        "at",
        "from",
        "by",
        "about",
        "between",
        "through",
        "during",
        "before",
        "after",
        "above",
        "below",
        "up",
        "down",
        "out",
        "off",
        "over",
        "under",
        "again",
        "further",
        "here",
        "there",
        "once",
        "just",
        "also",
        "into",
        "as",
    ]
    .into_iter()
    .collect()
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Default)]
pub enum RemoveStopWordsValue {
    #[default]
    Disabled,
    All,
    Languages(Vec<String>),
}

impl RemoveStopWordsValue {
    pub fn is_enabled_for(&self, lang: &str) -> bool {
        match self {
            RemoveStopWordsValue::Disabled => false,
            RemoveStopWordsValue::All => true,
            RemoveStopWordsValue::Languages(langs) => langs.iter().any(|l| l == lang),
        }
    }
}

impl serde::Serialize for RemoveStopWordsValue {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        match self {
            RemoveStopWordsValue::Disabled => serializer.serialize_bool(false),
            RemoveStopWordsValue::All => serializer.serialize_bool(true),
            RemoveStopWordsValue::Languages(langs) => langs.serialize(serializer),
        }
    }
}

impl<'de> serde::Deserialize<'de> for RemoveStopWordsValue {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        use serde::de;

        struct Visitor;
        impl<'de> de::Visitor<'de> for Visitor {
            type Value = RemoveStopWordsValue;

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("bool or array of language codes")
            }

            fn visit_bool<E: de::Error>(self, v: bool) -> std::result::Result<Self::Value, E> {
                if v {
                    Ok(RemoveStopWordsValue::All)
                } else {
                    Ok(RemoveStopWordsValue::Disabled)
                }
            }

            fn visit_seq<A: de::SeqAccess<'de>>(
                self,
                mut seq: A,
            ) -> std::result::Result<Self::Value, A::Error> {
                let mut langs = Vec::new();
                while let Some(val) = seq.next_element::<String>()? {
                    langs.push(val);
                }
                if langs.is_empty() {
                    Ok(RemoveStopWordsValue::Disabled)
                } else {
                    Ok(RemoveStopWordsValue::Languages(langs))
                }
            }
        }

        deserializer.deserialize_any(Visitor)
    }
}

fn stop_words_for_lang(lang: &str) -> Option<HashSet<&'static str>> {
    match lang {
        "en" => Some(english_stop_words()),
        _ => None,
    }
}

pub fn remove_stop_words(query: &str, setting: &RemoveStopWordsValue, query_type: &str) -> String {
    let langs: Vec<&str> = match setting {
        RemoveStopWordsValue::Disabled => return query.to_string(),
        RemoveStopWordsValue::All => vec!["en"],
        RemoveStopWordsValue::Languages(langs) => langs.iter().map(|s| s.as_str()).collect(),
    };

    let mut all_stop_words = HashSet::new();
    for lang in &langs {
        if let Some(sw) = stop_words_for_lang(lang) {
            all_stop_words.extend(sw);
        }
    }

    if all_stop_words.is_empty() {
        return query.to_string();
    }

    let words: Vec<&str> = query.split_whitespace().collect();
    if words.is_empty() {
        return query.to_string();
    }

    let trailing_space = query.ends_with(' ');
    let last_idx = words.len() - 1;

    let filtered: Vec<&str> = words
        .iter()
        .enumerate()
        .filter(|(i, w)| {
            let is_prefix_token = match query_type {
                "prefixAll" => true,
                "prefixLast" => *i == last_idx && !trailing_space,
                _ => false,
            };
            if is_prefix_token {
                return true;
            }
            !all_stop_words.contains(w.to_lowercase().as_str())
        })
        .map(|(_, w)| *w)
        .collect();

    if filtered.is_empty() {
        return query.to_string();
    }

    let mut result = filtered.join(" ");
    if trailing_space {
        result.push(' ');
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_disabled_noop() {
        let r = remove_stop_words(
            "the best search engine",
            &RemoveStopWordsValue::Disabled,
            "prefixLast",
        );
        assert_eq!(r, "the best search engine");
    }

    #[test]
    fn test_basic_removal() {
        let r = remove_stop_words(
            "the best search engine",
            &RemoveStopWordsValue::All,
            "prefixNone",
        );
        assert_eq!(r, "best search engine");
    }

    #[test]
    fn test_prefix_last_preserves_last_word() {
        let r = remove_stop_words("what is the", &RemoveStopWordsValue::All, "prefixLast");
        assert_eq!(r, "the");
    }

    #[test]
    fn test_prefix_last_trailing_space_strips_last() {
        let r = remove_stop_words("what is the ", &RemoveStopWordsValue::All, "prefixLast");
        assert_eq!(r, "what is the ");
    }

    #[test]
    fn test_prefix_all_preserves_all() {
        let r = remove_stop_words("the a is", &RemoveStopWordsValue::All, "prefixAll");
        assert_eq!(r, "the a is");
    }

    #[test]
    fn test_all_stop_words_preserves_original() {
        let r = remove_stop_words("the a an", &RemoveStopWordsValue::All, "prefixNone");
        assert_eq!(r, "the a an");
    }

    #[test]
    fn test_language_specific() {
        let r = remove_stop_words(
            "the best engine",
            &RemoveStopWordsValue::Languages(vec!["en".to_string()]),
            "prefixNone",
        );
        assert_eq!(r, "best engine");
    }

    #[test]
    fn test_unsupported_language_noop() {
        let r = remove_stop_words(
            "the best engine",
            &RemoveStopWordsValue::Languages(vec!["xx".to_string()]),
            "prefixNone",
        );
        assert_eq!(r, "the best engine");
    }

    #[test]
    fn test_case_insensitive() {
        let r = remove_stop_words(
            "The Best IS engine",
            &RemoveStopWordsValue::All,
            "prefixNone",
        );
        assert_eq!(r, "Best engine");
    }

    #[test]
    fn test_empty_query() {
        let r = remove_stop_words("", &RemoveStopWordsValue::All, "prefixNone");
        assert_eq!(r, "");
    }

    #[test]
    fn test_serde_bool_true() {
        let v: RemoveStopWordsValue = serde_json::from_str("true").unwrap();
        assert_eq!(v, RemoveStopWordsValue::All);
        assert_eq!(serde_json::to_string(&v).unwrap(), "true");
    }

    #[test]
    fn test_serde_bool_false() {
        let v: RemoveStopWordsValue = serde_json::from_str("false").unwrap();
        assert_eq!(v, RemoveStopWordsValue::Disabled);
        assert_eq!(serde_json::to_string(&v).unwrap(), "false");
    }

    #[test]
    fn test_serde_languages() {
        let v: RemoveStopWordsValue = serde_json::from_str(r#"["en","fr"]"#).unwrap();
        assert_eq!(
            v,
            RemoveStopWordsValue::Languages(vec!["en".to_string(), "fr".to_string()])
        );
        assert_eq!(serde_json::to_string(&v).unwrap(), r#"["en","fr"]"#);
    }

    #[test]
    fn test_mixed_stop_and_content_words() {
        let r = remove_stop_words(
            "how to build a search engine",
            &RemoveStopWordsValue::All,
            "prefixLast",
        );
        assert_eq!(r, "build search engine");
    }

    #[test]
    fn test_preserves_trailing_space() {
        let r = remove_stop_words("best search ", &RemoveStopWordsValue::All, "prefixLast");
        assert_eq!(r, "best search ");
    }
}
