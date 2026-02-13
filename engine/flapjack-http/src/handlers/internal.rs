use crate::handlers::AppState;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use flapjack::types::Document;
use flapjack_replication::types::{
    GetOpsQuery, GetOpsResponse, ReplicateOpsRequest, ReplicateOpsResponse,
};
use std::sync::Arc;

/// POST /internal/replicate
/// Receive operations from a peer and apply them to local index
/// Phase 4: Simple approach - reconstruct operations and submit through IndexManager
pub async fn replicate_ops(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ReplicateOpsRequest>,
) -> impl IntoResponse {
    let tenant_id = req.tenant_id.clone();

    // Create tenant if it doesn't exist
    if let Err(e) = state.manager.create_tenant(&tenant_id) {
        tracing::warn!("[REPL {}] failed to create tenant: {}", tenant_id, e);
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "error": format!("Failed to create tenant: {}", e)
            })),
        )
            .into_response();
    }

    // Process operations
    let mut max_seq = 0u64;
    let mut upserts = Vec::new();
    let mut deletes = Vec::new();

    for op_entry in &req.ops {
        max_seq = max_seq.max(op_entry.seq);

        match op_entry.op_type.as_str() {
            "upsert" => {
                // Extract document from payload
                if let Some(body) = op_entry.payload.get("body") {
                    match Document::from_json(body) {
                        Ok(doc) => upserts.push(doc),
                        Err(e) => {
                            tracing::warn!(
                                "[REPL {}] failed to parse upsert payload seq {}: {}",
                                tenant_id,
                                op_entry.seq,
                                e
                            );
                        }
                    }
                }
            }
            "delete" => {
                // Extract objectID from payload
                if let Some(object_id) = op_entry.payload.get("objectID").and_then(|v| v.as_str()) {
                    deletes.push(object_id.to_string());
                }
            }
            _ => {
                tracing::warn!(
                    "[REPL {}] unknown op_type {} at seq {}",
                    tenant_id,
                    op_entry.op_type,
                    op_entry.seq
                );
            }
        }
    }

    // Apply upserts
    if !upserts.is_empty() {
        if let Err(e) = state.manager.add_documents(&tenant_id, upserts) {
            tracing::error!("[REPL {}] failed to apply upserts: {}", tenant_id, e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": format!("Failed to apply upserts: {}", e)
                })),
            )
                .into_response();
        }
    }

    // Apply deletes
    if !deletes.is_empty() {
        if let Err(e) = state
            .manager
            .delete_documents_sync(&tenant_id, deletes)
            .await
        {
            tracing::error!("[REPL {}] failed to apply deletes: {}", tenant_id, e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": format!("Failed to apply deletes: {}", e)
                })),
            )
                .into_response();
        }
    }

    tracing::info!(
        "[REPL {}] applied {} ops (max_seq={})",
        tenant_id,
        req.ops.len(),
        max_seq
    );

    // Return acked sequence
    let response = ReplicateOpsResponse {
        tenant_id,
        acked_seq: max_seq,
    };

    (StatusCode::OK, Json(response)).into_response()
}

/// GET /internal/ops?tenant_id=X&since_seq=N
/// Fetch operations since a given sequence number for catch-up
pub async fn get_ops(
    State(state): State<Arc<AppState>>,
    Query(query): Query<GetOpsQuery>,
) -> impl IntoResponse {
    let tenant_id = query.tenant_id.clone();

    // Get oplog for tenant
    let oplog = match state.manager.get_oplog(&tenant_id) {
        Some(ol) => ol,
        None => {
            tracing::warn!("[REPL {}] oplog not found", tenant_id);
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({
                    "error": "Tenant not found"
                })),
            )
                .into_response();
        }
    };

    // Read ops since requested sequence
    let ops = match oplog.read_since(query.since_seq) {
        Ok(ops) => ops,
        Err(e) => {
            tracing::error!("[REPL {}] failed to read oplog: {}", tenant_id, e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": format!("Failed to read oplog: {}", e)
                })),
            )
                .into_response();
        }
    };

    let current_seq = oplog.current_seq();

    tracing::info!(
        "[REPL {}] serving {} ops (since_seq={}, current_seq={})",
        tenant_id,
        ops.len(),
        query.since_seq,
        current_seq
    );

    let response = GetOpsResponse {
        tenant_id,
        ops,
        current_seq,
    };

    (StatusCode::OK, Json(response)).into_response()
}

/// GET /internal/status
/// Return basic replication status for monitoring
pub async fn replication_status(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let (node_id, replication_enabled, peer_count) = match &state.replication_manager {
        Some(repl_mgr) => (repl_mgr.node_id().to_string(), true, repl_mgr.peer_count()),
        None => (
            std::env::var("FLAPJACK_NODE_ID").unwrap_or_else(|_| "unknown".to_string()),
            false,
            0,
        ),
    };

    // Get SSL renewal status if available
    let ssl_renewal = if let Some(ref ssl_mgr) = state.ssl_manager {
        Some(ssl_mgr.get_status().await)
    } else {
        None
    };

    let response = serde_json::json!({
        "node_id": node_id,
        "replication_enabled": replication_enabled,
        "peer_count": peer_count,
        "ssl_renewal": ssl_renewal,
    });

    (StatusCode::OK, Json(response)).into_response()
}

/// GET /.well-known/acme-challenge/:token
/// ACME http-01 challenge handler for Let's Encrypt validation
pub async fn acme_challenge(
    Path(token): Path<String>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    tracing::debug!("[SSL] ACME challenge request for token: {}", token);

    if let Some(ref ssl_mgr) = state.ssl_manager {
        if let Some(acme_client) = ssl_mgr.get_acme_client() {
            if let Some(response) = acme_client.get_challenge_response(&token) {
                tracing::info!("[SSL] Serving ACME challenge response for token: {}", token);
                return (StatusCode::OK, response).into_response();
            }
        }
    }

    tracing::warn!("[SSL] ACME challenge token not found: {}", token);
    (StatusCode::NOT_FOUND, "Challenge not found").into_response()
}
