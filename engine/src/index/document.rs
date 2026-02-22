use crate::error::{FlapjackError, Result};
use crate::index::facet_translation::{extract_facet_paths, is_hierarchical_facet};
use crate::index::schema::Schema;
use crate::index::settings::IndexSettings;
use crate::types::{Document, DocumentId, FieldValue};
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use tantivy::schema::{Field, OwnedValue};
use tantivy::TantivyDocument;

pub fn json_to_tantivy_doc(
    json: &Value,
    id_field: Field,
    json_search_field: Field,
    json_filter_field: Field,
    json_exact_field: Field,
    facets_field: Field,
) -> Result<TantivyDocument> {
    let mut tantivy_doc = TantivyDocument::new();

    let obj = json
        .as_object()
        .ok_or_else(|| FlapjackError::InvalidDocument("Expected JSON object".to_string()))?;

    // Accept both "_id" (internal) and "objectID" (Algolia-compatible, user-facing)
    let id = obj
        .get("_id")
        .or_else(|| obj.get("objectID"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| FlapjackError::MissingField("objectID".to_string()))?;

    tantivy_doc.add_text(id_field, id);

    let mut json_fields = Map::new();
    for (key, val) in obj {
        if key == "_id" || key == "objectID" {
            continue;
        }
        json_fields.insert(key.clone(), val.clone());
    }

    let json_value = Value::Object(json_fields.clone());
    let (search_json, mut filter_json) = split_by_type(&json_value);
    if let Value::Object(ref mut filter_map) = filter_json {
        filter_map.insert("objectID".to_string(), Value::String(id.to_string()));
    }

    tantivy_doc.add_object(json_search_field, json_to_btree(&search_json)?);
    tantivy_doc.add_object(json_filter_field, json_to_btree(&filter_json)?);
    tantivy_doc.add_object(json_exact_field, json_to_btree(&search_json)?);

    for (field_name, value) in &json_fields {
        let paths = if is_hierarchical_facet(value) {
            extract_facet_paths(field_name, value)?
        } else if let Value::String(s) = value {
            vec![format!("/{}/{}", field_name, s)]
        } else {
            vec![]
        };

        for path in paths {
            tantivy_doc.add_facet(facets_field, tantivy::schema::Facet::from(&path));
        }
    }

    Ok(tantivy_doc)
}

pub struct DocumentConverter {
    id_field: Field,
    json_search_field: Field,
    json_filter_field: Field,
    json_exact_field: Field,
    facets_field: Field,
    geo_lat_field: Option<Field>,
    geo_lng_field: Option<Field>,
}

impl DocumentConverter {
    pub fn new(_schema: &Schema, tantivy_schema: &tantivy::schema::Schema) -> Result<Self> {
        let id_field = tantivy_schema
            .get_field("_id")
            .map_err(|_| FlapjackError::FieldNotFound("_id".to_string()))?;
        let json_search_field = tantivy_schema
            .get_field("_json_search")
            .map_err(|_| FlapjackError::FieldNotFound("_json_search".to_string()))?;
        let json_filter_field = tantivy_schema
            .get_field("_json_filter")
            .map_err(|_| FlapjackError::FieldNotFound("_json_filter".to_string()))?;
        let json_exact_field = tantivy_schema
            .get_field("_json_exact")
            .map_err(|_| FlapjackError::FieldNotFound("_json_exact".to_string()))?;
        let facets_field = tantivy_schema
            .get_field("_facets")
            .map_err(|_| FlapjackError::FieldNotFound("_facets".to_string()))?;
        let geo_lat_field = tantivy_schema.get_field("_geo_lat").ok();
        let geo_lng_field = tantivy_schema.get_field("_geo_lng").ok();

        Ok(DocumentConverter {
            id_field,
            json_search_field,
            json_filter_field,
            json_exact_field,
            facets_field,
            geo_lat_field,
            geo_lng_field,
        })
    }

    pub fn to_tantivy(
        &self,
        doc: &Document,
        settings: Option<&IndexSettings>,
    ) -> Result<TantivyDocument> {
        let mut tantivy_doc = TantivyDocument::new();

        tantivy_doc.add_text(self.id_field, &doc.id);

        let mut json_fields = fields_to_json(&doc.fields);

        if let Value::Object(ref mut map) = json_fields {
            if let Some(geoloc) = map.remove("_geoloc") {
                if let Some((lat, lng)) = extract_geoloc(&geoloc) {
                    if let Some(f) = self.geo_lat_field {
                        tantivy_doc.add_f64(f, lat);
                    }
                    if let Some(f) = self.geo_lng_field {
                        tantivy_doc.add_f64(f, lng);
                    }
                }
                if let Value::Object(ref mut filter_map) = json_fields {
                    filter_map.insert("_geoloc".to_string(), geoloc.clone());
                }
            }
        }

        let (search_json, mut filter_json) = split_by_type(&json_fields);
        if let Value::Object(ref mut filter_map) = filter_json {
            filter_map.insert("objectID".to_string(), Value::String(doc.id.clone()));
        }

        tantivy_doc.add_object(self.json_search_field, json_to_btree(&search_json)?);
        tantivy_doc.add_object(self.json_filter_field, json_to_btree(&filter_json)?);
        tantivy_doc.add_object(self.json_exact_field, json_to_btree(&search_json)?);

        let facet_fields: std::collections::HashSet<String> =
            settings.map(|s| s.facet_set()).unwrap_or_default();

        for (field_name, value) in json_fields.as_object().unwrap() {
            let dominated = facet_fields.contains(field_name)
                || facet_fields
                    .iter()
                    .any(|f| f.starts_with(&format!("{}.", field_name)));
            if !dominated {
                continue;
            }

            let paths = if is_hierarchical_facet(value) {
                extract_facet_paths(field_name, value)?
            } else if let Value::String(s) = value {
                let truncated = if s.len() > 1000 {
                    &s[..1000]
                } else {
                    s.as_str()
                };
                vec![format!("/{}/{}", field_name, truncated)]
            } else if let Value::Array(arr) = value {
                arr.iter()
                    .filter_map(|item| {
                        if let Value::String(s) = item {
                            let truncated = if s.len() > 1000 {
                                &s[..1000]
                            } else {
                                s.as_str()
                            };
                            Some(format!("/{}/{}", field_name, truncated))
                        } else {
                            None
                        }
                    })
                    .collect()
            } else {
                vec![]
            };

            for path in &paths {
                tantivy_doc.add_facet(self.facets_field, tantivy::schema::Facet::from(path));
            }
        }

        Ok(tantivy_doc)
    }

    pub fn from_tantivy(
        &self,
        tantivy_doc: TantivyDocument,
        _tantivy_schema: &tantivy::schema::Schema,
        _ignored_doc_id: DocumentId,
    ) -> Result<Document> {
        let doc_id = tantivy_doc
            .get_first(self.id_field)
            .and_then(|v| {
                let owned: tantivy::schema::OwnedValue = v.into();
                match owned {
                    tantivy::schema::OwnedValue::Str(s) => Some(s),
                    _ => None,
                }
            })
            .ok_or_else(|| FlapjackError::MissingField("_id".to_string()))?;

        let json_value = tantivy_doc
            .get_first(self.json_filter_field)
            .ok_or_else(|| FlapjackError::MissingField("_json_filter".to_string()))?;

        let owned: OwnedValue = json_value.into();
        let fields = owned_value_to_fields(&owned)?;

        Ok(Document { id: doc_id, fields })
    }
}

fn owned_value_to_fields(
    value: &OwnedValue,
) -> Result<std::collections::HashMap<String, FieldValue>> {
    match value {
        OwnedValue::Object(pairs) => {
            let mut fields = std::collections::HashMap::new();
            for (key, val) in pairs {
                if let Some(fv) = owned_to_field_value(val) {
                    fields.insert(key.clone(), fv);
                }
            }
            Ok(fields)
        }
        _ => Err(FlapjackError::InvalidDocument(
            "Expected object".to_string(),
        )),
    }
}

fn owned_to_field_value(value: &OwnedValue) -> Option<FieldValue> {
    match value {
        OwnedValue::Null => None,
        OwnedValue::Str(s) => Some(FieldValue::Text(s.clone())),
        OwnedValue::I64(i) => Some(FieldValue::Integer(*i)),
        OwnedValue::U64(u) => Some(FieldValue::Integer(*u as i64)),
        OwnedValue::F64(f) => Some(FieldValue::Float(*f)),
        OwnedValue::Bool(b) => Some(FieldValue::Text(b.to_string())),
        OwnedValue::Array(arr) => {
            let items: Vec<FieldValue> = arr.iter().filter_map(owned_to_field_value).collect();
            if items.is_empty() {
                None
            } else {
                Some(FieldValue::Array(items))
            }
        }
        OwnedValue::Object(pairs) => {
            let mut map = std::collections::HashMap::new();
            for (k, v) in pairs {
                if let Some(fv) = owned_to_field_value(v) {
                    map.insert(k.clone(), fv);
                }
            }
            if map.is_empty() {
                None
            } else {
                Some(FieldValue::Object(map))
            }
        }
        _ => None,
    }
}

fn fields_to_json(fields: &std::collections::HashMap<String, FieldValue>) -> Value {
    let mut map = Map::new();
    for (key, value) in fields {
        let json_value = field_value_to_json(value);
        map.insert(key.clone(), json_value);
    }
    Value::Object(map)
}

fn field_value_to_json(value: &FieldValue) -> Value {
    match value {
        FieldValue::Object(map) => {
            let mut obj = Map::new();
            for (k, v) in map {
                obj.insert(k.clone(), field_value_to_json(v));
            }
            Value::Object(obj)
        }
        FieldValue::Array(items) => Value::Array(items.iter().map(field_value_to_json).collect()),
        FieldValue::Text(s) => Value::String(s.clone()),
        FieldValue::Integer(i) => Value::Number(serde_json::Number::from(*i)),
        FieldValue::Float(f) => serde_json::Number::from_f64(*f)
            .map(Value::Number)
            .unwrap_or(Value::Null),
        FieldValue::Date(d) => Value::Number(serde_json::Number::from(*d)),
        FieldValue::Facet(s) => Value::String(s.clone()),
    }
}

#[allow(dead_code)]
fn json_to_fields(json: &Value) -> Result<std::collections::HashMap<String, FieldValue>> {
    let mut fields = std::collections::HashMap::new();

    if let Value::Object(obj) = json {
        for (key, val) in obj {
            let field_value = match val {
                Value::String(s) => FieldValue::Text(s.clone()),
                Value::Number(n) => {
                    if let Some(i) = n.as_i64() {
                        FieldValue::Integer(i)
                    } else if let Some(f) = n.as_f64() {
                        FieldValue::Float(f)
                    } else {
                        continue;
                    }
                }
                Value::Bool(_) => continue,
                Value::Null => continue,
                Value::Array(_) => continue,
                Value::Object(_) => continue,
            };
            fields.insert(key.clone(), field_value);
        }
    }

    Ok(fields)
}

#[allow(dead_code)]
fn json_to_fields_full(json: &Value) -> Result<std::collections::HashMap<String, FieldValue>> {
    let mut fields = std::collections::HashMap::new();

    if let Value::Object(obj) = json {
        for (key, val) in obj {
            if let Some(field_value) = json_value_to_field_value(val) {
                fields.insert(key.clone(), field_value);
            }
        }
    }

    Ok(fields)
}

#[allow(dead_code)]
fn json_value_to_field_value(val: &Value) -> Option<FieldValue> {
    match val {
        Value::String(s) => Some(FieldValue::Text(s.clone())),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Some(FieldValue::Integer(i))
            } else {
                n.as_f64().map(FieldValue::Float)
            }
        }
        Value::Array(arr) => {
            let items: Vec<FieldValue> = arr.iter().filter_map(json_value_to_field_value).collect();
            if items.is_empty() {
                None
            } else {
                Some(FieldValue::Array(items))
            }
        }
        Value::Object(obj) => {
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
        Value::Null => None,
        Value::Bool(_) => None,
    }
}

fn json_to_btree(value: &Value) -> Result<BTreeMap<String, OwnedValue>> {
    match value {
        Value::Object(map) => {
            let mut btree = BTreeMap::new();
            for (k, v) in map {
                btree.insert(k.clone(), json_value_to_owned(v)?);
            }
            Ok(btree)
        }
        _ => Err(FlapjackError::InvalidDocument(
            "Expected JSON object".to_string(),
        )),
    }
}

#[allow(dead_code)]
fn btree_to_vec(btree: BTreeMap<String, OwnedValue>) -> Vec<(String, OwnedValue)> {
    btree.into_iter().collect()
}

fn json_value_to_owned(value: &Value) -> Result<OwnedValue> {
    match value {
        Value::Null => Ok(OwnedValue::Null),
        Value::Bool(b) => Ok(OwnedValue::Bool(*b)),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(OwnedValue::I64(i))
            } else if let Some(u) = n.as_u64() {
                Ok(OwnedValue::U64(u))
            } else if let Some(f) = n.as_f64() {
                Ok(OwnedValue::F64(f))
            } else {
                Err(FlapjackError::InvalidDocument("Invalid number".to_string()))
            }
        }
        Value::String(s) => Ok(OwnedValue::Str(s.clone())),
        Value::Array(arr) => {
            let owned_arr: Result<Vec<OwnedValue>> = arr.iter().map(json_value_to_owned).collect();
            Ok(OwnedValue::Array(owned_arr?))
        }
        Value::Object(map) => {
            let mut pairs = Vec::new();
            for (k, v) in map {
                pairs.push((k.clone(), json_value_to_owned(v)?));
            }
            Ok(OwnedValue::Object(pairs))
        }
    }
}

fn extract_geoloc(value: &Value) -> Option<(f64, f64)> {
    match value {
        Value::Object(map) => {
            let lat = map.get("lat").and_then(|v| v.as_f64())?;
            let lng = map.get("lng").and_then(|v| v.as_f64())?;
            if (-90.0..=90.0).contains(&lat) && (-180.0..=180.0).contains(&lng) {
                Some((lat, lng))
            } else {
                None
            }
        }
        Value::Array(arr) => {
            if let Some(first) = arr.first() {
                extract_geoloc(first)
            } else {
                None
            }
        }
        _ => None,
    }
}

fn split_by_type(value: &Value) -> (Value, Value) {
    match value {
        Value::Object(map) => {
            let mut search = Map::new();
            let mut filter = Map::new();
            for (k, v) in map {
                if v.is_null() {
                    continue;
                }
                let (s, f) = split_by_type(v);
                if !s.is_null() {
                    search.insert(k.clone(), s);
                }
                filter.insert(k.clone(), f);
            }
            (Value::Object(search), Value::Object(filter))
        }
        Value::Array(arr) => {
            let strings: Vec<String> = arr
                .iter()
                .filter_map(|item| item.as_str().map(|s| s.to_string()))
                .collect();
            let search_val = if strings.is_empty() {
                Value::Null
            } else {
                Value::String(strings.join(" "))
            };
            (search_val, value.clone())
        }
        Value::String(_) => (value.clone(), value.clone()),
        Value::Number(_) | Value::Bool(_) => (Value::Null, value.clone()),
        Value::Null => (Value::Null, Value::Null),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::collections::HashMap;
    use tantivy::schema::OwnedValue;

    // ── owned_to_field_value ──────────────────────────────────────────────

    #[test]
    fn owned_null_returns_none() {
        assert!(owned_to_field_value(&OwnedValue::Null).is_none());
    }

    #[test]
    fn owned_str_to_text() {
        let v = owned_to_field_value(&OwnedValue::Str("hello".to_string()));
        assert_eq!(v, Some(FieldValue::Text("hello".to_string())));
    }

    #[test]
    fn owned_i64_to_integer() {
        let v = owned_to_field_value(&OwnedValue::I64(42));
        assert_eq!(v, Some(FieldValue::Integer(42)));
    }

    #[test]
    fn owned_u64_to_integer() {
        let v = owned_to_field_value(&OwnedValue::U64(100));
        assert_eq!(v, Some(FieldValue::Integer(100)));
    }

    #[test]
    fn owned_f64_to_float() {
        let v = owned_to_field_value(&OwnedValue::F64(3.14));
        assert_eq!(v, Some(FieldValue::Float(3.14)));
    }

    #[test]
    fn owned_bool_to_text() {
        let v = owned_to_field_value(&OwnedValue::Bool(true));
        assert_eq!(v, Some(FieldValue::Text("true".to_string())));
    }

    #[test]
    fn owned_array_of_strings() {
        let arr = OwnedValue::Array(vec![
            OwnedValue::Str("a".to_string()),
            OwnedValue::Str("b".to_string()),
        ]);
        let v = owned_to_field_value(&arr);
        assert_eq!(
            v,
            Some(FieldValue::Array(vec![
                FieldValue::Text("a".to_string()),
                FieldValue::Text("b".to_string()),
            ]))
        );
    }

    #[test]
    fn owned_empty_array_returns_none() {
        let arr = OwnedValue::Array(vec![]);
        assert!(owned_to_field_value(&arr).is_none());
    }

    #[test]
    fn owned_array_with_only_nulls_returns_none() {
        let arr = OwnedValue::Array(vec![OwnedValue::Null, OwnedValue::Null]);
        assert!(owned_to_field_value(&arr).is_none());
    }

    #[test]
    fn owned_object_to_field_value() {
        let obj = OwnedValue::Object(vec![
            ("x".to_string(), OwnedValue::I64(1)),
            ("y".to_string(), OwnedValue::I64(2)),
        ]);
        match owned_to_field_value(&obj) {
            Some(FieldValue::Object(map)) => {
                assert_eq!(map.get("x"), Some(&FieldValue::Integer(1)));
                assert_eq!(map.get("y"), Some(&FieldValue::Integer(2)));
            }
            other => panic!("expected Object, got {:?}", other),
        }
    }

    #[test]
    fn owned_empty_object_returns_none() {
        let obj = OwnedValue::Object(vec![]);
        assert!(owned_to_field_value(&obj).is_none());
    }

    // ── owned_value_to_fields ────────────────────────────────────────────

    #[test]
    fn owned_value_to_fields_basic() {
        let obj = OwnedValue::Object(vec![
            ("name".to_string(), OwnedValue::Str("Laptop".to_string())),
            ("price".to_string(), OwnedValue::I64(999)),
        ]);
        let fields = owned_value_to_fields(&obj).unwrap();
        assert_eq!(
            fields.get("name"),
            Some(&FieldValue::Text("Laptop".to_string()))
        );
        assert_eq!(fields.get("price"), Some(&FieldValue::Integer(999)));
    }

    #[test]
    fn owned_value_to_fields_rejects_non_object() {
        let v = OwnedValue::Str("not an object".to_string());
        assert!(owned_value_to_fields(&v).is_err());
    }

    #[test]
    fn owned_value_to_fields_skips_null() {
        let obj = OwnedValue::Object(vec![
            ("a".to_string(), OwnedValue::Str("ok".to_string())),
            ("b".to_string(), OwnedValue::Null),
        ]);
        let fields = owned_value_to_fields(&obj).unwrap();
        assert_eq!(fields.len(), 1);
        assert!(fields.contains_key("a"));
    }

    // ── field_value_to_json ──────────────────────────────────────────────

    #[test]
    fn fv_text_to_json() {
        let v = field_value_to_json(&FieldValue::Text("hello".to_string()));
        assert_eq!(v, json!("hello"));
    }

    #[test]
    fn fv_integer_to_json() {
        let v = field_value_to_json(&FieldValue::Integer(42));
        assert_eq!(v, json!(42));
    }

    #[test]
    fn fv_float_to_json() {
        let v = field_value_to_json(&FieldValue::Float(3.14));
        assert_eq!(v, json!(3.14));
    }

    #[test]
    fn fv_float_nan_to_json_null() {
        let v = field_value_to_json(&FieldValue::Float(f64::NAN));
        assert_eq!(v, Value::Null);
    }

    #[test]
    fn fv_date_to_json() {
        let v = field_value_to_json(&FieldValue::Date(1000));
        assert_eq!(v, json!(1000));
    }

    #[test]
    fn fv_facet_to_json() {
        let v = field_value_to_json(&FieldValue::Facet("Electronics".to_string()));
        assert_eq!(v, json!("Electronics"));
    }

    #[test]
    fn fv_array_to_json() {
        let arr = FieldValue::Array(vec![
            FieldValue::Text("a".to_string()),
            FieldValue::Integer(1),
        ]);
        let v = field_value_to_json(&arr);
        assert_eq!(v, json!(["a", 1]));
    }

    #[test]
    fn fv_object_to_json() {
        let mut map = HashMap::new();
        map.insert("x".to_string(), FieldValue::Integer(1));
        let v = field_value_to_json(&FieldValue::Object(map));
        assert_eq!(v, json!({"x": 1}));
    }

    // ── fields_to_json roundtrip ─────────────────────────────────────────

    #[test]
    fn fields_to_json_roundtrip() {
        let mut fields = HashMap::new();
        fields.insert("name".to_string(), FieldValue::Text("Widget".to_string()));
        fields.insert("price".to_string(), FieldValue::Integer(10));
        let json_val = fields_to_json(&fields);
        let obj = json_val.as_object().unwrap();
        assert_eq!(obj["name"], json!("Widget"));
        assert_eq!(obj["price"], json!(10));
    }

    // ── json_value_to_owned ──────────────────────────────────────────────

    #[test]
    fn json_null_to_owned() {
        let v = json_value_to_owned(&Value::Null).unwrap();
        assert!(matches!(v, OwnedValue::Null));
    }

    #[test]
    fn json_bool_to_owned() {
        let v = json_value_to_owned(&json!(true)).unwrap();
        assert!(matches!(v, OwnedValue::Bool(true)));
    }

    #[test]
    fn json_int_to_owned() {
        let v = json_value_to_owned(&json!(42)).unwrap();
        assert!(matches!(v, OwnedValue::I64(42)));
    }

    #[test]
    fn json_float_to_owned() {
        let v = json_value_to_owned(&json!(3.14)).unwrap();
        match v {
            OwnedValue::F64(f) => assert!((f - 3.14).abs() < 1e-10),
            other => panic!("expected F64, got {:?}", other),
        }
    }

    #[test]
    fn json_string_to_owned() {
        let v = json_value_to_owned(&json!("hello")).unwrap();
        assert!(matches!(v, OwnedValue::Str(ref s) if s == "hello"));
    }

    #[test]
    fn json_array_to_owned() {
        let v = json_value_to_owned(&json!([1, "two"])).unwrap();
        match v {
            OwnedValue::Array(arr) => assert_eq!(arr.len(), 2),
            other => panic!("expected Array, got {:?}", other),
        }
    }

    #[test]
    fn json_object_to_owned() {
        let v = json_value_to_owned(&json!({"a": 1})).unwrap();
        match v {
            OwnedValue::Object(pairs) => {
                assert_eq!(pairs.len(), 1);
                assert_eq!(pairs[0].0, "a");
            }
            other => panic!("expected Object, got {:?}", other),
        }
    }

    // ── json_to_btree ────────────────────────────────────────────────────

    #[test]
    fn json_to_btree_basic() {
        let val = json!({"name": "Widget", "price": 10});
        let btree = json_to_btree(&val).unwrap();
        assert!(matches!(btree.get("name"), Some(OwnedValue::Str(s)) if s == "Widget"));
        assert!(matches!(btree.get("price"), Some(OwnedValue::I64(10))));
    }

    #[test]
    fn json_to_btree_rejects_non_object() {
        assert!(json_to_btree(&json!("string")).is_err());
        assert!(json_to_btree(&json!(42)).is_err());
        assert!(json_to_btree(&json!([1, 2])).is_err());
    }

    // ── extract_geoloc ───────────────────────────────────────────────────

    #[test]
    fn extract_geoloc_valid() {
        let v = json!({"lat": 48.8566, "lng": 2.3522});
        assert_eq!(extract_geoloc(&v), Some((48.8566, 2.3522)));
    }

    #[test]
    fn extract_geoloc_out_of_range_lat() {
        let v = json!({"lat": 91.0, "lng": 0.0});
        assert_eq!(extract_geoloc(&v), None);
    }

    #[test]
    fn extract_geoloc_out_of_range_lng() {
        let v = json!({"lat": 0.0, "lng": 181.0});
        assert_eq!(extract_geoloc(&v), None);
    }

    #[test]
    fn extract_geoloc_missing_lat() {
        let v = json!({"lng": 2.0});
        assert_eq!(extract_geoloc(&v), None);
    }

    #[test]
    fn extract_geoloc_missing_lng() {
        let v = json!({"lat": 48.0});
        assert_eq!(extract_geoloc(&v), None);
    }

    #[test]
    fn extract_geoloc_from_array() {
        let v = json!([{"lat": 48.8566, "lng": 2.3522}]);
        assert_eq!(extract_geoloc(&v), Some((48.8566, 2.3522)));
    }

    #[test]
    fn extract_geoloc_empty_array() {
        let v = json!([]);
        assert_eq!(extract_geoloc(&v), None);
    }

    #[test]
    fn extract_geoloc_string_returns_none() {
        let v = json!("not a geoloc");
        assert_eq!(extract_geoloc(&v), None);
    }

    #[test]
    fn extract_geoloc_boundary_values() {
        assert_eq!(
            extract_geoloc(&json!({"lat": 90.0, "lng": 180.0})),
            Some((90.0, 180.0))
        );
        assert_eq!(
            extract_geoloc(&json!({"lat": -90.0, "lng": -180.0})),
            Some((-90.0, -180.0))
        );
    }

    // ── split_by_type ────────────────────────────────────────────────────

    #[test]
    fn split_string_goes_to_both() {
        let (s, f) = split_by_type(&json!("hello"));
        assert_eq!(s, json!("hello"));
        assert_eq!(f, json!("hello"));
    }

    #[test]
    fn split_number_goes_to_filter_only() {
        let (s, f) = split_by_type(&json!(42));
        assert_eq!(s, Value::Null);
        assert_eq!(f, json!(42));
    }

    #[test]
    fn split_bool_goes_to_filter_only() {
        let (s, f) = split_by_type(&json!(true));
        assert_eq!(s, Value::Null);
        assert_eq!(f, json!(true));
    }

    #[test]
    fn split_null_gives_both_null() {
        let (s, f) = split_by_type(&Value::Null);
        assert_eq!(s, Value::Null);
        assert_eq!(f, Value::Null);
    }

    #[test]
    fn split_string_array_joins_for_search() {
        let v = json!(["red", "blue", "green"]);
        let (s, f) = split_by_type(&v);
        assert_eq!(s, json!("red blue green"));
        assert_eq!(f, json!(["red", "blue", "green"]));
    }

    #[test]
    fn split_numeric_array_no_search() {
        let v = json!([1, 2, 3]);
        let (s, _f) = split_by_type(&v);
        assert_eq!(s, Value::Null);
    }

    #[test]
    fn split_object_recurses() {
        let v = json!({"title": "Laptop", "price": 999});
        let (s, f) = split_by_type(&v);
        let search_obj = s.as_object().unwrap();
        let filter_obj = f.as_object().unwrap();
        // "title" is a string → in both search and filter
        assert_eq!(search_obj.get("title"), Some(&json!("Laptop")));
        assert_eq!(filter_obj.get("title"), Some(&json!("Laptop")));
        // "price" is a number → only in filter
        assert!(search_obj.get("price").is_none());
        assert_eq!(filter_obj.get("price"), Some(&json!(999)));
    }

    #[test]
    fn split_object_skips_null_fields() {
        let v = json!({"title": "Laptop", "removed": null});
        let (s, f) = split_by_type(&v);
        let search_obj = s.as_object().unwrap();
        let filter_obj = f.as_object().unwrap();
        assert!(search_obj.get("removed").is_none());
        assert!(filter_obj.get("removed").is_none());
    }

    // ── json_value_to_field_value ────────────────────────────────────────

    #[test]
    fn jv_string_to_text() {
        assert_eq!(
            json_value_to_field_value(&json!("hello")),
            Some(FieldValue::Text("hello".to_string()))
        );
    }

    #[test]
    fn jv_integer_to_integer() {
        assert_eq!(
            json_value_to_field_value(&json!(42)),
            Some(FieldValue::Integer(42))
        );
    }

    #[test]
    fn jv_float_to_float() {
        assert_eq!(
            json_value_to_field_value(&json!(3.14)),
            Some(FieldValue::Float(3.14))
        );
    }

    #[test]
    fn jv_null_returns_none() {
        assert_eq!(json_value_to_field_value(&Value::Null), None);
    }

    #[test]
    fn jv_bool_returns_none() {
        assert_eq!(json_value_to_field_value(&json!(true)), None);
    }

    #[test]
    fn jv_empty_array_returns_none() {
        assert_eq!(json_value_to_field_value(&json!([])), None);
    }

    #[test]
    fn jv_array_of_nulls_returns_none() {
        assert_eq!(json_value_to_field_value(&json!([null, null])), None);
    }

    #[test]
    fn jv_nested_object() {
        let v = json!({"x": 1, "y": "two"});
        match json_value_to_field_value(&v) {
            Some(FieldValue::Object(map)) => {
                assert_eq!(map.get("x"), Some(&FieldValue::Integer(1)));
                assert_eq!(map.get("y"), Some(&FieldValue::Text("two".to_string())));
            }
            other => panic!("expected Object, got {:?}", other),
        }
    }

    // ── json_to_fields (basic) ───────────────────────────────────────────

    #[test]
    fn json_to_fields_basic_types() {
        let v = json!({"name": "Widget", "price": 10, "weight": 1.5});
        let fields = json_to_fields(&v).unwrap();
        assert_eq!(
            fields.get("name"),
            Some(&FieldValue::Text("Widget".to_string()))
        );
        assert_eq!(fields.get("price"), Some(&FieldValue::Integer(10)));
        assert_eq!(fields.get("weight"), Some(&FieldValue::Float(1.5)));
    }

    #[test]
    fn json_to_fields_skips_non_primitives() {
        let v = json!({"name": "Widget", "tags": ["a", "b"], "meta": {"x": 1}, "flag": true, "nil": null});
        let fields = json_to_fields(&v).unwrap();
        assert_eq!(fields.len(), 1); // only "name"
    }

    #[test]
    fn json_to_fields_non_object_gives_empty() {
        let fields = json_to_fields(&json!("string")).unwrap();
        assert!(fields.is_empty());
    }
}
