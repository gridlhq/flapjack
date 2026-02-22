use crate::handlers::AppState;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use flapjack::index::oplog::OpLogEntry;
use flapjack::types::Document;
use flapjack::IndexManager;
use flapjack_replication::types::{
    GetOpsQuery, GetOpsResponse, ReplicateOpsRequest, ReplicateOpsResponse,
};
use std::sync::Arc;

/// Core apply logic: parse ops and write to IndexManager.
/// Returns the highest sequence number applied, or an error string.
/// Extracted for reuse by the HTTP handler and startup catch-up.
///
/// Implements LWW (last-writer-wins) conflict resolution:
/// - For upserts: (timestamp_ms, node_id) tuples are compared; higher wins.
/// - For deletes: only applied if no newer upsert has been recorded for the doc.
/// - LWW state is tracked in-memory in IndexManager::lww_map.
pub async fn apply_ops_to_manager(
    manager: &IndexManager,
    tenant_id: &str,
    ops: &[OpLogEntry],
) -> Result<u64, String> {
    // P3: Use get_or_load instead of create_tenant so that recover_from_oplog fires on
    // first access, rebuilding lww_map from the oplog before the LWW check below.
    // If the tenant does not exist yet, fall back to create_tenant (new index, no history).
    if manager.get_or_load(tenant_id).is_err() {
        let _ = manager.create_tenant(tenant_id);
    }

    let mut max_seq = 0u64;
    let mut upserts = Vec::new();
    let mut deletes = Vec::new();
    // Track the final winning op type for each doc ID so we can resolve
    // conflicts when the same doc has both upserts and deletes in one batch.
    let mut final_op_type: std::collections::HashMap<String, &str> =
        std::collections::HashMap::new();

    for op_entry in ops {
        max_seq = max_seq.max(op_entry.seq);
        let incoming = (op_entry.timestamp_ms, op_entry.node_id.clone());

        match op_entry.op_type.as_str() {
            "upsert" => {
                if let Some(body) = op_entry.payload.get("body") {
                    // LWW check: skip if an op with a higher (ts, node_id) was already applied.
                    let object_id = body
                        .get("_id")
                        .and_then(|v| v.as_str())
                        .or_else(|| body.get("objectID").and_then(|v| v.as_str()))
                        .unwrap_or("");
                    if !object_id.is_empty() {
                        if let Some(existing) = manager.get_lww(tenant_id, object_id) {
                            // existing wins if its (ts, node_id) >= incoming (ts, node_id)
                            if existing >= incoming {
                                tracing::debug!(
                                    "[REPL LWW] skipping stale upsert for {}/{} (existing={:?} >= incoming={:?})",
                                    tenant_id, object_id, existing, incoming
                                );
                                continue;
                            }
                        }
                    }
                    match Document::from_json(body) {
                        Ok(doc) => {
                            // Record LWW state before queuing so subsequent ops in this
                            // batch see the updated state.  Use add_documents_for_replication
                            // below so write_queue does NOT overwrite this with system time.
                            if !object_id.is_empty() {
                                manager.record_lww(tenant_id, object_id, incoming.0, incoming.1);
                                final_op_type.insert(object_id.to_string(), "upsert");
                            }
                            upserts.push(doc);
                        }
                        Err(e) => tracing::warn!(
                            "[REPL {}] failed to parse upsert seq {}: {}",
                            tenant_id,
                            op_entry.seq,
                            e
                        ),
                    }
                }
            }
            "delete" => {
                if let Some(id) = op_entry.payload.get("objectID").and_then(|v| v.as_str()) {
                    // LWW check: skip delete if a newer upsert was already applied.
                    if let Some(existing) = manager.get_lww(tenant_id, id) {
                        if existing > incoming {
                            tracing::debug!(
                                "[REPL LWW] skipping stale delete for {}/{} (existing={:?} > incoming={:?})",
                                tenant_id, id, existing, incoming
                            );
                            continue;
                        }
                    }
                    // Record the delete in LWW map so future upserts with older ts are rejected.
                    manager.record_lww(tenant_id, id, incoming.0, incoming.1.clone());
                    final_op_type.insert(id.to_string(), "delete");
                    deletes.push(id.to_string());
                }
            }
            _ => tracing::warn!(
                "[REPL {}] unknown op_type {} at seq {}",
                tenant_id,
                op_entry.op_type,
                op_entry.seq
            ),
        }
    }

    // Resolve batch ordering: when the same doc ID appears in both upserts and
    // deletes, only the final operation (by LWW timestamp) should be applied.
    // Without this, all upserts are applied first then all deletes, which would
    // cause a later re-upsert to be incorrectly deleted.
    upserts.retain(|doc| final_op_type.get(&doc.id).copied().unwrap_or("upsert") == "upsert");
    deletes.retain(|id| final_op_type.get(id.as_str()).copied().unwrap_or("delete") == "delete");

    // Deduplicate upserts: keep only the last version for each doc ID.
    // tantivy's delete_term only affects pre-existing docs, so adding two
    // docs with the same ID in one batch leaves both in the index.
    {
        let mut seen = std::collections::HashSet::new();
        let mut deduped = Vec::with_capacity(upserts.len());
        for doc in upserts.into_iter().rev() {
            if seen.insert(doc.id.clone()) {
                deduped.push(doc);
            }
        }
        deduped.reverse();
        upserts = deduped;
    }

    if !upserts.is_empty() {
        // Use for_replication variant so write_queue does NOT overwrite the op timestamps
        // already recorded in lww_map above with a newer system timestamp.
        manager
            .add_documents_for_replication(tenant_id, upserts)
            .map_err(|e| format!("add_documents failed: {}", e))?;
    }

    if !deletes.is_empty() {
        manager
            .delete_documents_sync_for_replication(tenant_id, deletes)
            .await
            .map_err(|e| format!("delete_documents failed: {}", e))?;
    }

    Ok(max_seq)
}

/// POST /internal/replicate
/// Receive operations from a peer and apply them to local index.
pub async fn replicate_ops(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ReplicateOpsRequest>,
) -> impl IntoResponse {
    let tenant_id = req.tenant_id.clone();

    match apply_ops_to_manager(&state.manager, &tenant_id, &req.ops).await {
        Ok(max_seq) => {
            tracing::info!(
                "[REPL {}] applied {} ops (max_seq={})",
                tenant_id,
                req.ops.len(),
                max_seq
            );
            (
                StatusCode::OK,
                Json(ReplicateOpsResponse {
                    tenant_id,
                    acked_seq: max_seq,
                }),
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!("[REPL {}] failed to apply ops: {}", tenant_id, e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e })),
            )
                .into_response()
        }
    }
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

    let storage_total_bytes: u64 = state
        .manager
        .all_tenant_storage()
        .iter()
        .map(|(_, b)| b)
        .sum();
    let tenant_count = state.manager.loaded_count();

    #[cfg(feature = "vector-search")]
    let vector_memory_bytes = state.manager.vector_memory_usage();
    #[cfg(not(feature = "vector-search"))]
    let vector_memory_bytes = 0usize;

    let response = serde_json::json!({
        "node_id": node_id,
        "replication_enabled": replication_enabled,
        "peer_count": peer_count,
        "ssl_renewal": ssl_renewal,
        "storage_total_bytes": storage_total_bytes,
        "tenant_count": tenant_count,
        "vector_memory_bytes": vector_memory_bytes,
    });

    (StatusCode::OK, Json(response)).into_response()
}

/// GET /internal/cluster/status
/// Return health status of all peers based on last_success timestamps.
/// Provides quick cluster health overview without active probing.
pub async fn cluster_status(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let repl_mgr = match &state.replication_manager {
        Some(r) => r,
        None => {
            return (
                StatusCode::OK,
                Json(serde_json::json!({
                    "node_id": std::env::var("FLAPJACK_NODE_ID").unwrap_or_else(|_| "unknown".to_string()),
                    "replication_enabled": false,
                    "peers": []
                })),
            )
                .into_response();
        }
    };

    let peers = repl_mgr
        .peer_statuses()
        .into_iter()
        .map(|ps| {
            serde_json::json!({
                "peer_id": ps.peer_id,
                "addr": ps.addr,
                "status": ps.status,
                "last_success_secs_ago": ps.last_success_secs_ago,
            })
        })
        .collect::<Vec<_>>();

    let healthy_count = peers.iter().filter(|p| p["status"] == "healthy").count();

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "node_id": repl_mgr.node_id(),
            "replication_enabled": true,
            "peers_total": repl_mgr.peer_count(),
            "peers_healthy": healthy_count,
            "peers": peers,
        })),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use flapjack::index::oplog::OpLogEntry;
    use flapjack::IndexManager;
    use tempfile::TempDir;

    fn make_upsert_op(
        seq: u64,
        ts: u64,
        node: &str,
        tenant: &str,
        id: &str,
        name: &str,
    ) -> OpLogEntry {
        OpLogEntry {
            seq,
            timestamp_ms: ts,
            node_id: node.to_string(),
            tenant_id: tenant.to_string(),
            op_type: "upsert".to_string(),
            payload: serde_json::json!({
                "objectID": id,
                "body": {"_id": id, "name": name}
            }),
        }
    }

    fn make_delete_op(seq: u64, ts: u64, node: &str, tenant: &str, id: &str) -> OpLogEntry {
        OpLogEntry {
            seq,
            timestamp_ms: ts,
            node_id: node.to_string(),
            tenant_id: tenant.to_string(),
            op_type: "delete".to_string(),
            payload: serde_json::json!({"objectID": id}),
        }
    }

    /// Poll until a document exists in the index (up to ~2s).
    /// Panics with a clear message if it never appears.
    async fn wait_for_doc_exists(manager: &IndexManager, tenant: &str, doc_id: &str) {
        for _ in 0..200 {
            if let Ok(Some(_)) = manager.get_document(tenant, doc_id) {
                return;
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        }
        panic!("{}[{}] never appeared in index after 2s", tenant, doc_id);
    }

    /// Poll until a document's text field equals the expected value (up to ~2s).
    /// Panics with a clear diff message if it never matches.
    async fn wait_for_field(
        manager: &IndexManager,
        tenant: &str,
        doc_id: &str,
        field: &str,
        expected: &str,
    ) {
        for _ in 0..200 {
            if let Ok(Some(doc)) = manager.get_document(tenant, doc_id) {
                if matches!(doc.fields.get(field), Some(flapjack::types::FieldValue::Text(s)) if s == expected)
                {
                    return;
                }
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        }
        let got = manager
            .get_document(tenant, doc_id)
            .ok()
            .flatten()
            .and_then(|d| d.fields.get(field).cloned());
        panic!(
            "{}[{}].{} never became {:?}; last value: {:?}",
            tenant, doc_id, field, expected, got
        );
    }

    // ── Basic apply ──

    #[tokio::test]
    async fn apply_ops_upsert_creates_document() {
        let tmp = TempDir::new().unwrap();
        let manager = IndexManager::new(tmp.path());
        let ops = vec![make_upsert_op(1, 1000, "node-a", "t1", "doc1", "Alice")];
        let result = apply_ops_to_manager(&manager, "t1", &ops).await;
        assert_eq!(result.unwrap(), 1);
        // Write queue is async — poll until committed
        wait_for_doc_exists(&manager, "t1", "doc1").await;
    }

    #[tokio::test]
    async fn apply_ops_delete_removes_document() {
        let tmp = TempDir::new().unwrap();
        let manager = IndexManager::new(tmp.path());
        // Insert first and confirm it's visible before testing deletion
        let upsert = vec![make_upsert_op(1, 1000, "node-a", "t1", "doc1", "Alice")];
        apply_ops_to_manager(&manager, "t1", &upsert).await.unwrap();
        wait_for_doc_exists(&manager, "t1", "doc1").await;
        // Now delete — delete_documents_sync_for_replication is synchronous
        let del = vec![make_delete_op(2, 2000, "node-a", "t1", "doc1")];
        apply_ops_to_manager(&manager, "t1", &del).await.unwrap();
        let doc = manager.get_document("t1", "doc1").unwrap();
        assert!(doc.is_none(), "doc1 should be gone after delete");
    }

    #[tokio::test]
    async fn apply_ops_returns_max_seq() {
        let tmp = TempDir::new().unwrap();
        let manager = IndexManager::new(tmp.path());
        let ops = vec![
            make_upsert_op(3, 1000, "node-a", "t1", "d1", "Alice"),
            make_upsert_op(7, 2000, "node-a", "t1", "d2", "Bob"),
            make_upsert_op(5, 1500, "node-a", "t1", "d3", "Carol"),
        ];
        let result = apply_ops_to_manager(&manager, "t1", &ops).await.unwrap();
        assert_eq!(result, 7, "should return max seq across all ops");
    }

    // ── LWW: newer timestamp wins ──

    #[tokio::test]
    async fn lww_newer_timestamp_overwrites_older() {
        let tmp = TempDir::new().unwrap();
        let manager = IndexManager::new(tmp.path());

        // Apply op at ts=2000 first — poll until it's visible
        let op_newer = vec![make_upsert_op(
            1,
            2000,
            "node-a",
            "t1",
            "doc1",
            "NewerAlice",
        )];
        apply_ops_to_manager(&manager, "t1", &op_newer)
            .await
            .unwrap();
        wait_for_field(&manager, "t1", "doc1", "name", "NewerAlice").await;

        // Apply op at ts=1000 (older) — REJECTED by LWW immediately, no async work
        let op_older = vec![make_upsert_op(
            2,
            1000,
            "node-b",
            "t1",
            "doc1",
            "OlderAlice",
        )];
        apply_ops_to_manager(&manager, "t1", &op_older)
            .await
            .unwrap();

        let doc = manager.get_document("t1", "doc1").unwrap().unwrap();
        let name = doc.fields.get("name");
        assert!(
            matches!(name, Some(flapjack::types::FieldValue::Text(s)) if s == "NewerAlice"),
            "newer write should win; got: {:?}",
            doc.fields.get("name")
        );
    }

    #[tokio::test]
    async fn lww_older_upsert_does_not_overwrite_newer() {
        let tmp = TempDir::new().unwrap();
        let manager = IndexManager::new(tmp.path());

        // Apply newer first, then try to apply older — both in one batch.
        // ts=5000 "Final" wins; ts=1000 "Stale" is deduped away before queuing.
        let ops = vec![
            make_upsert_op(1, 5000, "node-a", "t1", "doc1", "Final"),
            make_upsert_op(2, 1000, "node-b", "t1", "doc1", "Stale"),
        ];
        apply_ops_to_manager(&manager, "t1", &ops).await.unwrap();
        wait_for_field(&manager, "t1", "doc1", "name", "Final").await;

        let doc = manager.get_document("t1", "doc1").unwrap().unwrap();
        let name = doc.fields.get("name");
        assert!(
            matches!(name, Some(flapjack::types::FieldValue::Text(s)) if s == "Final"),
            "stale op should not overwrite newer; got: {:?}",
            doc.fields.get("name")
        );
    }

    // ── LWW: tie-break by node_id ──

    #[tokio::test]
    async fn lww_same_timestamp_higher_node_id_wins() {
        let tmp = TempDir::new().unwrap();
        let manager = IndexManager::new(tmp.path());

        // Apply from "z-node" — poll until visible
        let op_z = vec![make_upsert_op(1, 1000, "z-node", "t1", "doc1", "ZNode")];
        apply_ops_to_manager(&manager, "t1", &op_z).await.unwrap();
        wait_for_field(&manager, "t1", "doc1", "name", "ZNode").await;

        // "a-node" at same ts=1000 — REJECTED (z > a lexicographically), no async work
        let op_a = vec![make_upsert_op(2, 1000, "a-node", "t1", "doc1", "ANode")];
        apply_ops_to_manager(&manager, "t1", &op_a).await.unwrap();

        let doc = manager.get_document("t1", "doc1").unwrap().unwrap();
        let name = doc.fields.get("name");
        assert!(
            matches!(name, Some(flapjack::types::FieldValue::Text(s)) if s == "ZNode"),
            "z-node (higher lexicographic) should win tie-break; got: {:?}",
            doc.fields.get("name")
        );
    }

    // ── LWW: stale delete is rejected ──

    #[tokio::test]
    async fn lww_stale_delete_does_not_remove_newer_upsert() {
        let tmp = TempDir::new().unwrap();
        let manager = IndexManager::new(tmp.path());

        // Write doc at ts=2000 — poll until visible
        let upsert = vec![make_upsert_op(1, 2000, "node-a", "t1", "doc1", "Alice")];
        apply_ops_to_manager(&manager, "t1", &upsert).await.unwrap();
        wait_for_doc_exists(&manager, "t1", "doc1").await;

        // Try to delete with stale ts=1000 — REJECTED immediately by LWW, no async work
        let del = vec![make_delete_op(2, 1000, "node-b", "t1", "doc1")];
        apply_ops_to_manager(&manager, "t1", &del).await.unwrap();

        let doc = manager.get_document("t1", "doc1").unwrap();
        assert!(doc.is_some(), "stale delete should not remove a newer doc");
    }

    // ── LWW: same-node ops always apply in sequence ──

    #[tokio::test]
    async fn lww_same_node_sequential_ops_always_apply() {
        let tmp = TempDir::new().unwrap();
        let manager = IndexManager::new(tmp.path());

        // V1 first — poll until visible
        let op1 = vec![make_upsert_op(1, 1000, "node-a", "t1", "doc1", "V1")];
        apply_ops_to_manager(&manager, "t1", &op1).await.unwrap();
        wait_for_field(&manager, "t1", "doc1", "name", "V1").await;

        // V2 newer timestamp — accepted, poll until visible
        let op2 = vec![make_upsert_op(2, 2000, "node-a", "t1", "doc1", "V2")];
        apply_ops_to_manager(&manager, "t1", &op2).await.unwrap();
        wait_for_field(&manager, "t1", "doc1", "name", "V2").await;

        let doc = manager.get_document("t1", "doc1").unwrap().unwrap();
        let name = doc.fields.get("name");
        assert!(
            matches!(name, Some(flapjack::types::FieldValue::Text(s)) if s == "V2"),
            "sequential ops from same node should apply in order; got: {:?}",
            doc.fields.get("name")
        );
    }

    // ── LWW: primary write blocks stale replicated op ──
    // This test validates the fix for the "known limitation" from session 23:
    // primary-written docs must populate lww_map so stale replicated ops are rejected.

    #[tokio::test]
    async fn lww_primary_write_blocks_stale_replicated_op() {
        let tmp = TempDir::new().unwrap();
        let manager = IndexManager::new(tmp.path());

        // Write a doc via the primary path (add_documents_sync — goes through write_queue)
        let doc = flapjack::types::Document {
            id: "doc1".to_string(),
            fields: {
                let mut m = std::collections::HashMap::new();
                m.insert(
                    "name".to_string(),
                    flapjack::types::FieldValue::Text("Primary".to_string()),
                );
                m
            },
        };
        manager.create_tenant("t1").unwrap();
        manager.add_documents_sync("t1", vec![doc]).await.unwrap();

        // Confirm lww_map was populated by the write_queue
        let lww = manager.get_lww("t1", "doc1");
        assert!(
            lww.is_some(),
            "primary write must populate lww_map; got None"
        );
        let (primary_ts, _) = lww.unwrap();
        assert!(
            primary_ts > 0,
            "primary_ts should be a real system timestamp"
        );

        // Now try to replicate a stale op with ts=1 (much older than primary write).
        // LWW rejects this before queuing — no async work, result is immediately visible.
        let stale_op = vec![make_upsert_op(99, 1, "remote-node", "t1", "doc1", "Stale")];
        apply_ops_to_manager(&manager, "t1", &stale_op)
            .await
            .unwrap();

        // The stale replicated op must NOT overwrite the primary write
        let fetched = manager.get_document("t1", "doc1").unwrap().unwrap();
        let name = fetched.fields.get("name");
        assert!(
            matches!(name, Some(flapjack::types::FieldValue::Text(s)) if s == "Primary"),
            "stale replicated op must not overwrite primary write; got: {:?}",
            name
        );
    }

    // ── LWW: primary delete blocks stale replicated upsert ──

    #[tokio::test]
    async fn lww_primary_delete_blocks_stale_replicated_upsert() {
        let tmp = TempDir::new().unwrap();
        let manager = IndexManager::new(tmp.path());

        // First write the doc via primary path
        let doc = flapjack::types::Document {
            id: "doc1".to_string(),
            fields: {
                let mut m = std::collections::HashMap::new();
                m.insert(
                    "name".to_string(),
                    flapjack::types::FieldValue::Text("Primary".to_string()),
                );
                m
            },
        };
        manager.create_tenant("t1").unwrap();
        manager.add_documents_sync("t1", vec![doc]).await.unwrap();

        // Delete via primary path
        manager
            .delete_documents_sync("t1", vec!["doc1".to_string()])
            .await
            .unwrap();

        // Confirm lww_map records the delete timestamp
        let lww = manager.get_lww("t1", "doc1");
        assert!(lww.is_some(), "primary delete must populate lww_map");

        // Now try to replicate a stale upsert with ts=1 — REJECTED by LWW immediately.
        // No async work queued; result is visible without waiting.
        let stale_upsert = vec![make_upsert_op(
            99,
            1,
            "remote-node",
            "t1",
            "doc1",
            "StaleRevive",
        )];
        apply_ops_to_manager(&manager, "t1", &stale_upsert)
            .await
            .unwrap();

        let doc = manager.get_document("t1", "doc1").unwrap();
        assert!(
            doc.is_none(),
            "stale replicated upsert must not revive a primary-deleted doc"
        );
    }

    // ── LWW: lww_map rebuilt from oplog on restart (P3) ──
    // Without P3: after restart lww_map is empty → stale replicated ops bypass LWW
    // With P3:    recover_from_oplog rebuilds lww_map → stale ops correctly rejected

    #[tokio::test]
    async fn lww_map_rebuilt_from_oplog_blocks_stale_op_after_restart() {
        let tmp = TempDir::new().unwrap();
        let base = tmp.path().to_path_buf();

        // PHASE 1: Primary write — establishes LWW state in oplog
        let primary_ts;
        {
            let manager = IndexManager::new(&base);
            manager.create_tenant("t_restart").unwrap();
            let doc = flapjack::types::Document {
                id: "doc1".to_string(),
                fields: {
                    let mut m = std::collections::HashMap::new();
                    m.insert(
                        "name".to_string(),
                        flapjack::types::FieldValue::Text("Original".to_string()),
                    );
                    m
                },
            };
            manager
                .add_documents_sync("t_restart", vec![doc])
                .await
                .unwrap();

            // Capture oplog timestamp — this is what LWW must be rebuilt from
            let oplog = manager.get_or_create_oplog("t_restart").unwrap();
            let ops = oplog.read_since(0).unwrap();
            let upsert_op = ops
                .iter()
                .find(|o| o.op_type == "upsert")
                .expect("should have upsert in oplog after primary write");
            primary_ts = upsert_op.timestamp_ms;
            assert!(primary_ts > 0, "oplog should record a real timestamp");

            manager.graceful_shutdown().await;
        }

        // PHASE 2: Restart (new IndexManager = fresh empty lww_map until P3 fix)
        {
            let manager = IndexManager::new(&base);

            // Try to apply a stale replicated op (1ms before the primary write).
            // With P3: lww_map rebuilt from oplog → REJECTED immediately.
            // Without P3: would be accepted (queued async) — we poll briefly to detect that case.
            let stale_op = vec![make_upsert_op(
                99,
                primary_ts.saturating_sub(1),
                "remote-node",
                "t_restart",
                "doc1",
                "StaleOverwrite",
            )];
            apply_ops_to_manager(&manager, "t_restart", &stale_op)
                .await
                .unwrap();

            // Poll briefly — if P3 is broken the write queue would commit "StaleOverwrite"
            tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

            // P3: lww_map rebuilt from oplog → stale op rejected → "Original" survives
            let fetched = manager.get_document("t_restart", "doc1").unwrap();
            assert!(
                fetched.is_some(),
                "doc1 must exist (was written by primary)"
            );
            let name = fetched.unwrap().fields.get("name").cloned();
            assert!(
                matches!(&name, Some(flapjack::types::FieldValue::Text(s)) if s == "Original"),
                "stale replicated op must not overwrite after restart; lww_map must be rebuilt from oplog. got: {:?}",
                name
            );

            manager.graceful_shutdown().await;
        }
    }

    // ── LWW: lww_map rebuilt for normal restart (no uncommitted ops) ──
    // Covers the case where committed_seq is current (normal shutdown, not crash).
    // recover_from_oplog must still rebuild lww_map even when there's nothing to replay.

    #[tokio::test]
    async fn lww_map_rebuilt_on_normal_restart_no_uncommitted_ops() {
        let tmp = TempDir::new().unwrap();
        let base = tmp.path().to_path_buf();

        let primary_ts;
        {
            let manager = IndexManager::new(&base);
            manager.create_tenant("t_normal_restart").unwrap();
            let doc = flapjack::types::Document {
                id: "docA".to_string(),
                fields: {
                    let mut m = std::collections::HashMap::new();
                    m.insert(
                        "name".to_string(),
                        flapjack::types::FieldValue::Text("Persisted".to_string()),
                    );
                    m
                },
            };
            manager
                .add_documents_sync("t_normal_restart", vec![doc])
                .await
                .unwrap();

            let oplog = manager.get_or_create_oplog("t_normal_restart").unwrap();
            let ops = oplog.read_since(0).unwrap();
            primary_ts = ops
                .iter()
                .find(|o| o.op_type == "upsert")
                .map(|o| o.timestamp_ms)
                .unwrap_or(0);
            assert!(primary_ts > 0);

            // Normal clean shutdown: committed_seq is updated, no uncommitted ops
            manager.graceful_shutdown().await;
        }

        // Restart: committed_seq is current → no document replay needed.
        // But lww_map must still be rebuilt so stale ops are rejected.
        {
            let manager = IndexManager::new(&base);

            let stale_op = vec![make_upsert_op(
                99,
                primary_ts.saturating_sub(1),
                "remote-node",
                "t_normal_restart",
                "docA",
                "ShouldBeRejected",
            )];
            apply_ops_to_manager(&manager, "t_normal_restart", &stale_op)
                .await
                .unwrap();

            // Stale upsert is rejected by LWW (P3 correct). If P3 were broken the write
            // queue would commit "ShouldBeRejected" asynchronously — wait briefly to detect that.
            tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

            let fetched = manager.get_document("t_normal_restart", "docA").unwrap();
            assert!(fetched.is_some());
            let name = fetched.unwrap().fields.get("name").cloned();
            assert!(
                matches!(&name, Some(flapjack::types::FieldValue::Text(s)) if s == "Persisted"),
                "stale op must be rejected even after clean shutdown restart; got: {:?}",
                name
            );

            manager.graceful_shutdown().await;
        }
    }

    // ── LWW: lww_map rebuild blocks stale DELETE after restart (P3) ──
    // Variant of the P3 crash test but with a stale DELETE instead of a stale UPSERT.
    // A stale replicated delete arriving after restart must NOT remove a newer primary write.

    #[tokio::test]
    async fn lww_map_rebuilt_from_oplog_blocks_stale_delete_after_restart() {
        let tmp = TempDir::new().unwrap();
        let base = tmp.path().to_path_buf();

        let primary_ts;
        {
            let manager = IndexManager::new(&base);
            manager.create_tenant("t_del_restart").unwrap();
            let doc = flapjack::types::Document {
                id: "doc1".to_string(),
                fields: {
                    let mut m = std::collections::HashMap::new();
                    m.insert(
                        "name".to_string(),
                        flapjack::types::FieldValue::Text("ShouldSurvive".to_string()),
                    );
                    m
                },
            };
            manager
                .add_documents_sync("t_del_restart", vec![doc])
                .await
                .unwrap();

            let oplog = manager.get_or_create_oplog("t_del_restart").unwrap();
            let ops = oplog.read_since(0).unwrap();
            primary_ts = ops
                .iter()
                .find(|o| o.op_type == "upsert")
                .map(|o| o.timestamp_ms)
                .expect("should have upsert in oplog");
            assert!(primary_ts > 0);

            manager.graceful_shutdown().await;
        }

        // Restart: lww_map is rebuilt from oplog → stale delete ts=primary_ts-1 must be rejected
        {
            let manager = IndexManager::new(&base);

            let stale_delete = vec![make_delete_op(
                99,
                primary_ts.saturating_sub(1),
                "remote-node",
                "t_del_restart",
                "doc1",
            )];
            apply_ops_to_manager(&manager, "t_del_restart", &stale_delete)
                .await
                .unwrap();

            // Stale delete is rejected by LWW (P3 correct). If P3 were broken, the delete
            // runs synchronously via delete_documents_sync_for_replication (also .awaited),
            // so the outcome is committed before apply_ops_to_manager returns — no sleep needed.
            let fetched = manager.get_document("t_del_restart", "doc1").unwrap();
            assert!(
                fetched.is_some(),
                "stale delete must not remove doc after restart; lww_map must be rebuilt from oplog"
            );

            manager.graceful_shutdown().await;
        }
    }

    // ── Batch ordering: upsert→delete→re-upsert in a single batch ──
    // Regression test: apply_ops_to_manager used to split ops into separate
    // upserts and deletes lists, applying all upserts first then all deletes.
    // This caused a later re-upsert (ts=3000) to be overridden by an earlier
    // delete (ts=2000) because the delete was applied after the upsert.

    #[tokio::test]
    async fn batch_upsert_delete_reupsert_same_doc_keeps_final_upsert() {
        let tmp = TempDir::new().unwrap();
        let manager = IndexManager::new(tmp.path());

        // Single batch: create → delete → re-create the SAME doc
        let ops = vec![
            make_upsert_op(1, 1000, "node-a", "t1", "doc1", "Version1"),
            make_delete_op(2, 2000, "node-a", "t1", "doc1"),
            make_upsert_op(3, 3000, "node-a", "t1", "doc1", "Version3"),
        ];
        let result = apply_ops_to_manager(&manager, "t1", &ops).await;
        assert_eq!(result.unwrap(), 3);

        // Wait for write queue to commit — the ts=3000 re-upsert must win over the ts=2000 delete
        wait_for_field(&manager, "t1", "doc1", "name", "Version3").await;
    }

    #[tokio::test]
    async fn batch_upsert_then_delete_same_doc_deletes() {
        let tmp = TempDir::new().unwrap();
        let manager = IndexManager::new(tmp.path());

        // Single batch: create → delete the SAME doc (delete is final)
        let ops = vec![
            make_upsert_op(1, 1000, "node-a", "t1", "doc1", "ToDelete"),
            make_delete_op(2, 2000, "node-a", "t1", "doc1"),
        ];
        apply_ops_to_manager(&manager, "t1", &ops).await.unwrap();

        // The ts=2000 delete wins: the upsert is filtered from the batch, and the delete
        // runs synchronously via delete_documents_sync_for_replication. No sleep needed.
        let doc = manager.get_document("t1", "doc1").unwrap();
        assert!(
            doc.is_none(),
            "doc1 should be deleted — the ts=2000 delete is the final op"
        );
    }

    // ── Unknown op type skipped gracefully ──

    #[tokio::test]
    async fn apply_ops_unknown_type_skipped() {
        let tmp = TempDir::new().unwrap();
        let manager = IndexManager::new(tmp.path());
        let op = OpLogEntry {
            seq: 1,
            timestamp_ms: 1000,
            node_id: "node-a".to_string(),
            tenant_id: "t1".to_string(),
            op_type: "noop_unknown".to_string(),
            payload: serde_json::json!({}),
        };
        // Should not panic, just skip
        let result = apply_ops_to_manager(&manager, "t1", &[op]).await;
        assert_eq!(result.unwrap(), 1);
    }

    // ── /internal/storage endpoint tests ──

    use crate::handlers::metrics::MetricsState;
    use axum::body::Body;
    use axum::http::Request;
    use axum::routing::get;
    use axum::Router;
    use tower::ServiceExt;

    fn make_storage_state(tmp: &TempDir) -> Arc<AppState> {
        Arc::new(AppState {
            manager: IndexManager::new(tmp.path()),
            key_store: None,
            replication_manager: None,
            ssl_manager: None,
            analytics_engine: None,
            experiment_store: None,
            metrics_state: Some(MetricsState::new()),
            usage_counters: std::sync::Arc::new(dashmap::DashMap::new()),
            paused_indexes: crate::pause_registry::PausedIndexes::new(),
            start_time: std::time::Instant::now(),
            #[cfg(feature = "vector-search")]
            embedder_store: std::sync::Arc::new(crate::embedder_store::EmbedderStore::new()),
        })
    }

    #[tokio::test]
    async fn storage_all_returns_tenant_list() {
        let tmp = TempDir::new().unwrap();
        let state = make_storage_state(&tmp);

        state.manager.create_tenant("tenant_a").unwrap();
        state.manager.create_tenant("tenant_b").unwrap();

        let app = Router::new()
            .route("/internal/storage", get(super::storage_all))
            .with_state(state);

        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/internal/storage")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

        let tenants = json["tenants"].as_array().unwrap();
        assert_eq!(tenants.len(), 2, "should have 2 tenants");

        let ids: Vec<&str> = tenants.iter().map(|t| t["id"].as_str().unwrap()).collect();
        assert!(ids.contains(&"tenant_a"), "should contain tenant_a");
        assert!(ids.contains(&"tenant_b"), "should contain tenant_b");

        // Each tenant should have bytes field > 0 (tantivy creates meta files)
        for t in tenants {
            assert!(
                t["bytes"].as_u64().unwrap() > 0,
                "tenant {} should have non-zero bytes",
                t["id"]
            );
        }
    }

    #[tokio::test]
    async fn storage_index_returns_bytes_for_specific_tenant() {
        let tmp = TempDir::new().unwrap();
        let state = make_storage_state(&tmp);

        state.manager.create_tenant("my_index").unwrap();

        let app = Router::new()
            .route("/internal/storage/:indexName", get(super::storage_index))
            .with_state(state);

        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/internal/storage/my_index")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(json["index"].as_str().unwrap(), "my_index");
        assert!(
            json["bytes"].as_u64().unwrap() > 0,
            "existing tenant should have non-zero bytes"
        );
    }

    #[tokio::test]
    async fn storage_index_returns_zero_for_nonexistent() {
        let tmp = TempDir::new().unwrap();
        let state = make_storage_state(&tmp);

        let app = Router::new()
            .route("/internal/storage/:indexName", get(super::storage_index))
            .with_state(state);

        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/internal/storage/no_such_index")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(json["index"].as_str().unwrap(), "no_such_index");
        assert_eq!(
            json["bytes"].as_u64().unwrap(),
            0,
            "nonexistent tenant should have 0 bytes"
        );
    }

    // ── doc_count in /internal/storage ──

    #[tokio::test]
    async fn storage_index_includes_doc_count() {
        let tmp = TempDir::new().unwrap();
        let state = make_storage_state(&tmp);

        state.manager.create_tenant("dc_test").unwrap();
        let docs = vec![
            flapjack::types::Document {
                id: "d1".to_string(),
                fields: std::collections::HashMap::from([(
                    "name".to_string(),
                    flapjack::types::FieldValue::Text("Alice".to_string()),
                )]),
            },
            flapjack::types::Document {
                id: "d2".to_string(),
                fields: std::collections::HashMap::from([(
                    "name".to_string(),
                    flapjack::types::FieldValue::Text("Bob".to_string()),
                )]),
            },
        ];
        state
            .manager
            .add_documents_sync("dc_test", docs)
            .await
            .unwrap();

        let app = Router::new()
            .route("/internal/storage/:indexName", get(super::storage_index))
            .with_state(state);

        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/internal/storage/dc_test")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(json["doc_count"].as_u64().unwrap(), 2, "should have 2 docs");
    }

    #[tokio::test]
    async fn storage_all_includes_doc_count() {
        let tmp = TempDir::new().unwrap();
        let state = make_storage_state(&tmp);

        state.manager.create_tenant("t_dc").unwrap();
        let docs = vec![flapjack::types::Document {
            id: "d1".to_string(),
            fields: std::collections::HashMap::from([(
                "name".to_string(),
                flapjack::types::FieldValue::Text("Alice".to_string()),
            )]),
        }];
        state
            .manager
            .add_documents_sync("t_dc", docs)
            .await
            .unwrap();

        let app = Router::new()
            .route("/internal/storage", get(super::storage_all))
            .with_state(state);

        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/internal/storage")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

        let tenants = json["tenants"].as_array().unwrap();
        let tenant = tenants.iter().find(|t| t["id"] == "t_dc").unwrap();
        assert_eq!(
            tenant["doc_count"].as_u64().unwrap(),
            1,
            "should have 1 doc"
        );
    }

    // ── /internal/status enhancements ──

    #[tokio::test]
    async fn status_includes_storage_total_and_tenant_count() {
        let tmp = TempDir::new().unwrap();
        let state = make_storage_state(&tmp);

        state.manager.create_tenant("s1").unwrap();
        state.manager.create_tenant("s2").unwrap();

        let app = Router::new()
            .route("/internal/status", get(super::replication_status))
            .with_state(state);

        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/internal/status")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert!(
            json["storage_total_bytes"].as_u64().is_some(),
            "should have storage_total_bytes"
        );
        assert!(
            json["storage_total_bytes"].as_u64().unwrap() > 0,
            "total bytes should be > 0 with 2 tenants"
        );
        assert_eq!(
            json["tenant_count"].as_u64().unwrap(),
            2,
            "should have 2 tenants loaded"
        );
    }

    #[cfg(feature = "vector-search")]
    #[tokio::test]
    async fn test_internal_status_includes_vector_memory() {
        let tmp = TempDir::new().unwrap();
        let state = make_storage_state(&tmp);

        // Add some vectors so memory > 0
        let mut vi =
            flapjack::vector::index::VectorIndex::new(3, flapjack::vector::MetricKind::Cos)
                .unwrap();
        vi.add("doc1", &[1.0, 0.0, 0.0]).unwrap();
        vi.add("doc2", &[0.0, 1.0, 0.0]).unwrap();
        state.manager.set_vector_index("vec_tenant", vi);

        let app = Router::new()
            .route("/internal/status", get(super::replication_status))
            .with_state(state);

        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/internal/status")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert!(
            json["vector_memory_bytes"].is_number(),
            "status response should include vector_memory_bytes field, got: {:?}",
            json
        );
        assert!(
            json["vector_memory_bytes"].as_u64().unwrap() > 0,
            "vector_memory_bytes should be > 0 when vectors exist"
        );
    }

    // ── Pause endpoint tests ──

    fn make_pause_app(state: Arc<AppState>) -> Router {
        Router::new()
            .route(
                "/internal/pause/:indexName",
                axum::routing::post(super::pause_index),
            )
            .route(
                "/internal/resume/:indexName",
                axum::routing::post(super::resume_index),
            )
            .with_state(state)
    }

    #[tokio::test]
    async fn test_pause_endpoint_returns_200() {
        let tmp = TempDir::new().unwrap();
        let state = make_storage_state(&tmp);
        let app = make_pause_app(state);

        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/internal/pause/foo")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["index"], "foo");
        assert_eq!(json["paused"], true);
    }

    #[tokio::test]
    async fn test_pause_endpoint_unknown_index_still_200() {
        let tmp = TempDir::new().unwrap();
        let state = make_storage_state(&tmp);
        let app = make_pause_app(state);

        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/internal/pause/nonexistent_index")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_pause_endpoint_marks_index_in_registry() {
        let tmp = TempDir::new().unwrap();
        let state = make_storage_state(&tmp);
        let app = make_pause_app(state.clone());

        let _resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/internal/pause/foo")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert!(
            state.paused_indexes.is_paused("foo"),
            "registry should show foo as paused after endpoint call"
        );
    }

    #[tokio::test]
    async fn test_pause_endpoint_double_call_idempotent() {
        let tmp = TempDir::new().unwrap();
        let state = make_storage_state(&tmp);

        // First call
        let app1 = make_pause_app(state.clone());
        let resp1 = app1
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/internal/pause/foo")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp1.status(), StatusCode::OK);

        // Second call (same index)
        let app2 = make_pause_app(state);
        let resp2 = app2
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/internal/pause/foo")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp2.status(), StatusCode::OK);
    }

    // ── Resume endpoint tests ──

    #[tokio::test]
    async fn test_resume_endpoint_returns_200() {
        let tmp = TempDir::new().unwrap();
        let state = make_storage_state(&tmp);
        // Pause first so there's something to resume
        state.paused_indexes.pause("foo");
        let app = make_pause_app(state);

        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/internal/resume/foo")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["index"], "foo");
        assert_eq!(json["paused"], false);
    }

    #[tokio::test]
    async fn test_resume_endpoint_unknown_index_still_200() {
        let tmp = TempDir::new().unwrap();
        let state = make_storage_state(&tmp);
        let app = make_pause_app(state);

        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/internal/resume/never_paused")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_resume_endpoint_clears_pause_in_registry() {
        let tmp = TempDir::new().unwrap();
        let state = make_storage_state(&tmp);
        state.paused_indexes.pause("foo");
        let app = make_pause_app(state.clone());

        let _resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/internal/resume/foo")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert!(
            !state.paused_indexes.is_paused("foo"),
            "foo should no longer be paused after resume endpoint"
        );
    }

    #[tokio::test]
    async fn test_resume_endpoint_double_call_idempotent() {
        let tmp = TempDir::new().unwrap();
        let state = make_storage_state(&tmp);

        // First resume (not paused — should still be 200)
        let app1 = make_pause_app(state.clone());
        let resp1 = app1
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/internal/resume/foo")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp1.status(), StatusCode::OK);

        // Second resume
        let app2 = make_pause_app(state);
        let resp2 = app2
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/internal/resume/foo")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp2.status(), StatusCode::OK);
    }

    // ── Full cycle integration test (2I) ────────────────────────────────

    #[tokio::test]
    async fn test_full_pause_write_resume_cycle() {
        let tmp = TempDir::new().unwrap();
        let state = make_storage_state(&tmp);

        // Build a combined router with pause/resume + write + search endpoints
        fn make_cycle_app(state: Arc<AppState>) -> Router {
            Router::new()
                .route(
                    "/internal/pause/:indexName",
                    axum::routing::post(super::pause_index),
                )
                .route(
                    "/internal/resume/:indexName",
                    axum::routing::post(super::resume_index),
                )
                .route(
                    "/1/indexes/:indexName/batch",
                    axum::routing::post(crate::handlers::objects::add_documents),
                )
                .route(
                    "/1/indexes/:indexName/query",
                    axum::routing::post(crate::handlers::search::search),
                )
                .with_state(state)
        }

        // Step 1: Write before pause — should NOT be 503
        let app = make_cycle_app(state.clone());
        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/1/indexes/products/batch")
                    .header("Content-Type", "application/json")
                    .body(Body::from("{}"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_ne!(
            resp.status(),
            StatusCode::SERVICE_UNAVAILABLE,
            "step 1: write before pause should NOT return 503"
        );

        // Step 2: Pause "products"
        let app = make_cycle_app(state.clone());
        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/internal/pause/products")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            resp.status(),
            StatusCode::OK,
            "step 2: pause should return 200"
        );

        // Step 3: Write while paused — should be 503
        let app = make_cycle_app(state.clone());
        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/1/indexes/products/batch")
                    .header("Content-Type", "application/json")
                    .body(Body::from("{}"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            resp.status(),
            StatusCode::SERVICE_UNAVAILABLE,
            "step 3: write while paused should return 503"
        );
        // Verify Retry-After header is present (required by 2B checklist)
        assert_eq!(
            resp.headers()
                .get("Retry-After")
                .and_then(|v| v.to_str().ok()),
            Some("1"),
            "step 3: 503 response should include Retry-After: 1 header"
        );
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(
            json["error"], "index_paused",
            "step 3: error code should be index_paused"
        );

        // Step 4: Search/read while paused — reads must NOT be blocked
        let app = make_cycle_app(state.clone());
        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/1/indexes/products/query")
                    .header("Content-Type", "application/json")
                    .body(Body::from(r#"{"query":""}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_ne!(
            resp.status(),
            StatusCode::SERVICE_UNAVAILABLE,
            "step 4: search while paused must NOT return 503 — reads are never blocked"
        );

        // Step 5: Resume "products"
        let app = make_cycle_app(state.clone());
        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/internal/resume/products")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            resp.status(),
            StatusCode::OK,
            "step 5: resume should return 200"
        );

        // Step 6: Write after resume — should NOT be 503
        let app = make_cycle_app(state.clone());
        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/1/indexes/products/batch")
                    .header("Content-Type", "application/json")
                    .body(Body::from("{}"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_ne!(
            resp.status(),
            StatusCode::SERVICE_UNAVAILABLE,
            "step 6: write after resume should NOT return 503"
        );
    }
}

/// POST /internal/analytics-rollup
///
/// Receive a pre-computed analytics rollup from a peer and store it in the
/// global rollup cache. Part of Phase 4 (HA Analytics Tier 2).
///
/// No authentication required — relies on network isolation for peer trust,
/// same as the other /internal/* endpoints.
pub async fn receive_analytics_rollup(
    Json(rollup): Json<crate::analytics_cluster::AnalyticsRollup>,
) -> impl IntoResponse {
    let cache = crate::analytics_cluster::get_global_rollup_cache();
    tracing::debug!(
        "[ROLLUP] received rollup from peer={} index={} generated_at={}",
        rollup.node_id,
        rollup.index,
        rollup.generated_at_secs
    );
    cache.store(rollup);
    (StatusCode::OK, Json(serde_json::json!({"status": "ok"}))).into_response()
}

/// GET /internal/rollup-cache
///
/// Diagnostic endpoint: returns all entries currently stored in the global
/// rollup cache. Used by tests and operators to inspect the Tier 2 cache state.
///
/// Response: `{"count": N, "entries": [AnalyticsRollup, ...]}`
pub async fn rollup_cache_status() -> impl IntoResponse {
    let cache = crate::analytics_cluster::get_global_rollup_cache();
    let entries = cache.all_entries();
    let count = entries.len();
    (
        StatusCode::OK,
        Json(serde_json::json!({
            "count": count,
            "entries": entries
        })),
    )
        .into_response()
}

/// GET /internal/storage
/// Returns disk usage and doc count for all loaded tenants.
pub async fn storage_all(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let tenants: Vec<serde_json::Value> = state
        .manager
        .all_tenant_storage()
        .into_iter()
        .map(|(id, bytes)| {
            let doc_count = state.manager.tenant_doc_count(&id).unwrap_or(0);
            serde_json::json!({"id": id, "bytes": bytes, "doc_count": doc_count})
        })
        .collect();

    (
        StatusCode::OK,
        Json(serde_json::json!({ "tenants": tenants })),
    )
        .into_response()
}

/// GET /internal/storage/:indexName
/// Returns disk usage and doc count for a specific tenant.
pub async fn storage_index(
    State(state): State<Arc<AppState>>,
    Path(index_name): Path<String>,
) -> impl IntoResponse {
    let bytes = state.manager.tenant_storage_bytes(&index_name);
    let doc_count = state.manager.tenant_doc_count(&index_name).unwrap_or(0);
    (
        StatusCode::OK,
        Json(serde_json::json!({ "index": index_name, "bytes": bytes, "doc_count": doc_count })),
    )
        .into_response()
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

/// POST /internal/pause/:indexName
/// Mark an index as paused — writes will be rejected with 503.
pub async fn pause_index(
    State(state): State<Arc<AppState>>,
    Path(index_name): Path<String>,
) -> impl IntoResponse {
    state.paused_indexes.pause(&index_name);
    tracing::info!("[PAUSE] index '{}' paused", index_name);
    (
        StatusCode::OK,
        Json(serde_json::json!({"index": index_name, "paused": true})),
    )
        .into_response()
}

/// POST /internal/resume/:indexName
/// Clear the paused flag — writes resume normally.
pub async fn resume_index(
    State(state): State<Arc<AppState>>,
    Path(index_name): Path<String>,
) -> impl IntoResponse {
    state.paused_indexes.resume(&index_name);
    tracing::info!("[PAUSE] index '{}' resumed", index_name);
    (
        StatusCode::OK,
        Json(serde_json::json!({"index": index_name, "paused": false})),
    )
        .into_response()
}
