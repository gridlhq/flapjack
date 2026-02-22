use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Tenant (index) identifier — a plain string like `"products"`.
pub type TenantId = String;
/// Document identifier — matches `objectID` in the Algolia convention.
pub type DocumentId = String;

/// A document with an ID and a set of named fields.
///
/// Use [`Document::from_json`] to parse from a JSON object, or construct
/// directly for the manual writer API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    pub id: DocumentId,
    pub fields: HashMap<String, FieldValue>,
}

impl Document {
    /// Parse a [`Document`] from a JSON object.
    ///
    /// Accepts either `"objectID"` (Algolia convention) or `"_id"` as the
    /// document identifier. All other fields are converted to [`FieldValue`]s.
    ///
    /// # Errors
    ///
    /// Returns [`crate::FlapjackError::MissingField`] if neither `objectID` nor
    /// `_id` is present, or [`crate::FlapjackError::InvalidDocument`] if the value
    /// is not a JSON object.
    pub fn from_json(json: &serde_json::Value) -> crate::error::Result<Self> {
        use crate::error::FlapjackError;

        let obj = json
            .as_object()
            .ok_or_else(|| FlapjackError::InvalidDocument("Expected JSON object".to_string()))?;

        // Accept both "_id" (internal) and "objectID" (Algolia-compatible)
        let id = obj
            .get("_id")
            .or_else(|| obj.get("objectID"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| FlapjackError::MissingField("objectID".to_string()))?
            .to_string();

        let mut fields = HashMap::new();
        for (key, val) in obj {
            if key == "_id" || key == "objectID" {
                continue;
            }
            if let Some(field_value) = json_value_to_field_value(val) {
                fields.insert(key.clone(), field_value);
            }
        }

        Ok(Document { id, fields })
    }

    /// Convert Document back to flat JSON format (Algolia-compatible)
    /// Returns {"_id": "...", "field1": value1, "field2": value2, ...}
    pub fn to_json(&self) -> serde_json::Value {
        let mut map = serde_json::Map::new();
        map.insert(
            "_id".to_string(),
            serde_json::Value::String(self.id.clone()),
        );

        for (key, field_value) in &self.fields {
            map.insert(key.clone(), field_value_to_json_value(field_value));
        }

        serde_json::Value::Object(map)
    }
}

pub fn json_value_to_field_value(val: &serde_json::Value) -> Option<FieldValue> {
    match val {
        serde_json::Value::String(s) => Some(FieldValue::Text(s.clone())),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Some(FieldValue::Integer(i))
            } else {
                n.as_f64().map(FieldValue::Float)
            }
        }
        serde_json::Value::Array(arr) => {
            let items: Vec<FieldValue> = arr.iter().filter_map(json_value_to_field_value).collect();
            if items.is_empty() {
                None
            } else {
                Some(FieldValue::Array(items))
            }
        }
        serde_json::Value::Object(obj) => {
            let mut nested = std::collections::HashMap::new();
            for (k, v) in obj {
                if let Some(field_val) = json_value_to_field_value(v) {
                    nested.insert(k.clone(), field_val);
                }
            }
            if nested.is_empty() {
                None
            } else {
                Some(FieldValue::Object(nested))
            }
        }
        serde_json::Value::Null => None,
        serde_json::Value::Bool(_) => None,
    }
}

pub fn field_value_to_json_value(field_value: &FieldValue) -> serde_json::Value {
    match field_value {
        FieldValue::Text(s) => serde_json::Value::String(s.clone()),
        FieldValue::Integer(i) => serde_json::json!(i),
        FieldValue::Float(f) => serde_json::json!(f),
        FieldValue::Date(d) => serde_json::json!(d),
        FieldValue::Facet(f) => serde_json::Value::String(f.clone()),
        FieldValue::Array(arr) => {
            let items: Vec<serde_json::Value> = arr.iter().map(field_value_to_json_value).collect();
            serde_json::Value::Array(items)
        }
        FieldValue::Object(obj) => {
            let mut map = serde_json::Map::new();
            for (k, v) in obj {
                map.insert(k.clone(), field_value_to_json_value(v));
            }
            serde_json::Value::Object(map)
        }
    }
}

/// A dynamically-typed field value stored in a [`Document`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum FieldValue {
    Object(std::collections::HashMap<String, FieldValue>),
    Array(Vec<FieldValue>),
    Text(String),
    Integer(i64),
    Float(f64),
    Date(i64),
    Facet(String),
}

impl FieldValue {
    pub fn as_text(&self) -> Option<&str> {
        match self {
            FieldValue::Text(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_integer(&self) -> Option<i64> {
        match self {
            FieldValue::Integer(i) => Some(*i),
            _ => None,
        }
    }

    pub fn as_float(&self) -> Option<f64> {
        match self {
            FieldValue::Float(f) => Some(*f),
            _ => None,
        }
    }

    pub fn as_date(&self) -> Option<i64> {
        match self {
            FieldValue::Date(d) => Some(*d),
            _ => None,
        }
    }

    pub fn as_facet(&self) -> Option<&str> {
        match self {
            FieldValue::Facet(s) => Some(s),
            _ => None,
        }
    }
}

/// A search query (text only). Used internally by the query parser.
#[derive(Debug, Clone)]
pub struct Query {
    pub text: String,
}

/// A composable filter tree for narrowing search results.
///
/// Filters can be combined with [`Filter::And`] and [`Filter::Or`].
#[derive(Debug, Clone)]
pub enum Filter {
    Equals { field: String, value: FieldValue },
    NotEquals { field: String, value: FieldValue },
    GreaterThan { field: String, value: FieldValue },
    GreaterThanOrEqual { field: String, value: FieldValue },
    LessThan { field: String, value: FieldValue },
    LessThanOrEqual { field: String, value: FieldValue },
    Range { field: String, min: f64, max: f64 },
    Not(Box<Filter>),
    And(Vec<Filter>),
    Or(Vec<Filter>),
}

#[derive(Debug, Clone)]
pub enum SortOrder {
    Asc,
    Desc,
}

#[derive(Debug, Clone)]
pub enum Sort {
    ByField { field: String, order: SortOrder },
    ByRelevance,
}

/// Request facet counts for a specific field.
#[derive(Debug, Clone)]
pub struct FacetRequest {
    /// The field name (e.g. `"category"`).
    pub field: String,
    /// The Tantivy facet path prefix (e.g. `"/category"`).
    pub path: String,
}

/// Results returned by [`IndexManager::search`](crate::IndexManager::search).
#[derive(Debug, Clone)]
pub struct SearchResult {
    /// Matching documents sorted by relevance (or custom sort).
    pub documents: Vec<ScoredDocument>,
    /// Total number of matching documents (before pagination).
    pub total: usize,
    /// Facet counts keyed by field name.
    pub facets: HashMap<String, Vec<FacetCount>>,
    /// User data injected by query rules.
    pub user_data: Vec<serde_json::Value>,
    /// IDs of query rules that fired.
    pub applied_rules: Vec<String>,
}

/// A single facet value and its document count.
#[derive(Debug, Clone)]
pub struct FacetCount {
    pub path: String,
    pub count: u64,
}

/// A document paired with its relevance score.
#[derive(Debug, Clone)]
pub struct ScoredDocument {
    pub document: Document,
    pub score: f32,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskInfo {
    pub id: String,
    pub numeric_id: i64,
    pub status: TaskStatus,
    pub received_documents: usize,
    pub indexed_documents: usize,
    pub rejected_documents: Vec<DocFailure>,
    pub rejected_count: usize,
    pub created_at: std::time::SystemTime,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TaskStatus {
    Enqueued,
    Processing,
    Succeeded,
    Failed(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocFailure {
    pub doc_id: String,
    pub error: String,
    pub message: String,
}

impl TaskInfo {
    pub fn new(id: String, numeric_id: i64, received_documents: usize) -> Self {
        TaskInfo {
            id,
            numeric_id,
            status: TaskStatus::Enqueued,
            received_documents,
            indexed_documents: 0,
            rejected_documents: Vec::new(),
            rejected_count: 0,
            created_at: std::time::SystemTime::now(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Document::from_json ---

    #[test]
    fn from_json_with_object_id() {
        let json = serde_json::json!({"objectID": "abc", "title": "Hello"});
        let doc = Document::from_json(&json).unwrap();
        assert_eq!(doc.id, "abc");
        assert_eq!(doc.fields["title"], FieldValue::Text("Hello".to_string()));
    }

    #[test]
    fn from_json_with_underscore_id() {
        let json = serde_json::json!({"_id": "xyz", "name": "Test"});
        let doc = Document::from_json(&json).unwrap();
        assert_eq!(doc.id, "xyz");
    }

    #[test]
    fn from_json_underscore_id_takes_priority() {
        // _id is checked before objectID
        let json = serde_json::json!({"_id": "first", "objectID": "second", "v": 1});
        let doc = Document::from_json(&json).unwrap();
        assert_eq!(doc.id, "first");
    }

    #[test]
    fn from_json_missing_id_errors() {
        let json = serde_json::json!({"title": "No ID"});
        assert!(Document::from_json(&json).is_err());
    }

    #[test]
    fn from_json_not_object_errors() {
        let json = serde_json::json!("just a string");
        assert!(Document::from_json(&json).is_err());
    }

    #[test]
    fn from_json_id_not_in_fields() {
        let json = serde_json::json!({"objectID": "1", "color": "red"});
        let doc = Document::from_json(&json).unwrap();
        assert!(!doc.fields.contains_key("objectID"));
        assert!(!doc.fields.contains_key("_id"));
    }

    #[test]
    fn from_json_numeric_fields() {
        let json = serde_json::json!({"objectID": "1", "count": 42, "price": 9.99});
        let doc = Document::from_json(&json).unwrap();
        assert_eq!(doc.fields["count"], FieldValue::Integer(42));
        assert_eq!(doc.fields["price"], FieldValue::Float(9.99));
    }

    #[test]
    fn from_json_array_field() {
        let json = serde_json::json!({"objectID": "1", "tags": ["a", "b"]});
        let doc = Document::from_json(&json).unwrap();
        assert_eq!(
            doc.fields["tags"],
            FieldValue::Array(vec![
                FieldValue::Text("a".to_string()),
                FieldValue::Text("b".to_string()),
            ])
        );
    }

    #[test]
    fn from_json_nested_object() {
        let json = serde_json::json!({"objectID": "1", "meta": {"color": "red"}});
        let doc = Document::from_json(&json).unwrap();
        if let FieldValue::Object(obj) = &doc.fields["meta"] {
            assert_eq!(obj["color"], FieldValue::Text("red".to_string()));
        } else {
            panic!("expected Object");
        }
    }

    #[test]
    fn from_json_null_and_bool_skipped() {
        let json =
            serde_json::json!({"objectID": "1", "active": true, "deleted": null, "name": "ok"});
        let doc = Document::from_json(&json).unwrap();
        assert!(!doc.fields.contains_key("active"));
        assert!(!doc.fields.contains_key("deleted"));
        assert!(doc.fields.contains_key("name"));
    }

    // --- Document::to_json roundtrip ---

    #[test]
    fn to_json_roundtrip() {
        let mut fields = HashMap::new();
        fields.insert("title".to_string(), FieldValue::Text("Hello".to_string()));
        fields.insert("count".to_string(), FieldValue::Integer(5));
        let doc = Document {
            id: "abc".to_string(),
            fields,
        };
        let json = doc.to_json();
        assert_eq!(json["_id"], "abc");
        assert_eq!(json["title"], "Hello");
        assert_eq!(json["count"], 5);
    }

    // --- json_value_to_field_value ---

    #[test]
    fn json_string_to_text() {
        let v = serde_json::json!("hello");
        assert_eq!(
            json_value_to_field_value(&v),
            Some(FieldValue::Text("hello".to_string()))
        );
    }

    #[test]
    fn json_integer_to_integer() {
        let v = serde_json::json!(42);
        assert_eq!(json_value_to_field_value(&v), Some(FieldValue::Integer(42)));
    }

    #[test]
    fn json_float_to_float() {
        let v = serde_json::json!(2.5);
        assert_eq!(json_value_to_field_value(&v), Some(FieldValue::Float(2.5)));
    }

    #[test]
    fn json_null_returns_none() {
        assert_eq!(json_value_to_field_value(&serde_json::Value::Null), None);
    }

    #[test]
    fn json_bool_returns_none() {
        assert_eq!(json_value_to_field_value(&serde_json::json!(true)), None);
    }

    #[test]
    fn json_empty_array_returns_none() {
        assert_eq!(json_value_to_field_value(&serde_json::json!([])), None);
    }

    #[test]
    fn json_empty_object_returns_none() {
        assert_eq!(json_value_to_field_value(&serde_json::json!({})), None);
    }

    // --- field_value_to_json_value ---

    #[test]
    fn text_roundtrip() {
        let fv = FieldValue::Text("hello".to_string());
        assert_eq!(field_value_to_json_value(&fv), serde_json::json!("hello"));
    }

    #[test]
    fn integer_roundtrip() {
        let fv = FieldValue::Integer(42);
        assert_eq!(field_value_to_json_value(&fv), serde_json::json!(42));
    }

    #[test]
    fn float_roundtrip() {
        let fv = FieldValue::Float(2.5);
        assert_eq!(field_value_to_json_value(&fv), serde_json::json!(2.5));
    }

    #[test]
    fn facet_to_string() {
        let fv = FieldValue::Facet("/electronics/phones".to_string());
        assert_eq!(
            field_value_to_json_value(&fv),
            serde_json::json!("/electronics/phones")
        );
    }

    #[test]
    fn array_roundtrip() {
        let fv = FieldValue::Array(vec![
            FieldValue::Text("a".to_string()),
            FieldValue::Integer(1),
        ]);
        assert_eq!(field_value_to_json_value(&fv), serde_json::json!(["a", 1]));
    }

    // --- FieldValue accessors ---

    #[test]
    fn as_text_some() {
        assert_eq!(FieldValue::Text("hi".to_string()).as_text(), Some("hi"));
    }

    #[test]
    fn as_text_none_for_integer() {
        assert_eq!(FieldValue::Integer(1).as_text(), None);
    }

    #[test]
    fn as_integer_some() {
        assert_eq!(FieldValue::Integer(42).as_integer(), Some(42));
    }

    #[test]
    fn as_integer_none_for_text() {
        assert_eq!(FieldValue::Text("x".to_string()).as_integer(), None);
    }

    #[test]
    fn as_float_some() {
        assert_eq!(FieldValue::Float(1.5).as_float(), Some(1.5));
    }

    #[test]
    fn as_float_none_for_text() {
        assert_eq!(FieldValue::Text("x".to_string()).as_float(), None);
    }

    #[test]
    fn as_facet_some() {
        assert_eq!(FieldValue::Facet("/a".to_string()).as_facet(), Some("/a"));
    }

    #[test]
    fn as_facet_none_for_integer() {
        assert_eq!(FieldValue::Integer(1).as_facet(), None);
    }

    #[test]
    fn as_date_some() {
        assert_eq!(FieldValue::Date(1000).as_date(), Some(1000));
    }

    #[test]
    fn as_date_none_for_text() {
        assert_eq!(FieldValue::Text("x".to_string()).as_date(), None);
    }
}
