use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use std::sync::Arc;

use super::AppState;
use crate::dto::{
    AddDocumentsRequest, AddDocumentsResponse, BatchOperation, DeleteByQueryRequest,
    GetObjectsRequest, GetObjectsResponse,
};
use crate::filter_parser::parse_filter;
use crate::pause_registry::check_not_paused;
use flapjack::error::FlapjackError;
use flapjack::types::{Document, FieldValue};

/// Apply a built-in partial update operation (Increment, Decrement, Add, Remove, AddUnique).
/// Returns the new FieldValue for the field, or None if the operation is invalid.
fn apply_operation(
    existing: Option<&FieldValue>,
    operation: &str,
    value: &serde_json::Value,
) -> Option<FieldValue> {
    match operation {
        "Increment" | "IncrementFrom" | "IncrementSet" => {
            let delta = value.as_f64().unwrap_or(0.0);
            match existing {
                Some(FieldValue::Integer(n)) => Some(FieldValue::Integer(*n + delta as i64)),
                Some(FieldValue::Float(n)) => Some(FieldValue::Float(*n + delta)),
                _ => {
                    // Field missing or non-numeric: create with delta value
                    if delta.fract() == 0.0 {
                        Some(FieldValue::Integer(delta as i64))
                    } else {
                        Some(FieldValue::Float(delta))
                    }
                }
            }
        }
        "Decrement" | "DecrementFrom" | "DecrementSet" => {
            let delta = value.as_f64().unwrap_or(0.0);
            match existing {
                Some(FieldValue::Integer(n)) => Some(FieldValue::Integer(*n - delta as i64)),
                Some(FieldValue::Float(n)) => Some(FieldValue::Float(*n - delta)),
                _ => {
                    if delta.fract() == 0.0 {
                        Some(FieldValue::Integer(-(delta as i64)))
                    } else {
                        Some(FieldValue::Float(-delta))
                    }
                }
            }
        }
        "Add" => {
            let new_item = flapjack::types::json_value_to_field_value(value)?;
            match existing {
                Some(FieldValue::Array(arr)) => {
                    let mut new_arr = arr.clone();
                    new_arr.push(new_item);
                    Some(FieldValue::Array(new_arr))
                }
                None => Some(FieldValue::Array(vec![new_item])),
                _ => {
                    // Non-array: wrap existing + new into array
                    Some(FieldValue::Array(vec![existing.unwrap().clone(), new_item]))
                }
            }
        }
        "Remove" => {
            let remove_json = serde_json::to_string(value).unwrap_or_default();
            match existing {
                Some(FieldValue::Array(arr)) => {
                    let new_arr: Vec<FieldValue> = arr
                        .iter()
                        .filter(|item| {
                            let item_json = serde_json::to_string(
                                &flapjack::types::field_value_to_json_value(item),
                            )
                            .unwrap_or_default();
                            item_json != remove_json
                        })
                        .cloned()
                        .collect();
                    Some(FieldValue::Array(new_arr))
                }
                _ => existing.cloned(),
            }
        }
        "AddUnique" => {
            let new_item = flapjack::types::json_value_to_field_value(value)?;
            let new_json = serde_json::to_string(value).unwrap_or_default();
            match existing {
                Some(FieldValue::Array(arr)) => {
                    let already_exists = arr.iter().any(|item| {
                        let item_json = serde_json::to_string(
                            &flapjack::types::field_value_to_json_value(item),
                        )
                        .unwrap_or_default();
                        item_json == new_json
                    });
                    if already_exists {
                        Some(FieldValue::Array(arr.clone()))
                    } else {
                        let mut new_arr = arr.clone();
                        new_arr.push(new_item);
                        Some(FieldValue::Array(new_arr))
                    }
                }
                None => Some(FieldValue::Array(vec![new_item])),
                _ => Some(FieldValue::Array(vec![existing.unwrap().clone(), new_item])),
            }
        }
        _ => None,
    }
}

/// Check if a JSON value is a built-in operation object (has `_operation` key).
fn is_operation(value: &serde_json::Value) -> bool {
    value
        .as_object()
        .map(|obj| obj.contains_key("_operation"))
        .unwrap_or(false)
}

/// Merge partial update fields into an existing document, or create a new one.
/// Returns `None` only when the document doesn't exist and `create_if_not_exists` is false.
fn merge_partial_update(
    existing: Option<Document>,
    object_id: &str,
    body: &serde_json::Map<String, serde_json::Value>,
    create_if_not_exists: bool,
) -> Result<Option<Document>, FlapjackError> {
    match existing {
        Some(doc) => {
            let mut fields = doc.fields.clone();
            for (k, v) in body {
                if k == "objectID" || k == "id" {
                    continue;
                }
                if is_operation(v) {
                    let obj = v.as_object().unwrap();
                    let op = obj.get("_operation").and_then(|o| o.as_str()).unwrap_or("");
                    let op_value = obj.get("value").unwrap_or(&serde_json::Value::Null);
                    if let Some(new_val) = apply_operation(fields.get(k), op, op_value) {
                        fields.insert(k.clone(), new_val);
                    }
                } else if let Some(field_val) = flapjack::types::json_value_to_field_value(v) {
                    fields.insert(k.clone(), field_val);
                }
            }
            Ok(Some(Document {
                id: object_id.to_string(),
                fields,
            }))
        }
        None => {
            if !create_if_not_exists {
                return Ok(None);
            }
            let mut json_obj = serde_json::Map::new();
            json_obj.insert(
                "_id".to_string(),
                serde_json::Value::String(object_id.to_string()),
            );
            // For new documents, apply operations to empty fields
            let mut fields_from_ops = std::collections::HashMap::new();
            for (k, v) in body {
                if k == "objectID" || k == "id" {
                    continue;
                }
                if is_operation(v) {
                    let obj = v.as_object().unwrap();
                    let op = obj.get("_operation").and_then(|o| o.as_str()).unwrap_or("");
                    let op_value = obj.get("value").unwrap_or(&serde_json::Value::Null);
                    if let Some(new_val) = apply_operation(None, op, op_value) {
                        fields_from_ops.insert(k.clone(), new_val);
                    }
                } else {
                    json_obj.insert(k.clone(), v.clone());
                }
            }
            let mut doc = Document::from_json(&serde_json::Value::Object(json_obj))?;
            for (k, v) in fields_from_ops {
                doc.fields.insert(k, v);
            }
            Ok(Some(doc))
        }
    }
}

use super::field_value_to_json;

pub async fn add_documents_batch_impl(
    State(state): State<Arc<AppState>>,
    index_name: String,
    req: AddDocumentsRequest,
) -> Result<Json<AddDocumentsResponse>, FlapjackError> {
    state.manager.create_tenant(&index_name)?;

    let mut object_ids = Vec::new();
    let mut documents = Vec::new();
    let mut deletes = Vec::new();
    let mut explicit_delete_count: u64 = 0;

    let operations = match req {
        AddDocumentsRequest::Batch { requests } => requests,
        AddDocumentsRequest::Legacy { documents: docs } => docs
            .into_iter()
            .map(|body| BatchOperation {
                action: "addObject".to_string(),
                body,
                create_if_not_exists: None,
            })
            .collect(),
    };

    let max_batch_size: usize = std::env::var("FLAPJACK_MAX_BATCH_SIZE")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(10_000);
    if operations.len() > max_batch_size {
        return Err(FlapjackError::BatchTooLarge {
            size: operations.len(),
            max: max_batch_size,
        });
    }

    for op in operations {
        tracing::info!("Batch operation: action={}", op.action);
        match op.action.as_str() {
            "deleteObject" => {
                let object_id = op
                    .body
                    .get("objectID")
                    .or_else(|| op.body.get("id"))
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        FlapjackError::InvalidQuery("Missing objectID in deleteObject".to_string())
                    })?
                    .to_string();

                object_ids.push(object_id.clone());
                deletes.push(object_id);
                explicit_delete_count += 1;
            }
            "partialUpdateObject" | "partialUpdateObjectNoCreate" => {
                let object_id = op
                    .body
                    .get("objectID")
                    .or_else(|| op.body.get("id"))
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        FlapjackError::InvalidQuery(
                            "Missing objectID in partialUpdateObject".to_string(),
                        )
                    })?
                    .to_string();

                object_ids.push(object_id.clone());

                let create_if_not_exists = if op.action == "partialUpdateObjectNoCreate" {
                    false
                } else {
                    op.create_if_not_exists.unwrap_or(true)
                };

                let existing = state.manager.get_document(&index_name, &object_id)?;
                if existing.is_some() {
                    deletes.push(object_id.clone());
                }

                let body_map: serde_json::Map<String, serde_json::Value> =
                    op.body.into_iter().collect();
                if let Some(doc) =
                    merge_partial_update(existing, &object_id, &body_map, create_if_not_exists)?
                {
                    documents.push(doc);
                }
            }
            "updateObject" => {
                let object_id = op
                    .body
                    .get("objectID")
                    .or_else(|| op.body.get("id"))
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        FlapjackError::InvalidQuery("Missing objectID in updateObject".to_string())
                    })?
                    .to_string();

                object_ids.push(object_id.clone());

                let mut doc_map = op.body;
                doc_map.remove("objectID");
                doc_map.remove("id");

                let mut json_obj = serde_json::Map::new();
                json_obj.insert(
                    "_id".to_string(),
                    serde_json::Value::String(object_id.clone()),
                );
                for (k, v) in doc_map {
                    json_obj.insert(k, v);
                }

                let document = Document::from_json(&serde_json::Value::Object(json_obj))?;
                documents.push(document);
            }
            "addObject" => {
                let mut doc_map = op.body;
                let id = doc_map
                    .remove("objectID")
                    .or_else(|| doc_map.remove("id"))
                    .and_then(|v| v.as_str().map(String::from))
                    .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

                object_ids.push(id.clone());

                let mut json_obj = serde_json::Map::new();
                json_obj.insert("_id".to_string(), serde_json::Value::String(id.clone()));
                for (k, v) in doc_map {
                    json_obj.insert(k, v);
                }

                let document = Document::from_json(&serde_json::Value::Object(json_obj))?;
                documents.push(document);
            }
            _ => {
                return Err(FlapjackError::InvalidQuery(format!(
                    "Unsupported batch action: {}",
                    op.action
                )));
            }
        }
    }

    // Capture oplog seq before write so we can replicate only the new ops.
    let pre_seq = state
        .manager
        .get_oplog(&index_name)
        .map(|ol| ol.current_seq())
        .unwrap_or(0);

    let task = if documents.is_empty() && !deletes.is_empty() {
        state
            .manager
            .delete_documents_sync(&index_name, deletes)
            .await?;
        // Deletes committed synchronously — replicate immediately.
        trigger_replication(&state, &index_name, pre_seq, false);
        // Increment delete counter for pure-delete batches
        if explicit_delete_count > 0 {
            let entry = state
                .usage_counters
                .entry(index_name.clone())
                .or_insert_with(crate::usage_middleware::TenantUsageCounters::new);
            entry
                .documents_deleted_total
                .fetch_add(explicit_delete_count, std::sync::atomic::Ordering::Relaxed);
        }
        let noop = state.manager.make_noop_task(&index_name)?;
        return Ok(Json(AddDocumentsResponse::Algolia {
            task_id: noop.numeric_id,
            object_ids,
        }));
    } else if !deletes.is_empty() {
        // Batch has explicit deletes (e.g. partialUpdateObject) — delete first, then add
        state
            .manager
            .delete_documents_sync(&index_name, deletes)
            .await?;
        let t = state.manager.add_documents(&index_name, documents)?;
        // Adds are async — wait for write queue flush before reading oplog.
        trigger_replication(&state, &index_name, pre_seq, true);
        t
    } else {
        // addObject/updateObject — always upsert (Algolia replaces if objectID exists)
        let t = state.manager.add_documents(&index_name, documents)?;
        trigger_replication(&state, &index_name, pre_seq, true);
        t
    };

    // Increment usage counters
    {
        let entry = state
            .usage_counters
            .entry(index_name.clone())
            .or_insert_with(crate::usage_middleware::TenantUsageCounters::new);
        let doc_count = object_ids.len() as u64 - explicit_delete_count;
        if doc_count > 0 {
            entry
                .documents_indexed_total
                .fetch_add(doc_count, std::sync::atomic::Ordering::Relaxed);
        }
        if explicit_delete_count > 0 {
            entry
                .documents_deleted_total
                .fetch_add(explicit_delete_count, std::sync::atomic::Ordering::Relaxed);
        }
    }

    Ok(Json(AddDocumentsResponse::Algolia {
        task_id: task.numeric_id,
        object_ids,
    }))
}

/// Spawn a background task to replicate newly committed ops to peers.
///
/// `needs_delay`: if true, waits 300ms for the write queue to flush before
/// reading the oplog. Set false when writes are already committed (sync path).
fn trigger_replication(state: &Arc<AppState>, index_name: &str, pre_seq: u64, needs_delay: bool) {
    let repl_mgr = match &state.replication_manager {
        Some(r) => Arc::clone(r),
        None => return,
    };
    let mgr = Arc::clone(&state.manager);
    let tenant = index_name.to_string();

    tokio::spawn(async move {
        if needs_delay {
            // Write queue flushes every ~100ms; 300ms gives a comfortable margin.
            tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
        }
        if let Some(oplog) = mgr.get_oplog(&tenant) {
            match oplog.read_since(pre_seq) {
                Ok(ops) if !ops.is_empty() => {
                    repl_mgr.replicate_ops(&tenant, ops).await;
                }
                Ok(_) => {} // Nothing new (empty write or timing miss — catch-up handles it)
                Err(e) => tracing::warn!("[REPL] failed to read oplog for {}: {}", tenant, e),
            }
        }
    });
}

/// Add or update documents in batch
#[utoipa::path(
    post,
    path = "/1/indexes/{indexName}/batch",
    tag = "documents",
    params(
        ("indexName" = String, Path, description = "Index name")
    ),
    request_body(content = serde_json::Value, description = "Batch operations or single document"),
    responses(
        (status = 200, description = "Documents added successfully", body = AddDocumentsResponse),
        (status = 400, description = "Invalid request")
    ),
    security(
        ("api_key" = [])
    )
)]
pub async fn add_documents(
    State(state): State<Arc<AppState>>,
    Path(index_name): Path<String>,
    Json(req): Json<serde_json::Value>,
) -> Result<Json<AddDocumentsResponse>, FlapjackError> {
    check_not_paused(&state.paused_indexes, &index_name)?;
    if let Ok(batch_req) = serde_json::from_value::<AddDocumentsRequest>(req.clone()) {
        return add_documents_batch_impl(State(state), index_name, batch_req).await;
    }

    let mut doc_map = req
        .as_object()
        .ok_or_else(|| FlapjackError::InvalidQuery("Expected object".to_string()))?
        .clone();

    let id = doc_map
        .remove("objectID")
        .or_else(|| doc_map.remove("id"))
        .and_then(|v| v.as_str().map(String::from))
        .ok_or_else(|| FlapjackError::InvalidQuery("Missing objectID or id field".to_string()))?;

    let fields = doc_map
        .into_iter()
        .filter_map(|(key, value)| {
            let field_value = match value {
                serde_json::Value::String(s) => Some(FieldValue::Text(s)),
                serde_json::Value::Number(n) => n
                    .as_i64()
                    .map(FieldValue::Integer)
                    .or_else(|| n.as_f64().map(FieldValue::Float)),
                serde_json::Value::Array(arr) => {
                    if arr.len() == 1 {
                        arr[0].as_str().map(|s| FieldValue::Facet(s.to_string()))
                    } else {
                        None
                    }
                }
                _ => None,
            };
            field_value.map(|v| (key, v))
        })
        .collect();

    let document = Document {
        id: id.clone(),
        fields,
    };
    let task = state.manager.add_documents(&index_name, vec![document])?;

    Ok(Json(AddDocumentsResponse::Algolia {
        task_id: task.numeric_id,
        object_ids: vec![id],
    }))
}

/// Get a single object by ID
#[utoipa::path(
    get,
    path = "/1/indexes/{indexName}/{objectID}",
    tag = "documents",
    params(
        ("indexName" = String, Path, description = "Index name"),
        ("objectID" = String, Path, description = "Object ID to retrieve")
    ),
    responses(
        (status = 200, description = "Object retrieved successfully", body = serde_json::Value),
        (status = 404, description = "Object not found")
    ),
    security(
        ("api_key" = [])
    )
)]
pub async fn get_object(
    State(state): State<Arc<AppState>>,
    Path((index_name, object_id)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let doc = state
        .manager
        .get_document(&index_name, &object_id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    match doc {
        None => Err((
            StatusCode::NOT_FOUND,
            format!("Object {} not found", object_id),
        )),
        Some(document) => {
            let mut obj = serde_json::Map::new();
            obj.insert(
                "objectID".to_string(),
                serde_json::Value::String(document.id),
            );

            for (key, value) in document.fields {
                obj.insert(key, field_value_to_json(&value));
            }

            Ok(Json(serde_json::Value::Object(obj)))
        }
    }
}

/// Delete a single object by ID
#[utoipa::path(
    delete,
    path = "/1/indexes/{indexName}/{objectID}",
    tag = "documents",
    params(
        ("indexName" = String, Path, description = "Index name"),
        ("objectID" = String, Path, description = "Object ID to delete")
    ),
    responses(
        (status = 200, description = "Object deleted successfully", body = serde_json::Value),
        (status = 404, description = "Object not found")
    ),
    security(
        ("api_key" = [])
    )
)]
pub async fn delete_object(
    State(state): State<Arc<AppState>>,
    Path((index_name, object_id)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, FlapjackError> {
    check_not_paused(&state.paused_indexes, &index_name)?;
    let pre_seq = state
        .manager
        .get_oplog(&index_name)
        .map(|ol| ol.current_seq())
        .unwrap_or(0);
    state
        .manager
        .delete_documents_sync(&index_name, vec![object_id])
        .await?;
    trigger_replication(&state, &index_name, pre_seq, false);

    // Increment usage counter: 1 document deleted
    state
        .usage_counters
        .entry(index_name.clone())
        .or_insert_with(crate::usage_middleware::TenantUsageCounters::new)
        .documents_deleted_total
        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

    let task = state.manager.make_noop_task(&index_name)?;
    Ok(Json(serde_json::json!({
        "taskID": task.numeric_id,
        "deletedAt": chrono::Utc::now().to_rfc3339()
    })))
}

/// Update or create an object
#[utoipa::path(
    put,
    path = "/1/indexes/{indexName}/{objectID}",
    tag = "documents",
    params(
        ("indexName" = String, Path, description = "Index name"),
        ("objectID" = String, Path, description = "Object ID to update or create")
    ),
    request_body(content = serde_json::Value, description = "Object data"),
    responses(
        (status = 200, description = "Object updated successfully", body = serde_json::Value),
        (status = 400, description = "Invalid request")
    ),
    security(
        ("api_key" = [])
    )
)]
pub async fn put_object(
    State(state): State<Arc<AppState>>,
    Path((index_name, object_id)): Path<(String, String)>,
    Json(mut body): Json<serde_json::Map<String, serde_json::Value>>,
) -> Result<Json<serde_json::Value>, FlapjackError> {
    check_not_paused(&state.paused_indexes, &index_name)?;
    state.manager.create_tenant(&index_name)?;

    body.remove("objectID");
    body.remove("id");

    let mut json_obj = serde_json::Map::new();
    json_obj.insert(
        "_id".to_string(),
        serde_json::Value::String(object_id.clone()),
    );
    for (k, v) in body {
        json_obj.insert(k, v);
    }

    let document = Document::from_json(&serde_json::Value::Object(json_obj))?;

    let pre_seq = state
        .manager
        .get_oplog(&index_name)
        .map(|ol| ol.current_seq())
        .unwrap_or(0);
    state
        .manager
        .delete_documents_sync(&index_name, vec![object_id.clone()])
        .await?;
    state
        .manager
        .add_documents_sync(&index_name, vec![document])
        .await?;
    trigger_replication(&state, &index_name, pre_seq, false);

    // Increment usage counter: 1 document indexed (put = upsert)
    state
        .usage_counters
        .entry(index_name.clone())
        .or_insert_with(crate::usage_middleware::TenantUsageCounters::new)
        .documents_indexed_total
        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

    let task = state.manager.make_noop_task(&index_name)?;
    Ok(Json(serde_json::json!({
        "taskID": task.numeric_id,
        "objectID": object_id,
        "updatedAt": chrono::Utc::now().to_rfc3339()
    })))
}

/// Get multiple objects by ID in batch
#[utoipa::path(
    post,
    path = "/1/indexes/{indexName}/objects",
    tag = "documents",
    params(
        ("indexName" = String, Path, description = "Index name")
    ),
    request_body = GetObjectsRequest,
    responses(
        (status = 200, description = "Objects retrieved successfully", body = GetObjectsResponse)
    ),
    security(
        ("api_key" = [])
    )
)]
pub async fn get_objects(
    State(state): State<Arc<AppState>>,
    Json(req): Json<GetObjectsRequest>,
) -> Result<Json<GetObjectsResponse>, FlapjackError> {
    let mut results = Vec::new();

    for request in req.requests {
        match state
            .manager
            .get_document(&request.index_name, &request.object_id)
        {
            Ok(Some(document)) => {
                let mut obj = serde_json::Map::new();
                obj.insert(
                    "objectID".to_string(),
                    serde_json::Value::String(document.id),
                );

                for (key, value) in document.fields {
                    if let Some(attrs) = &request.attributes_to_retrieve {
                        if !attrs.contains(&key) {
                            continue;
                        }
                    }
                    obj.insert(key, field_value_to_json(&value));
                }

                results.push(serde_json::Value::Object(obj));
            }
            Ok(None) => {
                results.push(serde_json::Value::Null);
            }
            Err(_) => {
                results.push(serde_json::Value::Null);
            }
        }
    }

    Ok(Json(GetObjectsResponse { results }))
}

/// Delete objects matching a filter query
#[utoipa::path(
    post,
    path = "/1/indexes/{indexName}/deleteByQuery",
    tag = "documents",
    params(
        ("indexName" = String, Path, description = "Index name")
    ),
    request_body = DeleteByQueryRequest,
    responses(
        (status = 200, description = "Objects deleted successfully", body = serde_json::Value),
        (status = 400, description = "Invalid filter query")
    ),
    security(
        ("api_key" = [])
    )
)]
pub async fn delete_by_query(
    State(state): State<Arc<AppState>>,
    Path(index_name): Path<String>,
    Json(req): Json<DeleteByQueryRequest>,
) -> Result<Json<serde_json::Value>, FlapjackError> {
    check_not_paused(&state.paused_indexes, &index_name)?;
    let filter = if let Some(filter_str) = &req.filters {
        Some(
            parse_filter(filter_str)
                .map_err(|e| FlapjackError::InvalidQuery(format!("Filter parse error: {}", e)))?,
        )
    } else {
        return Err(FlapjackError::InvalidQuery(
            "filters parameter required".to_string(),
        ));
    };

    const BATCH_SIZE: usize = 1000;
    let mut all_ids = Vec::new();
    let mut offset = 0;

    loop {
        let result = state.manager.search_with_facets(
            &index_name,
            "",
            filter.as_ref(),
            None,
            BATCH_SIZE,
            offset,
            None,
        )?;

        if result.documents.is_empty() {
            break;
        }

        for doc in &result.documents {
            all_ids.push(doc.document.id.clone());
        }

        offset += result.documents.len();

        if result.documents.len() < BATCH_SIZE {
            break;
        }

        if offset >= result.total {
            break;
        }
    }

    if all_ids.is_empty() {
        let task = state.manager.make_noop_task(&index_name)?;
        return Ok(Json(serde_json::json!({
            "taskID": task.numeric_id,
            "deletedAt": chrono::Utc::now().to_rfc3339()
        })));
    }

    let deleted_count = all_ids.len() as u64;
    let pre_seq = state
        .manager
        .get_oplog(&index_name)
        .map(|ol| ol.current_seq())
        .unwrap_or(0);
    state
        .manager
        .delete_documents_sync(&index_name, all_ids)
        .await?;
    trigger_replication(&state, &index_name, pre_seq, false);

    // Increment usage counter: N documents deleted by query
    state
        .usage_counters
        .entry(index_name.clone())
        .or_insert_with(crate::usage_middleware::TenantUsageCounters::new)
        .documents_deleted_total
        .fetch_add(deleted_count, std::sync::atomic::Ordering::Relaxed);

    let task = state.manager.make_noop_task(&index_name)?;
    Ok(Json(serde_json::json!({
        "taskID": task.numeric_id,
        "deletedAt": chrono::Utc::now().to_rfc3339()
    })))
}

/// Add a record with an auto-generated objectID (Algolia-compatible)
#[utoipa::path(
    post,
    path = "/1/indexes/{indexName}",
    tag = "documents",
    params(
        ("indexName" = String, Path, description = "Index name")
    ),
    request_body(content = serde_json::Value, description = "Object data (objectID is auto-generated)"),
    responses(
        (status = 200, description = "Object created successfully", body = serde_json::Value)
    ),
    security(
        ("api_key" = [])
    )
)]
pub async fn add_record_auto_id(
    State(state): State<Arc<AppState>>,
    Path(index_name): Path<String>,
    Json(mut body): Json<serde_json::Map<String, serde_json::Value>>,
) -> Result<Json<serde_json::Value>, FlapjackError> {
    check_not_paused(&state.paused_indexes, &index_name)?;
    state.manager.create_tenant(&index_name)?;

    let generated_id = uuid::Uuid::new_v4().to_string();

    body.remove("objectID");
    body.remove("id");

    let mut json_obj = serde_json::Map::new();
    json_obj.insert(
        "_id".to_string(),
        serde_json::Value::String(generated_id.clone()),
    );
    for (k, v) in body {
        json_obj.insert(k, v);
    }

    let document = Document::from_json(&serde_json::Value::Object(json_obj))?;
    let pre_seq = state
        .manager
        .get_oplog(&index_name)
        .map(|ol| ol.current_seq())
        .unwrap_or(0);
    let task = state.manager.add_documents(&index_name, vec![document])?;
    trigger_replication(&state, &index_name, pre_seq, true);

    // Increment usage counter: 1 document indexed (auto-id create)
    state
        .usage_counters
        .entry(index_name.clone())
        .or_insert_with(crate::usage_middleware::TenantUsageCounters::new)
        .documents_indexed_total
        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

    Ok(Json(serde_json::json!({
        "taskID": task.numeric_id,
        "objectID": generated_id,
        "createdAt": chrono::Utc::now().to_rfc3339()
    })))
}

/// Partial update a record (Algolia-compatible dedicated endpoint)
#[utoipa::path(
    post,
    path = "/1/indexes/{indexName}/{objectID}/partial",
    tag = "documents",
    params(
        ("indexName" = String, Path, description = "Index name"),
        ("objectID" = String, Path, description = "Object ID to partially update"),
        ("createIfNotExists" = Option<bool>, Query, description = "Create the record if it doesn't exist (default: true)")
    ),
    request_body(content = serde_json::Value, description = "Fields to update"),
    responses(
        (status = 200, description = "Object partially updated", body = serde_json::Value)
    ),
    security(
        ("api_key" = [])
    )
)]
pub async fn partial_update_object(
    State(state): State<Arc<AppState>>,
    Path((index_name, object_id)): Path<(String, String)>,
    Query(params): Query<PartialUpdateParams>,
    Json(body): Json<serde_json::Map<String, serde_json::Value>>,
) -> Result<Json<serde_json::Value>, FlapjackError> {
    check_not_paused(&state.paused_indexes, &index_name)?;
    state.manager.create_tenant(&index_name)?;

    let create_if_not_exists = params.create_if_not_exists.unwrap_or(true);
    let existing = state.manager.get_document(&index_name, &object_id)?;

    let pre_seq = state
        .manager
        .get_oplog(&index_name)
        .map(|ol| ol.current_seq())
        .unwrap_or(0);

    if existing.is_some() {
        state
            .manager
            .delete_documents_sync(&index_name, vec![object_id.clone()])
            .await?;
    }

    if let Some(doc) = merge_partial_update(existing, &object_id, &body, create_if_not_exists)? {
        state
            .manager
            .add_documents_sync(&index_name, vec![doc])
            .await?;
    }

    trigger_replication(&state, &index_name, pre_seq, false);

    let task = state.manager.make_noop_task(&index_name)?;
    Ok(Json(serde_json::json!({
        "taskID": task.numeric_id,
        "objectID": object_id,
        "updatedAt": chrono::Utc::now().to_rfc3339()
    })))
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PartialUpdateParams {
    pub create_if_not_exists: Option<bool>,
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── is_operation ──

    #[test]
    fn is_operation_true() {
        let v = serde_json::json!({"_operation": "Increment", "value": 1});
        assert!(is_operation(&v));
    }

    #[test]
    fn is_operation_false_no_key() {
        let v = serde_json::json!({"name": "Alice"});
        assert!(!is_operation(&v));
    }

    #[test]
    fn is_operation_false_not_object() {
        assert!(!is_operation(&serde_json::json!("hello")));
        assert!(!is_operation(&serde_json::json!(42)));
        assert!(!is_operation(&serde_json::json!(null)));
    }

    // ── apply_operation: Increment ──

    #[test]
    fn increment_integer() {
        let existing = Some(FieldValue::Integer(10));
        let result = apply_operation(existing.as_ref(), "Increment", &serde_json::json!(5));
        assert_eq!(result, Some(FieldValue::Integer(15)));
    }

    #[test]
    fn increment_float() {
        let existing = Some(FieldValue::Float(1.5));
        let result = apply_operation(existing.as_ref(), "Increment", &serde_json::json!(0.5));
        assert_eq!(result, Some(FieldValue::Float(2.0)));
    }

    #[test]
    fn increment_missing_field_integer() {
        let result = apply_operation(None, "Increment", &serde_json::json!(7));
        assert_eq!(result, Some(FieldValue::Integer(7)));
    }

    #[test]
    fn increment_missing_field_float() {
        let result = apply_operation(None, "Increment", &serde_json::json!(2.5));
        assert_eq!(result, Some(FieldValue::Float(2.5)));
    }

    #[test]
    fn increment_from_alias() {
        let existing = Some(FieldValue::Integer(3));
        let result = apply_operation(existing.as_ref(), "IncrementFrom", &serde_json::json!(10));
        assert_eq!(result, Some(FieldValue::Integer(13)));
    }

    // ── apply_operation: Decrement ──

    #[test]
    fn decrement_integer() {
        let existing = Some(FieldValue::Integer(10));
        let result = apply_operation(existing.as_ref(), "Decrement", &serde_json::json!(3));
        assert_eq!(result, Some(FieldValue::Integer(7)));
    }

    #[test]
    fn decrement_float() {
        let existing = Some(FieldValue::Float(5.0));
        let result = apply_operation(existing.as_ref(), "Decrement", &serde_json::json!(1.5));
        assert_eq!(result, Some(FieldValue::Float(3.5)));
    }

    #[test]
    fn decrement_missing_field() {
        let result = apply_operation(None, "Decrement", &serde_json::json!(4));
        assert_eq!(result, Some(FieldValue::Integer(-4)));
    }

    // ── apply_operation: Add ──

    #[test]
    fn add_to_existing_array() {
        let existing = Some(FieldValue::Array(vec![FieldValue::Text("a".into())]));
        let result = apply_operation(existing.as_ref(), "Add", &serde_json::json!("b"));
        let arr = match result {
            Some(FieldValue::Array(arr)) => arr,
            _ => panic!("expected array"),
        };
        assert_eq!(
            arr,
            vec![FieldValue::Text("a".into()), FieldValue::Text("b".into())]
        );
    }

    #[test]
    fn add_to_none_creates_array() {
        let result = apply_operation(None, "Add", &serde_json::json!("x"));
        match result {
            Some(FieldValue::Array(arr)) => {
                assert_eq!(arr, vec![FieldValue::Text("x".into())]);
            }
            _ => panic!("expected array"),
        }
    }

    #[test]
    fn add_to_non_array_wraps() {
        let existing = Some(FieldValue::Text("old".into()));
        let result = apply_operation(existing.as_ref(), "Add", &serde_json::json!("new"));
        let arr = match result {
            Some(FieldValue::Array(arr)) => arr,
            _ => panic!("expected array"),
        };
        assert_eq!(
            arr,
            vec![
                FieldValue::Text("old".into()),
                FieldValue::Text("new".into())
            ]
        );
    }

    // ── apply_operation: Remove ──

    #[test]
    fn remove_from_array() {
        let existing = Some(FieldValue::Array(vec![
            FieldValue::Text("a".into()),
            FieldValue::Text("b".into()),
            FieldValue::Text("c".into()),
        ]));
        let result = apply_operation(existing.as_ref(), "Remove", &serde_json::json!("b"));
        match result {
            Some(FieldValue::Array(arr)) => {
                assert_eq!(arr.len(), 2);
                assert_eq!(arr[0], FieldValue::Text("a".into()));
                assert_eq!(arr[1], FieldValue::Text("c".into()));
            }
            _ => panic!("expected array"),
        }
    }

    #[test]
    fn remove_nonexistent_item() {
        let existing = Some(FieldValue::Array(vec![FieldValue::Text("a".into())]));
        let result = apply_operation(existing.as_ref(), "Remove", &serde_json::json!("z"));
        match result {
            Some(FieldValue::Array(arr)) => {
                assert_eq!(arr, vec![FieldValue::Text("a".into())]);
            }
            _ => panic!("expected array"),
        }
    }

    #[test]
    fn remove_from_non_array_returns_existing() {
        let existing = Some(FieldValue::Text("hello".into()));
        let result = apply_operation(existing.as_ref(), "Remove", &serde_json::json!("x"));
        assert_eq!(result, Some(FieldValue::Text("hello".into())));
    }

    // ── apply_operation: AddUnique ──

    #[test]
    fn add_unique_new_item() {
        let existing = Some(FieldValue::Array(vec![FieldValue::Text("a".into())]));
        let result = apply_operation(existing.as_ref(), "AddUnique", &serde_json::json!("b"));
        match result {
            Some(FieldValue::Array(arr)) => {
                assert_eq!(
                    arr,
                    vec![FieldValue::Text("a".into()), FieldValue::Text("b".into())]
                );
            }
            _ => panic!("expected array"),
        }
    }

    #[test]
    fn add_unique_duplicate_item() {
        let existing = Some(FieldValue::Array(vec![FieldValue::Text("a".into())]));
        let result = apply_operation(existing.as_ref(), "AddUnique", &serde_json::json!("a"));
        match result {
            Some(FieldValue::Array(arr)) => {
                assert_eq!(arr, vec![FieldValue::Text("a".into())]);
            }
            _ => panic!("expected array"),
        }
    }

    #[test]
    fn add_unique_to_none() {
        let result = apply_operation(None, "AddUnique", &serde_json::json!("x"));
        match result {
            Some(FieldValue::Array(arr)) => {
                assert_eq!(arr, vec![FieldValue::Text("x".into())]);
            }
            _ => panic!("expected array"),
        }
    }

    // ── apply_operation: edge cases ──

    #[test]
    fn unknown_operation_returns_none() {
        let result = apply_operation(None, "FooBar", &serde_json::json!(1));
        assert_eq!(result, None);
    }

    #[test]
    fn increment_negative_delta() {
        let existing = Some(FieldValue::Integer(10));
        let result = apply_operation(existing.as_ref(), "Increment", &serde_json::json!(-3));
        assert_eq!(result, Some(FieldValue::Integer(7)));
    }

    #[test]
    fn decrement_from_alias() {
        let existing = Some(FieldValue::Integer(10));
        let result = apply_operation(existing.as_ref(), "DecrementFrom", &serde_json::json!(4));
        assert_eq!(result, Some(FieldValue::Integer(6)));
    }

    #[test]
    fn increment_set_alias() {
        let existing = Some(FieldValue::Integer(5));
        let result = apply_operation(existing.as_ref(), "IncrementSet", &serde_json::json!(2));
        assert_eq!(result, Some(FieldValue::Integer(7)));
    }

    #[test]
    fn remove_from_empty_array() {
        let existing = Some(FieldValue::Array(vec![]));
        let result = apply_operation(existing.as_ref(), "Remove", &serde_json::json!("a"));
        match result {
            Some(FieldValue::Array(arr)) => assert!(arr.is_empty()),
            _ => panic!("expected empty array"),
        }
    }

    #[test]
    fn add_unique_integer_dedup() {
        let existing = Some(FieldValue::Array(vec![FieldValue::Integer(42)]));
        let result = apply_operation(existing.as_ref(), "AddUnique", &serde_json::json!(42));
        match result {
            Some(FieldValue::Array(arr)) => {
                assert_eq!(arr, vec![FieldValue::Integer(42)]);
            }
            _ => panic!("expected array"),
        }
    }

    // ── Write handler guard (pause) tests ──────────────────────────────

    use axum::body::Body;
    use axum::http::Request;
    use axum::routing::{delete, post, put};
    use axum::Router;
    use tempfile::TempDir;
    use tower::ServiceExt;

    fn make_write_guard_state(tmp: &TempDir) -> Arc<AppState> {
        Arc::new(AppState {
            manager: flapjack::IndexManager::new(tmp.path()),
            key_store: None,
            replication_manager: None,
            ssl_manager: None,
            analytics_engine: None,
            experiment_store: None,
            metrics_state: None,
            usage_counters: Arc::new(dashmap::DashMap::new()),
            paused_indexes: crate::pause_registry::PausedIndexes::new(),
            start_time: std::time::Instant::now(),
            #[cfg(feature = "vector-search")]
            embedder_store: Arc::new(crate::embedder_store::EmbedderStore::new()),
        })
    }

    fn make_write_guard_app(state: Arc<AppState>) -> Router {
        Router::new()
            .route("/1/indexes/:indexName/batch", post(super::add_documents))
            .route("/1/indexes/:indexName/:objectID", put(super::put_object))
            .route(
                "/1/indexes/:indexName/:objectID",
                delete(super::delete_object),
            )
            .route(
                "/1/indexes/:indexName/:objectID/partial",
                post(super::partial_update_object),
            )
            .route(
                "/1/indexes/:indexName/deleteByQuery",
                post(super::delete_by_query),
            )
            .route("/1/indexes/:indexName", post(super::add_record_auto_id))
            .with_state(state)
    }

    #[tokio::test]
    async fn test_add_documents_blocked_when_paused() {
        let tmp = TempDir::new().unwrap();
        let state = make_write_guard_state(&tmp);
        state.paused_indexes.pause("test_index");
        let app = make_write_guard_app(state);

        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/1/indexes/test_index/batch")
                    .header("Content-Type", "application/json")
                    .body(Body::from("{}"))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["error"], "index_paused");
    }

    #[tokio::test]
    async fn test_put_object_blocked_when_paused() {
        let tmp = TempDir::new().unwrap();
        let state = make_write_guard_state(&tmp);
        state.paused_indexes.pause("test_index");
        let app = make_write_guard_app(state);

        let resp = app
            .oneshot(
                Request::builder()
                    .method("PUT")
                    .uri("/1/indexes/test_index/obj1")
                    .header("Content-Type", "application/json")
                    .body(Body::from("{}"))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["error"], "index_paused");
    }

    #[tokio::test]
    async fn test_delete_object_blocked_when_paused() {
        let tmp = TempDir::new().unwrap();
        let state = make_write_guard_state(&tmp);
        state.paused_indexes.pause("test_index");
        let app = make_write_guard_app(state);

        let resp = app
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri("/1/indexes/test_index/obj1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["error"], "index_paused");
    }

    #[tokio::test]
    async fn test_partial_update_blocked_when_paused() {
        let tmp = TempDir::new().unwrap();
        let state = make_write_guard_state(&tmp);
        state.paused_indexes.pause("test_index");
        let app = make_write_guard_app(state);

        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/1/indexes/test_index/obj1/partial")
                    .header("Content-Type", "application/json")
                    .body(Body::from("{}"))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["error"], "index_paused");
    }

    #[tokio::test]
    async fn test_delete_by_query_blocked_when_paused() {
        let tmp = TempDir::new().unwrap();
        let state = make_write_guard_state(&tmp);
        state.paused_indexes.pause("test_index");
        let app = make_write_guard_app(state);

        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/1/indexes/test_index/deleteByQuery")
                    .header("Content-Type", "application/json")
                    .body(Body::from("{}"))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["error"], "index_paused");
    }

    #[tokio::test]
    async fn test_add_record_auto_id_blocked_when_paused() {
        let tmp = TempDir::new().unwrap();
        let state = make_write_guard_state(&tmp);
        state.paused_indexes.pause("test_index");
        let app = make_write_guard_app(state);

        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/1/indexes/test_index")
                    .header("Content-Type", "application/json")
                    .body(Body::from("{}"))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["error"], "index_paused");
    }

    // ── Reads-unaffected tests (2G) ─────────────────────────────────────

    fn make_read_write_app(state: Arc<AppState>) -> Router {
        Router::new()
            .route(
                "/1/indexes/:indexName/query",
                post(crate::handlers::search::search),
            )
            .route(
                "/1/indexes/:indexName/:objectID",
                axum::routing::get(super::get_object),
            )
            .route("/1/indexes/:indexName/batch", post(super::add_documents))
            .with_state(state)
    }

    #[tokio::test]
    async fn test_search_allowed_when_paused() {
        let tmp = TempDir::new().unwrap();
        let state = make_write_guard_state(&tmp);
        state.paused_indexes.pause("test_index");
        let app = make_read_write_app(state);

        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/1/indexes/test_index/query")
                    .header("Content-Type", "application/json")
                    .body(Body::from(r#"{"query":""}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        // Should be anything but 503 — likely 404 (TenantNotFound) since no index exists
        assert_ne!(
            resp.status(),
            StatusCode::SERVICE_UNAVAILABLE,
            "search should NOT be blocked when index is paused; got 503"
        );
    }

    #[tokio::test]
    async fn test_get_object_allowed_when_paused() {
        let tmp = TempDir::new().unwrap();
        let state = make_write_guard_state(&tmp);
        state.paused_indexes.pause("test_index");
        let app = make_read_write_app(state);

        let resp = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/1/indexes/test_index/obj1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        // Should be anything but 503 — likely 500 or 404
        assert_ne!(
            resp.status(),
            StatusCode::SERVICE_UNAVAILABLE,
            "get_object should NOT be blocked when index is paused; got 503"
        );
    }

    // ── Cross-index isolation test (2H) ─────────────────────────────────

    #[tokio::test]
    async fn test_pause_does_not_affect_other_indexes() {
        let tmp = TempDir::new().unwrap();
        let state = make_write_guard_state(&tmp);
        // Pause "foo" but write to "bar"
        state.paused_indexes.pause("foo");
        let app = make_write_guard_app(state);

        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/1/indexes/bar/batch")
                    .header("Content-Type", "application/json")
                    .body(Body::from("{}"))
                    .unwrap(),
            )
            .await
            .unwrap();

        // "bar" is not paused — should NOT be 503
        assert_ne!(
            resp.status(),
            StatusCode::SERVICE_UNAVAILABLE,
            "writes to 'bar' should NOT be blocked when only 'foo' is paused; got 503"
        );
    }
}
