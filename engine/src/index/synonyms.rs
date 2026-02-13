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
