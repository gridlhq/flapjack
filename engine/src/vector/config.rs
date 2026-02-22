use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::VectorError;

/// Source type for an embedder configuration.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EmbedderSource {
    OpenAi,
    Rest,
    #[default]
    UserProvided,
    FastEmbed,
}

/// Configuration for creating an embedder.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
#[serde(default)]
pub struct EmbedderConfig {
    pub source: EmbedderSource,
    pub model: Option<String>,
    pub api_key: Option<String>,
    pub dimensions: Option<usize>,
    pub url: Option<String>,
    pub request: Option<serde_json::Value>,
    pub response: Option<serde_json::Value>,
    pub headers: Option<HashMap<String, String>>,
    pub document_template: Option<String>,
    pub document_template_max_bytes: Option<usize>,
}

impl EmbedderConfig {
    /// Build a DocumentTemplate from this embedder's template configuration.
    pub fn document_template(&self) -> DocumentTemplate {
        DocumentTemplate::new(
            self.document_template.clone(),
            self.document_template_max_bytes,
        )
    }

    /// Validate that required fields are present for the given source type.
    pub fn validate(&self) -> Result<(), VectorError> {
        match self.source {
            EmbedderSource::OpenAi => {
                if self.api_key.is_none() {
                    return Err(VectorError::EmbeddingError(
                        "openAi embedder requires `apiKey`".into(),
                    ));
                }
            }
            EmbedderSource::Rest => {
                let mut missing = Vec::new();
                if self.url.is_none() {
                    missing.push("`url`");
                }
                if self.request.is_none() {
                    missing.push("`request`");
                }
                if self.response.is_none() {
                    missing.push("`response`");
                }
                if !missing.is_empty() {
                    return Err(VectorError::EmbeddingError(format!(
                        "rest embedder requires {}",
                        missing.join(", ")
                    )));
                }
            }
            EmbedderSource::UserProvided => {
                if self.dimensions.is_none() {
                    return Err(VectorError::EmbeddingError(
                        "userProvided embedder requires `dimensions`".into(),
                    ));
                }
            }
            EmbedderSource::FastEmbed => {
                // No mandatory fields — model defaults to bge-small-en-v1.5.
                // Dimension validation happens in FastEmbedEmbedder::new() where
                // the model info is available.
            }
        }
        Ok(())
    }
}

/// A single entry in the embedder fingerprint, capturing the semantic-relevant
/// fields of one embedder configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EmbedderFingerprintEntry {
    pub name: String,
    pub source: EmbedderSource,
    pub model: Option<String>,
    pub dimensions: usize,
    pub document_template: Option<String>,
    pub document_template_max_bytes: Option<usize>,
}

/// Fingerprint capturing all embedder configurations for a tenant.
/// Used to detect when embedder settings change, invalidating stored vectors.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EmbedderFingerprint {
    pub version: u32,
    pub embedders: Vec<EmbedderFingerprintEntry>,
}

impl EmbedderFingerprint {
    /// Build a fingerprint from the current embedder configs and actual dimensions
    /// from the VectorIndex.
    pub fn from_configs(configs: &[(String, EmbedderConfig)], actual_dimensions: usize) -> Self {
        let mut entries: Vec<EmbedderFingerprintEntry> = configs
            .iter()
            .map(|(name, config)| EmbedderFingerprintEntry {
                name: name.clone(),
                source: config.source,
                model: config.model.clone(),
                dimensions: actual_dimensions,
                document_template: config.document_template.clone(),
                document_template_max_bytes: config.document_template_max_bytes,
            })
            .collect();
        entries.sort_by(|a, b| a.name.cmp(&b.name));
        Self {
            version: 1,
            embedders: entries,
        }
    }

    /// Check whether the current embedder configs match this fingerprint.
    /// Returns true if all semantic fields match (name, source, model, template).
    /// Dimensions: if config.dimensions is Some(n), checks n == entry.dimensions.
    /// If config.dimensions is None (auto-detect), skips dimension check.
    pub fn matches_configs(&self, configs: &[(String, EmbedderConfig)]) -> bool {
        let mut sorted: Vec<(String, &EmbedderConfig)> =
            configs.iter().map(|(n, c)| (n.clone(), c)).collect();
        sorted.sort_by(|a, b| a.0.cmp(&b.0));

        if sorted.len() != self.embedders.len() {
            return false;
        }

        for (entry, (name, config)) in self.embedders.iter().zip(sorted.iter()) {
            if entry.name != *name {
                return false;
            }
            if entry.source != config.source {
                return false;
            }
            if entry.model != config.model {
                return false;
            }
            if entry.document_template != config.document_template {
                return false;
            }
            if entry.document_template_max_bytes != config.document_template_max_bytes {
                return false;
            }
            // Dimensions: only check if config specifies them (Some).
            // None means auto-detect — matches any stored dimensions.
            if let Some(dim) = config.dimensions {
                if dim != entry.dimensions {
                    return false;
                }
            }
        }

        true
    }

    /// Save fingerprint to `{dir}/fingerprint.json`.
    pub fn save(&self, dir: &std::path::Path) -> Result<(), std::io::Error> {
        std::fs::create_dir_all(dir)?;
        let path = dir.join("fingerprint.json");
        let json = serde_json::to_string_pretty(self).map_err(std::io::Error::other)?;
        std::fs::write(&path, json)
    }

    /// Load fingerprint from `{dir}/fingerprint.json`.
    pub fn load(dir: &std::path::Path) -> Result<Self, std::io::Error> {
        let path = dir.join("fingerprint.json");
        let data = std::fs::read_to_string(&path)?;
        serde_json::from_str(&data)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
    }
}

/// Document template for rendering searchable text from JSON documents.
pub struct DocumentTemplate {
    pub template: Option<String>,
    pub max_bytes: usize,
}

impl Default for DocumentTemplate {
    fn default() -> Self {
        Self {
            template: None,
            max_bytes: 400,
        }
    }
}

impl DocumentTemplate {
    pub fn new(template: Option<String>, max_bytes: Option<usize>) -> Self {
        Self {
            template,
            max_bytes: max_bytes.unwrap_or(400),
        }
    }

    /// Render a JSON document into a searchable text string.
    ///
    /// If a template is set, substitute `{{doc.field_name}}` patterns.
    /// If no template, concatenate all string values separated by `. `.
    /// Truncate to `max_bytes` at a UTF-8 boundary.
    pub fn render(&self, document: &serde_json::Value) -> String {
        let result = match &self.template {
            Some(tmpl) => Self::render_template(tmpl, document),
            None => Self::render_default(document),
        };
        truncate_utf8(&result, self.max_bytes)
    }

    /// Substitute `{{doc.field.path}}` placeholders with values from the document.
    fn render_template(template: &str, document: &serde_json::Value) -> String {
        let mut result = String::new();
        let mut rest = template;
        while let Some(start) = rest.find("{{doc.") {
            result.push_str(&rest[..start]);
            let after_open = &rest[start + 6..]; // skip "{{doc."
            if let Some(end) = after_open.find("}}") {
                let field_path = &after_open[..end];
                let value = resolve_path(document, field_path);
                result.push_str(value);
                rest = &after_open[end + 2..];
            } else {
                // No closing }}, copy the remainder literally and stop
                result.push_str(&rest[start..]);
                return result;
            }
        }
        result.push_str(rest);
        result
    }

    /// Default: concatenate all top-level user string values separated by `. `.
    /// Skips internal fields (`_id`, `objectID`) which carry no semantic meaning.
    fn render_default(document: &serde_json::Value) -> String {
        let obj = match document.as_object() {
            Some(o) => o,
            None => return String::new(),
        };
        let parts: Vec<&str> = obj
            .iter()
            .filter(|(k, _)| *k != "_id" && *k != "objectID")
            .filter_map(|(_, v)| v.as_str())
            .collect();
        parts.join(". ")
    }
}

/// Navigate a dot-separated path into a JSON value, returning the string value or "".
fn resolve_path<'a>(value: &'a serde_json::Value, path: &str) -> &'a str {
    let mut current = value;
    for key in path.split('.') {
        match current.get(key) {
            Some(v) => current = v,
            None => return "",
        }
    }
    current.as_str().unwrap_or("")
}

/// Truncate a string to at most `max_bytes` at a UTF-8 char boundary.
fn truncate_utf8(s: &str, max_bytes: usize) -> String {
    if s.len() <= max_bytes {
        return s.to_owned();
    }
    // Find the last valid char boundary at or before max_bytes
    let mut end = max_bytes;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    s[..end].to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Config validation tests (3.5) ──

    #[test]
    fn test_openai_config_requires_api_key() {
        let config = EmbedderConfig {
            source: EmbedderSource::OpenAi,
            ..Default::default()
        };
        let result = config.validate();
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            VectorError::EmbeddingError(_)
        ));
    }

    #[test]
    fn test_rest_config_requires_url_and_templates() {
        // Missing all three
        let config = EmbedderConfig {
            source: EmbedderSource::Rest,
            ..Default::default()
        };
        assert!(config.validate().is_err());

        // Has url but missing request and response
        let config = EmbedderConfig {
            source: EmbedderSource::Rest,
            url: Some("http://example.com".into()),
            ..Default::default()
        };
        assert!(config.validate().is_err());

        // Has url + request but missing response
        let config = EmbedderConfig {
            source: EmbedderSource::Rest,
            url: Some("http://example.com".into()),
            request: Some(serde_json::json!({"input": "{{text}}"})),
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_user_provided_requires_dimensions() {
        let config = EmbedderConfig {
            source: EmbedderSource::UserProvided,
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_valid_configs_pass_validation() {
        let openai = EmbedderConfig {
            source: EmbedderSource::OpenAi,
            api_key: Some("sk-test".into()),
            ..Default::default()
        };
        assert!(openai.validate().is_ok());

        let rest = EmbedderConfig {
            source: EmbedderSource::Rest,
            url: Some("http://example.com/embed".into()),
            request: Some(serde_json::json!({"input": "{{text}}"})),
            response: Some(serde_json::json!({"embedding": "{{embedding}}"})),
            ..Default::default()
        };
        assert!(rest.validate().is_ok());

        let user_provided = EmbedderConfig {
            source: EmbedderSource::UserProvided,
            dimensions: Some(384),
            ..Default::default()
        };
        assert!(user_provided.validate().is_ok());
    }

    #[test]
    fn test_config_serde_roundtrip() {
        let config = EmbedderConfig {
            source: EmbedderSource::OpenAi,
            api_key: Some("sk-test".into()),
            model: Some("text-embedding-3-small".into()),
            dimensions: Some(1536),
            ..Default::default()
        };
        let json = serde_json::to_string(&config).unwrap();
        // Verify camelCase serialization
        assert!(json.contains("apiKey"));
        assert!(json.contains("openAi"));
        assert!(!json.contains("api_key"));

        let roundtripped: EmbedderConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(roundtripped.source, EmbedderSource::OpenAi);
        assert_eq!(roundtripped.api_key.as_deref(), Some("sk-test"));
        assert_eq!(
            roundtripped.model.as_deref(),
            Some("text-embedding-3-small")
        );
        assert_eq!(roundtripped.dimensions, Some(1536));
    }

    // ── Document template tests (3.26) ──

    #[test]
    fn test_template_field_substitution() {
        let tmpl = DocumentTemplate::new(Some("{{doc.title}} {{doc.body}}".into()), None);
        let doc = serde_json::json!({
            "title": "MacBook Pro",
            "body": "The new MacBook is fast"
        });
        assert_eq!(tmpl.render(&doc), "MacBook Pro The new MacBook is fast");
    }

    #[test]
    fn test_template_missing_field() {
        let tmpl = DocumentTemplate::new(Some("{{doc.title}} by {{doc.author}}".into()), None);
        let doc = serde_json::json!({"title": "Hello"});
        assert_eq!(tmpl.render(&doc), "Hello by ");
    }

    #[test]
    fn test_template_default_all_fields() {
        let tmpl = DocumentTemplate::new(None, None);
        let doc = serde_json::json!({
            "title": "Hello",
            "body": "World",
            "count": 42
        });
        let result = tmpl.render(&doc);
        // Should concatenate string fields, skip non-strings
        assert!(result.contains("Hello"));
        assert!(result.contains("World"));
        assert!(!result.contains("42"));
    }

    #[test]
    fn test_template_default_excludes_id_fields() {
        let tmpl = DocumentTemplate::new(None, None);
        let doc = serde_json::json!({
            "_id": "abc-123-uuid",
            "objectID": "obj456",
            "title": "Hello",
            "body": "World"
        });
        let result = tmpl.render(&doc);
        // _id and objectID should be excluded — they are metadata, not content
        assert!(
            !result.contains("abc-123-uuid"),
            "default template should exclude _id"
        );
        assert!(
            !result.contains("obj456"),
            "default template should exclude objectID"
        );
        assert!(result.contains("Hello"));
        assert!(result.contains("World"));
    }

    #[test]
    fn test_template_max_bytes_truncation() {
        let tmpl = DocumentTemplate::new(None, Some(10));
        let doc = serde_json::json!({
            "body": "This is a long text that should be truncated"
        });
        let result = tmpl.render(&doc);
        assert!(result.len() <= 10);
    }

    #[test]
    fn test_template_nested_field() {
        let tmpl = DocumentTemplate::new(Some("{{doc.meta.author}}".into()), None);
        let doc = serde_json::json!({
            "meta": {"author": "Stuart"}
        });
        assert_eq!(tmpl.render(&doc), "Stuart");
    }

    #[test]
    fn test_template_unclosed_placeholder() {
        let tmpl = DocumentTemplate::new(Some("Hello {{doc.title and more text".into()), None);
        let doc = serde_json::json!({"title": "World"});
        let result = tmpl.render(&doc);
        // Unclosed placeholder should be preserved literally, no duplication
        assert_eq!(result, "Hello {{doc.title and more text");
    }

    // ── EmbedderConfig::document_template tests (7.4) ──

    #[test]
    fn test_document_template_from_embedder_config() {
        let config = EmbedderConfig {
            source: EmbedderSource::Rest,
            url: Some("http://example.com/embed".into()),
            request: Some(serde_json::json!({"input": "{{text}}"})),
            response: Some(serde_json::json!({"embedding": "{{embedding}}"})),
            document_template: Some("{{doc.title}} {{doc.body}}".into()),
            document_template_max_bytes: Some(200),
            ..Default::default()
        };
        let tmpl = config.document_template();
        let doc = serde_json::json!({
            "title": "MacBook Pro",
            "body": "The new MacBook is fast"
        });
        assert_eq!(tmpl.render(&doc), "MacBook Pro The new MacBook is fast");
        assert_eq!(tmpl.max_bytes, 200);
    }

    #[test]
    fn test_document_template_from_embedder_config_defaults() {
        let config = EmbedderConfig {
            source: EmbedderSource::UserProvided,
            dimensions: Some(384),
            ..Default::default()
        };
        let tmpl = config.document_template();
        // No template set → default behavior (all string fields, 400 bytes max)
        assert!(tmpl.template.is_none());
        assert_eq!(tmpl.max_bytes, 400);
        let doc = serde_json::json!({
            "title": "Hello",
            "body": "World"
        });
        let result = tmpl.render(&doc);
        assert!(result.contains("Hello"));
        assert!(result.contains("World"));
    }

    #[test]
    fn test_template_default_non_string_fields_only() {
        let tmpl = DocumentTemplate::new(None, None);
        let doc = serde_json::json!({
            "count": 42,
            "active": true,
            "price": 9.99
        });
        let result = tmpl.render(&doc);
        assert!(
            result.is_empty(),
            "default template should skip non-string fields, got: {result:?}"
        );
    }

    #[test]
    fn test_template_default_excludes_vectors_object() {
        // _vectors as an object should not be rendered (as_str returns None for objects)
        let tmpl = DocumentTemplate::new(None, None);
        let doc = serde_json::json!({
            "title": "Hello",
            "_vectors": { "default": [0.1, 0.2, 0.3] }
        });
        let result = tmpl.render(&doc);
        assert!(result.contains("Hello"));
        assert!(
            !result.contains("0.1"),
            "vectors object should not be rendered as text"
        );
        assert!(
            !result.contains("default"),
            "vectors embedder name should not leak into text"
        );
    }

    #[test]
    fn test_template_utf8_truncation_boundary() {
        // Multi-byte UTF-8: each emoji is 4 bytes
        let tmpl = DocumentTemplate::new(None, Some(5));
        let doc = serde_json::json!({
            "text": "\u{1F600}\u{1F601}\u{1F602}"  // 3 emojis = 12 bytes
        });
        let result = tmpl.render(&doc);
        // max_bytes=5, first emoji is 4 bytes, second starts at byte 4 and ends at byte 8
        // So only 1 emoji fits (4 bytes <= 5, but 8 bytes > 5)
        assert!(result.len() <= 5);
        assert_eq!(
            result.chars().count(),
            1,
            "should truncate to 1 emoji at char boundary"
        );
    }

    // ── EmbedderFingerprint tests (8.13) ──

    #[test]
    fn test_fingerprint_from_configs() {
        let configs = vec![
            (
                "beta".to_string(),
                EmbedderConfig {
                    source: EmbedderSource::Rest,
                    model: Some("model-b".into()),
                    dimensions: Some(768),
                    ..Default::default()
                },
            ),
            (
                "alpha".to_string(),
                EmbedderConfig {
                    source: EmbedderSource::OpenAi,
                    model: Some("model-a".into()),
                    dimensions: Some(1536),
                    ..Default::default()
                },
            ),
        ];
        let fp = EmbedderFingerprint::from_configs(&configs, 1536);
        assert_eq!(fp.version, 1);
        assert_eq!(fp.embedders.len(), 2);
        // Should be sorted by name
        assert_eq!(fp.embedders[0].name, "alpha");
        assert_eq!(fp.embedders[1].name, "beta");
        assert_eq!(fp.embedders[0].source, EmbedderSource::OpenAi);
        assert_eq!(fp.embedders[1].source, EmbedderSource::Rest);
    }

    #[test]
    fn test_fingerprint_matches_same_configs() {
        let configs = vec![(
            "default".to_string(),
            EmbedderConfig {
                source: EmbedderSource::Rest,
                model: Some("text-embedding-3-small".into()),
                dimensions: Some(1536),
                document_template: Some("{{doc.title}}".into()),
                document_template_max_bytes: Some(400),
                ..Default::default()
            },
        )];
        let fp = EmbedderFingerprint::from_configs(&configs, 1536);
        assert!(fp.matches_configs(&configs));
    }

    #[test]
    fn test_fingerprint_mismatch_different_model() {
        let configs_v1 = vec![(
            "default".to_string(),
            EmbedderConfig {
                source: EmbedderSource::OpenAi,
                model: Some("text-embedding-3-small".into()),
                dimensions: Some(1536),
                ..Default::default()
            },
        )];
        let fp = EmbedderFingerprint::from_configs(&configs_v1, 1536);

        let configs_v2 = vec![(
            "default".to_string(),
            EmbedderConfig {
                source: EmbedderSource::OpenAi,
                model: Some("text-embedding-3-large".into()),
                dimensions: Some(1536),
                ..Default::default()
            },
        )];
        assert!(!fp.matches_configs(&configs_v2));
    }

    #[test]
    fn test_fingerprint_mismatch_different_source() {
        let configs_v1 = vec![(
            "default".to_string(),
            EmbedderConfig {
                source: EmbedderSource::OpenAi,
                model: Some("model-a".into()),
                dimensions: Some(1536),
                ..Default::default()
            },
        )];
        let fp = EmbedderFingerprint::from_configs(&configs_v1, 1536);

        let configs_v2 = vec![(
            "default".to_string(),
            EmbedderConfig {
                source: EmbedderSource::Rest,
                model: Some("model-a".into()),
                dimensions: Some(1536),
                ..Default::default()
            },
        )];
        assert!(!fp.matches_configs(&configs_v2));
    }

    #[test]
    fn test_fingerprint_mismatch_different_dimensions() {
        let configs_v1 = vec![(
            "default".to_string(),
            EmbedderConfig {
                source: EmbedderSource::OpenAi,
                model: Some("model-a".into()),
                dimensions: Some(1536),
                ..Default::default()
            },
        )];
        let fp = EmbedderFingerprint::from_configs(&configs_v1, 1536);

        let configs_v2 = vec![(
            "default".to_string(),
            EmbedderConfig {
                source: EmbedderSource::OpenAi,
                model: Some("model-a".into()),
                dimensions: Some(768),
                ..Default::default()
            },
        )];
        assert!(!fp.matches_configs(&configs_v2));
    }

    #[test]
    fn test_fingerprint_dimensions_none_in_config_matches_any() {
        let configs_v1 = vec![(
            "default".to_string(),
            EmbedderConfig {
                source: EmbedderSource::OpenAi,
                model: Some("model-a".into()),
                dimensions: Some(1536),
                ..Default::default()
            },
        )];
        let fp = EmbedderFingerprint::from_configs(&configs_v1, 1536);

        // Config with dimensions=None (auto-detect) should match any fingerprint dimensions
        let configs_v2 = vec![(
            "default".to_string(),
            EmbedderConfig {
                source: EmbedderSource::OpenAi,
                model: Some("model-a".into()),
                dimensions: None,
                ..Default::default()
            },
        )];
        assert!(fp.matches_configs(&configs_v2));
    }

    #[test]
    fn test_fingerprint_mismatch_embedder_added() {
        let configs_v1 = vec![(
            "default".to_string(),
            EmbedderConfig {
                source: EmbedderSource::OpenAi,
                model: Some("model-a".into()),
                dimensions: Some(1536),
                ..Default::default()
            },
        )];
        let fp = EmbedderFingerprint::from_configs(&configs_v1, 1536);

        let configs_v2 = vec![
            (
                "default".to_string(),
                EmbedderConfig {
                    source: EmbedderSource::OpenAi,
                    model: Some("model-a".into()),
                    dimensions: Some(1536),
                    ..Default::default()
                },
            ),
            (
                "secondary".to_string(),
                EmbedderConfig {
                    source: EmbedderSource::Rest,
                    model: Some("model-b".into()),
                    dimensions: Some(768),
                    ..Default::default()
                },
            ),
        ];
        assert!(!fp.matches_configs(&configs_v2));
    }

    #[test]
    fn test_fingerprint_mismatch_embedder_removed() {
        let configs_v1 = vec![
            (
                "default".to_string(),
                EmbedderConfig {
                    source: EmbedderSource::OpenAi,
                    model: Some("model-a".into()),
                    dimensions: Some(1536),
                    ..Default::default()
                },
            ),
            (
                "secondary".to_string(),
                EmbedderConfig {
                    source: EmbedderSource::Rest,
                    model: Some("model-b".into()),
                    dimensions: Some(768),
                    ..Default::default()
                },
            ),
        ];
        let fp = EmbedderFingerprint::from_configs(&configs_v1, 1536);

        let configs_v2 = vec![(
            "default".to_string(),
            EmbedderConfig {
                source: EmbedderSource::OpenAi,
                model: Some("model-a".into()),
                dimensions: Some(1536),
                ..Default::default()
            },
        )];
        assert!(!fp.matches_configs(&configs_v2));
    }

    #[test]
    fn test_fingerprint_mismatch_template_changed() {
        let configs_v1 = vec![(
            "default".to_string(),
            EmbedderConfig {
                source: EmbedderSource::Rest,
                model: Some("model-a".into()),
                dimensions: Some(1536),
                document_template: Some("{{doc.title}} {{doc.body}}".into()),
                ..Default::default()
            },
        )];
        let fp = EmbedderFingerprint::from_configs(&configs_v1, 1536);

        let configs_v2 = vec![(
            "default".to_string(),
            EmbedderConfig {
                source: EmbedderSource::Rest,
                model: Some("model-a".into()),
                dimensions: Some(1536),
                document_template: Some("{{doc.title}}".into()),
                ..Default::default()
            },
        )];
        assert!(!fp.matches_configs(&configs_v2));
    }

    // ── FastEmbed source tests (9.2) ──

    #[test]
    fn test_fastembed_source_serde() {
        let source = EmbedderSource::FastEmbed;
        let json = serde_json::to_string(&source).unwrap();
        assert_eq!(json, "\"fastEmbed\"");
        let deserialized: EmbedderSource = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, EmbedderSource::FastEmbed);
    }

    #[test]
    fn test_fastembed_config_validate_ok() {
        let config = EmbedderConfig {
            source: EmbedderSource::FastEmbed,
            ..Default::default()
        };
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_fastembed_config_validate_no_mandatory_fields() {
        let config = EmbedderConfig {
            source: EmbedderSource::FastEmbed,
            model: None,
            api_key: None,
            dimensions: None,
            url: None,
            request: None,
            response: None,
            headers: None,
            document_template: None,
            document_template_max_bytes: None,
        };
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_fingerprint_save_and_load_roundtrip() {
        let configs = vec![(
            "default".to_string(),
            EmbedderConfig {
                source: EmbedderSource::Rest,
                model: Some("text-embedding-3-small".into()),
                dimensions: Some(1536),
                document_template: Some("{{doc.title}} {{doc.body}}".into()),
                document_template_max_bytes: Some(400),
                ..Default::default()
            },
        )];
        let fp = EmbedderFingerprint::from_configs(&configs, 1536);

        let tmp = tempfile::TempDir::new().unwrap();
        fp.save(tmp.path()).unwrap();

        let loaded = EmbedderFingerprint::load(tmp.path()).unwrap();
        assert_eq!(fp, loaded);
        assert!(loaded.matches_configs(&configs));
    }

    // ── FastEmbed fingerprint tests (9.13) ──

    #[test]
    fn test_fingerprint_fastembed_source() {
        let configs = vec![(
            "default".to_string(),
            EmbedderConfig {
                source: EmbedderSource::FastEmbed,
                model: Some("bge-small-en-v1.5".into()),
                dimensions: Some(384),
                ..Default::default()
            },
        )];
        let fp = EmbedderFingerprint::from_configs(&configs, 384);

        let tmp = tempfile::TempDir::new().unwrap();
        fp.save(tmp.path()).unwrap();

        let loaded = EmbedderFingerprint::load(tmp.path()).unwrap();
        assert_eq!(fp, loaded);
        assert!(loaded.matches_configs(&configs));
        assert_eq!(loaded.embedders[0].source, EmbedderSource::FastEmbed);
    }

    #[test]
    fn test_fingerprint_fastembed_model_change_mismatch() {
        let configs_v1 = vec![(
            "default".to_string(),
            EmbedderConfig {
                source: EmbedderSource::FastEmbed,
                model: Some("bge-small-en-v1.5".into()),
                dimensions: Some(384),
                ..Default::default()
            },
        )];
        let fp = EmbedderFingerprint::from_configs(&configs_v1, 384);

        let configs_v2 = vec![(
            "default".to_string(),
            EmbedderConfig {
                source: EmbedderSource::FastEmbed,
                model: Some("all-MiniLM-L6-v2".into()),
                dimensions: Some(384),
                ..Default::default()
            },
        )];
        assert!(
            !fp.matches_configs(&configs_v2),
            "different model should not match"
        );
    }
}
