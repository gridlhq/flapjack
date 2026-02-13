//! Async write queue with hybrid batching for Flapjack.

use crate::types::{DocFailure, Document, TaskInfo, TaskStatus};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio::time::timeout_at;

pub enum WriteAction {
    Add(Document),
    Upsert(Document),
    Delete(String),
    Compact,
}

pub struct WriteOp {
    pub task_id: String,
    pub actions: Vec<WriteAction>,
}

pub type WriteQueue = mpsc::Sender<WriteOp>;

pub fn create_write_queue(
    tenant_id: String,
    index: Arc<crate::index::Index>,
    writers: Arc<
        dashmap::DashMap<String, Arc<tokio::sync::Mutex<crate::index::ManagedIndexWriter>>>,
    >,
    tasks: Arc<dashmap::DashMap<String, TaskInfo>>,
    base_path: std::path::PathBuf,
    oplog: Option<Arc<crate::index::oplog::OpLog>>,
    facet_cache: Arc<
        dashmap::DashMap<
            String,
            Arc<(
                std::time::Instant,
                usize,
                std::collections::HashMap<String, Vec<crate::types::FacetCount>>,
            )>,
        >,
    >,
) -> (
    WriteQueue,
    tokio::task::JoinHandle<crate::error::Result<()>>,
) {
    let (tx, rx) = mpsc::channel(1000);

    if let Some(ref ol) = oplog {
        tracing::info!(
            "[WQ {}] using shared oplog, seq={}",
            tenant_id,
            ol.current_seq()
        );
    }

    let handle = tokio::spawn(async move {
        process_writes(
            tenant_id,
            index,
            writers,
            tasks,
            rx,
            base_path,
            oplog,
            facet_cache,
        )
        .await
    });

    (tx, handle)
}

async fn process_writes(
    tenant_id: String,
    index: Arc<crate::index::Index>,
    _writers: Arc<
        dashmap::DashMap<String, Arc<tokio::sync::Mutex<crate::index::ManagedIndexWriter>>>,
    >,
    tasks: Arc<dashmap::DashMap<String, TaskInfo>>,
    mut rx: mpsc::Receiver<WriteOp>,
    base_path: std::path::PathBuf,
    oplog: Option<Arc<crate::index::oplog::OpLog>>,
    facet_cache: Arc<
        dashmap::DashMap<
            String,
            Arc<(
                std::time::Instant,
                usize,
                std::collections::HashMap<String, Vec<crate::types::FacetCount>>,
            )>,
        >,
    >,
) -> crate::error::Result<()> {
    let mut writer = match index.writer() {
        Ok(w) => {
            tracing::info!("Write queue started for tenant {}", tenant_id);
            w
        }
        Err(e) => {
            tracing::error!("Failed to create writer for tenant {}: {}", tenant_id, e);
            return Err(e);
        }
    };

    // Merge segments when >30% of docs are deleted, so disk space is
    // gradually reclaimed without aggressive write amplification.
    let mut merge_policy = tantivy::merge_policy::LogMergePolicy::default();
    merge_policy.set_del_docs_ratio_before_merge(0.3);
    writer.set_merge_policy(Box::new(merge_policy));
    let mut pending = Vec::new();
    let mut deadline = Instant::now() + Duration::from_millis(100);

    loop {
        tracing::warn!(
            "[WQ {}] waiting, pending={}, deadline_in={}ms",
            tenant_id,
            pending.len(),
            deadline
                .saturating_duration_since(Instant::now())
                .as_millis()
        );
        match timeout_at(deadline.into(), rx.recv()).await {
            Ok(Some(op)) => {
                let action_count = op.actions.len();
                let is_compact = matches!(op.actions.first(), Some(WriteAction::Compact));
                tracing::warn!(
                    "[WQ {}] received op task={} actions={}{}",
                    tenant_id,
                    op.task_id,
                    action_count,
                    if is_compact { " (compact)" } else { "" }
                );

                if is_compact {
                    // Flush any pending writes first
                    if !pending.is_empty() {
                        commit_batch(
                            &index,
                            &tasks,
                            &mut pending,
                            &mut writer,
                            &tenant_id,
                            &base_path,
                            &oplog,
                            &facet_cache,
                        )
                        .await?;
                    }
                    compact_segments(&index, &tasks, &op.task_id, &mut writer, &tenant_id)?;
                    deadline = Instant::now() + Duration::from_millis(100);
                    continue;
                }

                pending.push(op);
                if pending.len() >= 10 {
                    tracing::warn!(
                        "[WQ {}] batch threshold, committing {} ops",
                        tenant_id,
                        pending.len()
                    );
                    commit_batch(
                        &index,
                        &tasks,
                        &mut pending,
                        &mut writer,
                        &tenant_id,
                        &base_path,
                        &oplog,
                        &facet_cache,
                    )
                    .await?;
                    deadline = Instant::now() + Duration::from_millis(100);
                }
            }
            Ok(None) => {
                tracing::warn!(
                    "[WQ {}] channel closed, flushing {} pending",
                    tenant_id,
                    pending.len()
                );
                if !pending.is_empty() {
                    commit_batch(
                        &index,
                        &tasks,
                        &mut pending,
                        &mut writer,
                        &tenant_id,
                        &base_path,
                        &oplog,
                        &facet_cache,
                    )
                    .await?;
                }
                break;
            }
            Err(_timeout) => {
                tracing::warn!(
                    "[WQ {}] timeout, flushing {} pending",
                    tenant_id,
                    pending.len()
                );
                if !pending.is_empty() {
                    commit_batch(
                        &index,
                        &tasks,
                        &mut pending,
                        &mut writer,
                        &tenant_id,
                        &base_path,
                        &oplog,
                        &facet_cache,
                    )
                    .await?;
                }
                deadline = Instant::now() + Duration::from_millis(100);
            }
        }
    }
    Ok(())
}

async fn commit_batch(
    index: &Arc<crate::index::Index>,
    tasks: &Arc<dashmap::DashMap<String, TaskInfo>>,
    ops: &mut Vec<WriteOp>,
    writer: &mut crate::index::ManagedIndexWriter,
    tenant_id: &str,
    base_path: &std::path::Path,
    oplog: &Option<Arc<crate::index::oplog::OpLog>>,
    facet_cache: &Arc<
        dashmap::DashMap<
            String,
            Arc<(
                std::time::Instant,
                usize,
                std::collections::HashMap<String, Vec<crate::types::FacetCount>>,
            )>,
        >,
    >,
) -> crate::error::Result<()> {
    tracing::warn!("[WQ {}] commit_batch: {} operations", tenant_id, ops.len());

    let schema = index.inner().schema();
    let id_field = schema.get_field("_id").unwrap();

    let settings_path = base_path.join(tenant_id).join("settings.json");
    let settings = if settings_path.exists() {
        Some(crate::index::settings::IndexSettings::load(&settings_path)?)
    } else {
        None
    };

    for op in ops.drain(..) {
        tasks.alter(&op.task_id, |_, mut task| {
            task.status = TaskStatus::Processing;
            task
        });

        let mut valid_docs = Vec::new();
        let mut rejected = Vec::new();
        let mut deleted_ids: Vec<String> = Vec::new();

        for action in op.actions {
            match action {
                WriteAction::Delete(object_id) => {
                    let term = tantivy::Term::from_field_text(id_field, &object_id);
                    writer.delete_term(term);
                    deleted_ids.push(object_id);
                }
                WriteAction::Add(doc) => {
                    let doc_json = doc.to_json();
                    let estimated_size = serde_json::to_string(&doc_json)
                        .map(|s| s.len())
                        .unwrap_or(0);
                    if let Err(e) = index.memory_budget().validate_document_size(estimated_size) {
                        rejected.push(DocFailure {
                            doc_id: doc.id,
                            error: classify_error(&e),
                            message: e.to_string(),
                        });
                        continue;
                    }
                    match index.converter().to_tantivy(&doc, settings.as_ref()) {
                        Ok(tantivy_doc) => {
                            valid_docs.push((doc.id.clone(), doc_json, tantivy_doc));
                        }
                        Err(e) => {
                            rejected.push(DocFailure {
                                doc_id: doc.id,
                                error: classify_error(&e),
                                message: e.to_string(),
                            });
                        }
                    }
                }
                WriteAction::Upsert(doc) => {
                    let doc_json = doc.to_json();
                    let estimated_size = serde_json::to_string(&doc_json)
                        .map(|s| s.len())
                        .unwrap_or(0);
                    if let Err(e) = index.memory_budget().validate_document_size(estimated_size) {
                        rejected.push(DocFailure {
                            doc_id: doc.id,
                            error: classify_error(&e),
                            message: e.to_string(),
                        });
                        continue;
                    }
                    let term = tantivy::Term::from_field_text(id_field, &doc.id);
                    writer.delete_term(term);

                    match index.converter().to_tantivy(&doc, settings.as_ref()) {
                        Ok(tantivy_doc) => {
                            valid_docs.push((doc.id.clone(), doc_json, tantivy_doc));
                        }
                        Err(e) => {
                            rejected.push(DocFailure {
                                doc_id: doc.id,
                                error: classify_error(&e),
                                message: e.to_string(),
                            });
                        }
                    }
                }
                WriteAction::Compact => {
                    // Handled in the process_writes loop, should not reach here
                }
            }
        }

        let mut valid_docs_json: Vec<(String, serde_json::Value)> = Vec::new();
        for (doc_id, doc, tantivy_doc) in &valid_docs {
            writer.add_document(tantivy_doc.clone())?;
            valid_docs_json.push((doc_id.clone(), doc.clone()));
        }

        if let Some(ref ol) = oplog {
            let mut batch_ops: Vec<(String, serde_json::Value)> = Vec::new();
            for (doc_id, doc) in &valid_docs_json {
                batch_ops.push((
                    "upsert".into(),
                    serde_json::json!({"objectID": doc_id, "body": doc}),
                ));
            }
            for did in &deleted_ids {
                batch_ops.push(("delete".into(), serde_json::json!({"objectID": did})));
            }
            if !batch_ops.is_empty() {
                if let Err(e) = ol.append_batch(&batch_ops) {
                    tracing::error!("[WQ {}] oplog append failed: {}", tenant_id, e);
                }
            }
        }

        tracing::info!(
            "[WQ {}] committing {} adds, {} deletes, {} rejected",
            tenant_id,
            valid_docs.len(),
            deleted_ids.len(),
            rejected.len()
        );
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| writer.commit())) {
            Ok(Ok(_opstamp)) => {}
            Ok(Err(e)) => {
                tracing::error!("[WQ {}] commit error: {}", tenant_id, e);
                return Err(e.into());
            }
            Err(panic_info) => {
                let msg = if let Some(s) = panic_info.downcast_ref::<String>() {
                    s.clone()
                } else if let Some(s) = panic_info.downcast_ref::<&str>() {
                    s.to_string()
                } else {
                    "unknown panic in tantivy commit".to_string()
                };
                tracing::error!("[WQ {}] PANIC during commit: {}", tenant_id, msg);
                return Err(crate::error::FlapjackError::Tantivy(msg));
            }
        }
        index.reader().reload()?;
        index.invalidate_searchable_paths_cache();
        facet_cache.retain(|k, _| !k.starts_with(&format!("{}:", tenant_id)));

        if let Some(ref ol) = oplog {
            let seq = ol.current_seq();
            let sidecar_path = base_path.join(tenant_id).join("committed_seq");
            if let Err(e) = std::fs::write(&sidecar_path, seq.to_string()) {
                tracing::error!("[WQ {}] failed to write committed_seq: {}", tenant_id, e);
            }
            let retention = std::env::var("FLAPJACK_OPLOG_RETENTION")
                .ok()
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(1000);
            if seq > retention {
                let _ = ol.truncate_before(seq - retention);
            }

            // TODO: Replication moved to HTTP layer to avoid circular dependency
            // Replication is now triggered from handlers after successful write operations
        }

        let numeric_id = if let Some(task_ref) = tasks.get(&op.task_id) {
            task_ref.numeric_id.to_string()
        } else {
            op.task_id.clone()
        };

        let total_rejected = rejected.len();
        rejected.truncate(100);
        let rejected_final = rejected.clone();

        tasks.alter(&op.task_id, |_, mut task| {
            task.status = TaskStatus::Succeeded;
            task.indexed_documents = valid_docs.len() + deleted_ids.len();
            task.rejected_documents = rejected_final.clone();
            task.rejected_count = total_rejected;
            task
        });
        tasks.alter(&numeric_id, |_, mut task| {
            task.status = TaskStatus::Succeeded;
            task.indexed_documents = valid_docs.len() + deleted_ids.len();
            task.rejected_documents = rejected_final.clone();
            task.rejected_count = total_rejected;
            task
        });
    }

    Ok(())
}

/// Force-merge all segments into one and garbage-collect stale files.
fn compact_segments(
    index: &Arc<crate::index::Index>,
    tasks: &Arc<dashmap::DashMap<String, TaskInfo>>,
    task_id: &str,
    writer: &mut crate::index::ManagedIndexWriter,
    tenant_id: &str,
) -> crate::error::Result<()> {
    tasks.alter(task_id, |_, mut t| {
        t.status = TaskStatus::Processing;
        t
    });

    let segment_ids = index.inner().searchable_segment_ids()?;
    tracing::info!(
        "[WQ {}] compacting {} segments",
        tenant_id,
        segment_ids.len()
    );

    let result: crate::error::Result<()> = (|| {
        if segment_ids.len() > 1 {
            let merge_future = writer.merge(&segment_ids);
            // Block on the merge (runs in Tantivy's merge thread pool).
            // wait() returns Option<SegmentMeta>; None means all docs were deleted.
            if let Err(e) = merge_future.wait() {
                tracing::error!("[WQ {}] merge failed: {}", tenant_id, e);
                return Err(crate::error::FlapjackError::Tantivy(e.to_string()));
            }
        }

        // Clean up orphaned segment files left by completed merges
        let gc_result = writer
            .garbage_collect_files()
            .wait()
            .map_err(|e| crate::error::FlapjackError::Tantivy(e.to_string()))?;
        tracing::info!(
            "[WQ {}] compact done, gc removed {} files",
            tenant_id,
            gc_result.deleted_files.len()
        );

        index.reader().reload()?;
        index.invalidate_searchable_paths_cache();
        Ok(())
    })();

    let numeric_id = if let Some(task_ref) = tasks.get(task_id) {
        task_ref.numeric_id.to_string()
    } else {
        task_id.to_string()
    };

    let status = match &result {
        Ok(()) => TaskStatus::Succeeded,
        Err(e) => TaskStatus::Failed(e.to_string()),
    };
    tasks.alter(task_id, |_, mut t| {
        t.status = status.clone();
        t
    });
    tasks.alter(&numeric_id, |_, mut t| {
        t.status = status;
        t
    });

    result
}

fn classify_error(e: &crate::error::FlapjackError) -> String {
    match e {
        crate::error::FlapjackError::FieldNotFound(_) => "field_not_found".to_string(),
        crate::error::FlapjackError::TypeMismatch { .. } => "type_mismatch".to_string(),
        crate::error::FlapjackError::MissingField(_) => "missing_field".to_string(),
        crate::error::FlapjackError::DocumentTooLarge { .. } => "document_too_large".to_string(),
        _ => "validation_error".to_string(),
    }
}
