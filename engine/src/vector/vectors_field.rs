use std::collections::HashMap;

use super::VectorError;
use crate::types::Document;

/// Extract user-provided vectors from a document's JSON representation.
/// Returns `Ok(None)` if `_vectors` field is absent.
/// Returns `Ok(Some(HashMap))` with per-embedder results: Ok(vec) for valid, Err for invalid.
/// Returns `Err` if `_vectors` is present but not a JSON object (malformed input).
pub fn extract_vectors(
    doc_json: &serde_json::Value,
) -> Result<Option<HashMap<String, Result<Vec<f32>, VectorError>>>, VectorError> {
    let vectors_val = match doc_json.get("_vectors") {
        Some(v) => v,
        None => return Ok(None),
    };

    let vectors_obj = vectors_val.as_object().ok_or_else(|| {
        VectorError::EmbeddingError(format!(
            "_vectors must be a JSON object mapping embedder names to vectors, got {}",
            value_type_name(vectors_val)
        ))
    })?;

    let mut result = HashMap::with_capacity(vectors_obj.len());
    for (embedder_name, value) in vectors_obj {
        result.insert(embedder_name.clone(), parse_vector(value));
    }
    Ok(Some(result))
}

/// Parse a single vector value: expects a JSON array of numbers.
fn parse_vector(value: &serde_json::Value) -> Result<Vec<f32>, VectorError> {
    let arr = value.as_array().ok_or_else(|| {
        VectorError::EmbeddingError(format!(
            "_vectors value must be an array of floats, got {}",
            value_type_name(value)
        ))
    })?;

    arr.iter()
        .enumerate()
        .map(|(i, v)| {
            v.as_f64().map(|f| f as f32).ok_or_else(|| {
                VectorError::EmbeddingError(format!(
                    "_vectors array element [{}] is not a number",
                    i
                ))
            })
        })
        .collect()
}

fn value_type_name(v: &serde_json::Value) -> &'static str {
    match v {
        serde_json::Value::Null => "null",
        serde_json::Value::Bool(_) => "boolean",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::String(_) => "string",
        serde_json::Value::Array(_) => "array",
        serde_json::Value::Object(_) => "object",
    }
}

/// Remove `_vectors` from a Document's fields in-place.
/// Call BEFORE to_tantivy() to prevent large float arrays from being indexed.
pub fn strip_vectors_from_document(doc: &mut Document) {
    doc.fields.remove("_vectors");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::FieldValue;

    #[test]
    fn test_extract_vectors_present() {
        let doc_json = serde_json::json!({
            "_id": "doc1",
            "title": "Hello",
            "_vectors": {
                "default": [0.1, 0.2, 0.3]
            }
        });
        let result = extract_vectors(&doc_json).unwrap();
        assert!(result.is_some());
        let map = result.unwrap();
        assert!(map.contains_key("default"));
        let vec = map["default"].as_ref().unwrap();
        assert_eq!(vec.len(), 3);
        assert!((vec[0] - 0.1).abs() < 0.001);
        assert!((vec[1] - 0.2).abs() < 0.001);
        assert!((vec[2] - 0.3).abs() < 0.001);
    }

    #[test]
    fn test_extract_vectors_absent() {
        let doc_json = serde_json::json!({
            "_id": "doc1",
            "title": "Hello"
        });
        let result = extract_vectors(&doc_json).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_extract_vectors_multiple_embedders() {
        let doc_json = serde_json::json!({
            "_id": "doc1",
            "_vectors": {
                "default": [0.1, 0.2, 0.3],
                "mymodel": [0.4, 0.5]
            }
        });
        let result = extract_vectors(&doc_json).unwrap();
        assert!(result.is_some());
        let map = result.unwrap();
        assert_eq!(map.len(), 2);
        assert!(map["default"].is_ok());
        assert_eq!(map["default"].as_ref().unwrap().len(), 3);
        assert!(map["mymodel"].is_ok());
        assert_eq!(map["mymodel"].as_ref().unwrap().len(), 2);
    }

    #[test]
    fn test_extract_vectors_invalid_not_array() {
        let doc_json = serde_json::json!({
            "_id": "doc1",
            "_vectors": {
                "default": "not_an_array"
            }
        });
        let result = extract_vectors(&doc_json).unwrap();
        assert!(result.is_some());
        let map = result.unwrap();
        assert!(map["default"].is_err());
        let err_msg = format!("{}", map["default"].as_ref().unwrap_err());
        assert!(
            err_msg.contains("string"),
            "error should mention the actual type, got: {err_msg}"
        );
    }

    #[test]
    fn test_extract_vectors_invalid_not_floats() {
        let doc_json = serde_json::json!({
            "_id": "doc1",
            "_vectors": {
                "default": ["a", "b"]
            }
        });
        let result = extract_vectors(&doc_json).unwrap();
        assert!(result.is_some());
        let map = result.unwrap();
        assert!(map["default"].is_err());
        let err_msg = format!("{}", map["default"].as_ref().unwrap_err());
        assert!(
            err_msg.contains("[0]"),
            "error should mention the array index, got: {err_msg}"
        );
    }

    #[test]
    fn test_extract_vectors_mixed_valid_invalid() {
        let doc_json = serde_json::json!({
            "_id": "doc1",
            "_vectors": {
                "default": [0.1, 0.2],
                "bad": "nope"
            }
        });
        let result = extract_vectors(&doc_json).unwrap();
        assert!(result.is_some());
        let map = result.unwrap();
        assert!(map["default"].is_ok());
        assert_eq!(map["default"].as_ref().unwrap(), &vec![0.1f32, 0.2f32]);
        assert!(map["bad"].is_err());
    }

    #[test]
    fn test_extract_vectors_not_object() {
        // _vectors as array — should be Err, not silently ignored
        let doc_json = serde_json::json!({
            "_id": "doc1",
            "_vectors": [0.1, 0.2, 0.3]
        });
        let result = extract_vectors(&doc_json);
        assert!(
            result.is_err(),
            "_vectors as array should return Err, not Ok(None)"
        );
        let err_msg = format!("{}", result.unwrap_err());
        assert!(
            err_msg.contains("object"),
            "error should mention expected type, got: {err_msg}"
        );

        // _vectors as string
        let doc_json = serde_json::json!({
            "_id": "doc1",
            "_vectors": "not_valid"
        });
        assert!(extract_vectors(&doc_json).is_err());

        // _vectors as null
        let doc_json = serde_json::json!({
            "_id": "doc1",
            "_vectors": null
        });
        assert!(extract_vectors(&doc_json).is_err());

        // _vectors as number
        let doc_json = serde_json::json!({
            "_id": "doc1",
            "_vectors": 42
        });
        assert!(extract_vectors(&doc_json).is_err());
    }

    #[test]
    fn test_extract_vectors_empty_object() {
        let doc_json = serde_json::json!({
            "_id": "doc1",
            "_vectors": {}
        });
        let result = extract_vectors(&doc_json).unwrap();
        assert!(
            result.is_some(),
            "_vectors: {{}} should return Some(empty map)"
        );
        let map = result.unwrap();
        assert!(map.is_empty());
    }

    #[test]
    fn test_extract_vectors_null_embedder_value() {
        let doc_json = serde_json::json!({
            "_id": "doc1",
            "_vectors": {
                "default": null
            }
        });
        let result = extract_vectors(&doc_json).unwrap();
        assert!(result.is_some());
        let map = result.unwrap();
        assert!(map["default"].is_err(), "null embedder value should be Err");
        let err_msg = format!("{}", map["default"].as_ref().unwrap_err());
        assert!(
            err_msg.contains("null"),
            "error should mention null type, got: {err_msg}"
        );
    }

    #[test]
    fn test_strip_vectors_from_document() {
        let mut fields = std::collections::HashMap::new();
        fields.insert("title".to_string(), FieldValue::Text("Hello".to_string()));
        fields.insert(
            "_vectors".to_string(),
            FieldValue::Object({
                let mut inner = std::collections::HashMap::new();
                inner.insert(
                    "default".to_string(),
                    FieldValue::Array(vec![FieldValue::Float(0.1), FieldValue::Float(0.2)]),
                );
                inner
            }),
        );
        let mut doc = Document {
            id: "doc1".to_string(),
            fields,
        };

        strip_vectors_from_document(&mut doc);

        assert!(!doc.fields.contains_key("_vectors"));
        assert!(doc.fields.contains_key("title"));
        assert_eq!(doc.fields["title"], FieldValue::Text("Hello".to_string()));
    }

    #[test]
    fn test_extract_vectors_empty_array() {
        let doc_json = serde_json::json!({
            "_id": "doc1",
            "_vectors": {
                "default": []
            }
        });
        let result = extract_vectors(&doc_json).unwrap();
        assert!(result.is_some());
        let map = result.unwrap();
        assert!(map["default"].is_ok());
        assert!(map["default"].as_ref().unwrap().is_empty());
    }

    #[test]
    fn test_extract_vectors_integer_values() {
        // JSON integers (no decimal point) should be parsed as f32 via as_f64()
        let doc_json = serde_json::json!({
            "_vectors": {
                "default": [1, 2, 3]
            }
        });
        let result = extract_vectors(&doc_json).unwrap().unwrap();
        let vec = result["default"].as_ref().unwrap();
        assert_eq!(vec, &[1.0f32, 2.0, 3.0]);
    }

    #[test]
    fn test_extract_vectors_mixed_types_in_array() {
        // Array where some elements are valid floats and one is a string
        let doc_json = serde_json::json!({
            "_vectors": {
                "default": [0.1, "bad", 0.3]
            }
        });
        let result = extract_vectors(&doc_json).unwrap().unwrap();
        assert!(result["default"].is_err(), "mixed-type array should fail");
        let err_msg = format!("{}", result["default"].as_ref().unwrap_err());
        assert!(
            err_msg.contains("[1]"),
            "error should report index of bad element, got: {err_msg}"
        );
    }

    #[test]
    fn test_strip_vectors_from_document_without_vectors() {
        let mut fields = std::collections::HashMap::new();
        fields.insert("title".to_string(), FieldValue::Text("Hello".to_string()));
        let mut doc = Document {
            id: "doc1".to_string(),
            fields,
        };

        // Should be a no-op, not panic
        strip_vectors_from_document(&mut doc);

        assert!(doc.fields.contains_key("title"));
        assert_eq!(doc.fields.len(), 1);
    }
}
