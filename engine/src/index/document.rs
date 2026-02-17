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
