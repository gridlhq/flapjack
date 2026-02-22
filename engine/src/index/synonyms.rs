use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
#[serde(rename_all = "lowercase")]
pub enum Synonym {
    #[serde(rename = "synonym")]
    Regular {
        #[serde(rename = "objectID")]
        object_id: String,
        synonyms: Vec<String>,
    },
    #[serde(rename = "onewaysynonym")]
    OneWay {
        #[serde(rename = "objectID")]
        object_id: String,
        input: String,
        synonyms: Vec<String>,
    },
    #[serde(rename = "altcorrection1")]
    AltCorrection1 {
        #[serde(rename = "objectID")]
        object_id: String,
        word: String,
        corrections: Vec<String>,
    },
    #[serde(rename = "altcorrection2")]
    AltCorrection2 {
        #[serde(rename = "objectID")]
        object_id: String,
        word: String,
        corrections: Vec<String>,
    },
    #[serde(rename = "placeholder")]
    Placeholder {
        #[serde(rename = "objectID")]
        object_id: String,
        placeholder: String,
        replacements: Vec<String>,
    },
}

impl Synonym {
    pub fn object_id(&self) -> &str {
        match self {
            Synonym::Regular { object_id, .. } => object_id,
            Synonym::OneWay { object_id, .. } => object_id,
            Synonym::AltCorrection1 { object_id, .. } => object_id,
            Synonym::AltCorrection2 { object_id, .. } => object_id,
            Synonym::Placeholder { object_id, .. } => object_id,
        }
    }

    pub fn synonym_type(&self) -> &str {
        match self {
            Synonym::Regular { .. } => "synonym",
            Synonym::OneWay { .. } => "onewaysynonym",
            Synonym::AltCorrection1 { .. } => "altcorrection1",
            Synonym::AltCorrection2 { .. } => "altcorrection2",
            Synonym::Placeholder { .. } => "placeholder",
        }
    }

    pub fn matches_text(&self, text: &str) -> bool {
        let lower = text.to_lowercase();
        match self {
            Synonym::Regular { synonyms, .. } => {
                synonyms.iter().any(|s| s.to_lowercase().contains(&lower))
            }
            Synonym::OneWay {
                input, synonyms, ..
            } => {
                input.to_lowercase().contains(&lower)
                    || synonyms.iter().any(|s| s.to_lowercase().contains(&lower))
            }
            Synonym::AltCorrection1 {
                word, corrections, ..
            }
            | Synonym::AltCorrection2 {
                word, corrections, ..
            } => {
                word.to_lowercase().contains(&lower)
                    || corrections
                        .iter()
                        .any(|c| c.to_lowercase().contains(&lower))
            }
            Synonym::Placeholder {
                placeholder,
                replacements,
                ..
            } => {
                placeholder.to_lowercase().contains(&lower)
                    || replacements
                        .iter()
                        .any(|r| r.to_lowercase().contains(&lower))
            }
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct SynonymStore {
    synonyms: HashMap<String, Synonym>,
}

impl SynonymStore {
    pub fn new() -> Self {
        Self {
            synonyms: HashMap::new(),
        }
    }

    pub fn load<P: AsRef<Path>>(path: P) -> crate::error::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let synonyms: Vec<Synonym> = serde_json::from_str(&content)?;

        let mut store = Self::new();
        for syn in synonyms {
            store.synonyms.insert(syn.object_id().to_string(), syn);
        }

        Ok(store)
    }

    pub fn save<P: AsRef<Path>>(&self, path: P) -> crate::error::Result<()> {
        let synonyms: Vec<&Synonym> = self.synonyms.values().collect();
        let content = serde_json::to_string_pretty(&synonyms)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    pub fn get(&self, object_id: &str) -> Option<&Synonym> {
        self.synonyms.get(object_id)
    }

    pub fn insert(&mut self, synonym: Synonym) {
        self.synonyms
            .insert(synonym.object_id().to_string(), synonym);
    }

    pub fn remove(&mut self, object_id: &str) -> Option<Synonym> {
        self.synonyms.remove(object_id)
    }

    pub fn clear(&mut self) {
        self.synonyms.clear();
    }

    pub fn search(
        &self,
        query: &str,
        synonym_type: Option<&str>,
        page: usize,
        hits_per_page: usize,
    ) -> (Vec<Synonym>, usize) {
        let filtered: Vec<Synonym> = self
            .synonyms
            .values()
            .filter(|syn| {
                if !query.is_empty() && !syn.matches_text(query) {
                    return false;
                }
                if let Some(typ) = synonym_type {
                    if syn.synonym_type() != typ {
                        return false;
                    }
                }
                true
            })
            .cloned()
            .collect();

        let total = filtered.len();
        let start = page * hits_per_page;
        let end = (start + hits_per_page).min(total);

        let page_items = if start < total {
            filtered[start..end].to_vec()
        } else {
            Vec::new()
        };

        (page_items, total)
    }

    pub fn expand_query(&self, query: &str) -> Vec<String> {
        let tokens: Vec<&str> = query.split_whitespace().collect();
        let mut expanded = vec![query.to_string()];

        for syn in self.synonyms.values() {
            match syn {
                Synonym::Regular { synonyms, .. } => {
                    for token in &tokens {
                        for s in synonyms {
                            if s.eq_ignore_ascii_case(token) {
                                for alt in synonyms {
                                    if !alt.eq_ignore_ascii_case(token) {
                                        let new_query = query.replace(token, alt);
                                        if !expanded.contains(&new_query) {
                                            expanded.push(new_query);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                Synonym::OneWay {
                    input, synonyms, ..
                } => {
                    if query.to_lowercase().contains(&input.to_lowercase()) {
                        for s in synonyms {
                            let new_query = query
                                .to_lowercase()
                                .replace(&input.to_lowercase(), &s.to_lowercase());
                            if !expanded.contains(&new_query) {
                                expanded.push(new_query);
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        expanded
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn regular(id: &str, words: &[&str]) -> Synonym {
        Synonym::Regular {
            object_id: id.to_string(),
            synonyms: words.iter().map(|s| s.to_string()).collect(),
        }
    }

    fn oneway(id: &str, input: &str, synonyms: &[&str]) -> Synonym {
        Synonym::OneWay {
            object_id: id.to_string(),
            input: input.to_string(),
            synonyms: synonyms.iter().map(|s| s.to_string()).collect(),
        }
    }

    // -- Synonym accessors --

    #[test]
    fn object_id_regular() {
        let s = regular("syn-1", &["a", "b"]);
        assert_eq!(s.object_id(), "syn-1");
    }

    #[test]
    fn object_id_oneway() {
        let s = oneway("ow-1", "phone", &["mobile"]);
        assert_eq!(s.object_id(), "ow-1");
    }

    #[test]
    fn synonym_type_variants() {
        assert_eq!(regular("r", &["a"]).synonym_type(), "synonym");
        assert_eq!(oneway("o", "x", &["y"]).synonym_type(), "onewaysynonym");
        let alt1 = Synonym::AltCorrection1 {
            object_id: "a".into(),
            word: "w".into(),
            corrections: vec![],
        };
        assert_eq!(alt1.synonym_type(), "altcorrection1");
        let ph = Synonym::Placeholder {
            object_id: "p".into(),
            placeholder: "<x>".into(),
            replacements: vec![],
        };
        assert_eq!(ph.synonym_type(), "placeholder");
    }

    // -- matches_text --

    #[test]
    fn matches_text_regular_case_insensitive() {
        let s = regular("1", &["Laptop", "Notebook"]);
        assert!(s.matches_text("laptop"));
        assert!(s.matches_text("NOTEBOOK"));
        assert!(!s.matches_text("phone"));
    }

    #[test]
    fn matches_text_oneway_input_and_synonyms() {
        let s = oneway("1", "phone", &["mobile", "cell"]);
        assert!(s.matches_text("phone"));
        assert!(s.matches_text("mobile"));
        assert!(!s.matches_text("tablet"));
    }

    #[test]
    fn matches_text_partial_match() {
        let s = regular("1", &["smartphone"]);
        assert!(s.matches_text("smart"), "substring should match (contains)");
    }

    // -- SynonymStore CRUD --

    #[test]
    fn store_insert_get_remove() {
        let mut store = SynonymStore::new();
        assert!(store.get("syn-1").is_none());

        store.insert(regular("syn-1", &["a", "b"]));
        assert!(store.get("syn-1").is_some());

        let removed = store.remove("syn-1");
        assert!(removed.is_some());
        assert!(store.get("syn-1").is_none());
    }

    #[test]
    fn store_clear() {
        let mut store = SynonymStore::new();
        store.insert(regular("1", &["a"]));
        store.insert(regular("2", &["b"]));
        store.clear();
        assert!(store.get("1").is_none());
        assert!(store.get("2").is_none());
    }

    #[test]
    fn store_insert_overwrites_same_id() {
        let mut store = SynonymStore::new();
        store.insert(regular("1", &["old"]));
        store.insert(regular("1", &["new"]));
        let syn = store.get("1").unwrap();
        match syn {
            Synonym::Regular { synonyms, .. } => assert_eq!(synonyms, &["new"]),
            _ => panic!("wrong variant"),
        }
    }

    // -- SynonymStore::search --

    #[test]
    fn search_empty_query_returns_all() {
        let mut store = SynonymStore::new();
        store.insert(regular("1", &["a"]));
        store.insert(regular("2", &["b"]));
        let (results, total) = store.search("", None, 0, 10);
        assert_eq!(total, 2);
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn search_filters_by_text() {
        let mut store = SynonymStore::new();
        store.insert(regular("1", &["laptop", "notebook"]));
        store.insert(regular("2", &["phone", "mobile"]));
        let (results, total) = store.search("laptop", None, 0, 10);
        assert_eq!(total, 1);
        assert_eq!(results[0].object_id(), "1");
    }

    #[test]
    fn search_filters_by_type() {
        let mut store = SynonymStore::new();
        store.insert(regular("1", &["a", "b"]));
        store.insert(oneway("2", "x", &["y"]));
        let (results, total) = store.search("", Some("onewaysynonym"), 0, 10);
        assert_eq!(total, 1);
        assert_eq!(results[0].object_id(), "2");
    }

    #[test]
    fn search_pagination() {
        let mut store = SynonymStore::new();
        for i in 0..5 {
            store.insert(regular(&format!("syn-{}", i), &[&format!("word-{}", i)]));
        }
        let (page0, total) = store.search("", None, 0, 2);
        assert_eq!(total, 5);
        assert_eq!(page0.len(), 2);

        let (page2, _) = store.search("", None, 2, 2);
        assert_eq!(page2.len(), 1, "last page should have 1 item");

        let (page_oob, _) = store.search("", None, 10, 2);
        assert!(page_oob.is_empty(), "out-of-bounds page should be empty");
    }

    // -- expand_query --

    #[test]
    fn expand_regular_synonym() {
        let mut store = SynonymStore::new();
        store.insert(regular("1", &["laptop", "notebook"]));

        let expanded = store.expand_query("laptop");
        assert!(expanded.contains(&"laptop".to_string()));
        assert!(expanded.contains(&"notebook".to_string()));
    }

    #[test]
    fn expand_oneway_synonym() {
        let mut store = SynonymStore::new();
        store.insert(oneway("1", "phone", &["mobile", "cell"]));

        let expanded = store.expand_query("phone");
        assert!(expanded.contains(&"phone".to_string()));
        assert!(expanded.contains(&"mobile".to_string()));
        assert!(expanded.contains(&"cell".to_string()));
    }

    #[test]
    fn expand_no_match_returns_original() {
        let mut store = SynonymStore::new();
        store.insert(regular("1", &["laptop", "notebook"]));

        let expanded = store.expand_query("phone");
        assert_eq!(expanded, vec!["phone"]);
    }

    #[test]
    fn expand_oneway_not_reverse() {
        let mut store = SynonymStore::new();
        store.insert(oneway("1", "phone", &["mobile"]));

        let expanded = store.expand_query("mobile");
        assert_eq!(
            expanded,
            vec!["mobile"],
            "one-way should not expand reverse direction"
        );
    }

    // -- serde round-trip --

    #[test]
    fn serde_regular_roundtrip() {
        let syn = regular("1", &["a", "b"]);
        let json = serde_json::to_string(&syn).unwrap();
        let back: Synonym = serde_json::from_str(&json).unwrap();
        assert_eq!(syn, back);
    }

    #[test]
    fn serde_oneway_roundtrip() {
        let syn = oneway("1", "phone", &["mobile"]);
        let json = serde_json::to_string(&syn).unwrap();
        let back: Synonym = serde_json::from_str(&json).unwrap();
        assert_eq!(syn, back);
    }
}
