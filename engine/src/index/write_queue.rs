//! Async write queue with hybrid batching for Flapjack.

use crate::types::{DocFailure, Document, TaskInfo, TaskStatus};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio::time::timeout_at;

/// Vector search context for the write queue.
/// When `vector-search` feature is disabled, this is a zero-sized type.
pub(crate) struct VectorWriteContext {
    #[cfg(feature = "vector-search")]
    pub vector_indices:
        Arc<dashmap::DashMap<String, Arc<std::sync::RwLock<crate::vector::index::VectorIndex>>>>,
}

impl VectorWriteContext {
    #[cfg(feature = "vector-search")]
    pub fn new(
        vector_indices: Arc<
            dashmap::DashMap<String, Arc<std::sync::RwLock<crate::vector::index::VectorIndex>>>,
        >,
    ) -> Self {
        Self { vector_indices }
    }

    #[cfg(not(feature = "vector-search"))]
    pub fn new() -> Self {
        Self {}
    }
}

pub enum WriteAction {
    Add(Document),
    Upsert(Document),
    /// Like Upsert but skips lww_map update — used by apply_ops_to_manager which
    /// has already recorded the correct op timestamp in lww_map before queuing.
    UpsertNoLwwUpdate(Document),
    Delete(String),
    /// Like Delete but skips lww_map update — same rationale as UpsertNoLwwUpdate.
    DeleteNoLwwUpdate(String),
    Compact,
}

pub struct WriteOp {
    pub task_id: String,
    pub actions: Vec<WriteAction>,
}

pub type WriteQueue = mpsc::Sender<WriteOp>;

pub(crate) fn create_write_queue(
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
    lww_map: Arc<dashmap::DashMap<String, dashmap::DashMap<String, (u64, String)>>>,
    vector_ctx: VectorWriteContext,
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
            lww_map,
            vector_ctx,
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
    lww_map: Arc<dashmap::DashMap<String, dashmap::DashMap<String, (u64, String)>>>,
    vector_ctx: VectorWriteContext,
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
        if pending.is_empty() {
            tracing::trace!(
                "[WQ {}] idle, deadline_in={}ms",
                tenant_id,
                deadline
                    .saturating_duration_since(Instant::now())
                    .as_millis()
            );
        } else {
            tracing::debug!(
                "[WQ {}] waiting, pending={}, deadline_in={}ms",
                tenant_id,
                pending.len(),
                deadline
                    .saturating_duration_since(Instant::now())
                    .as_millis()
            );
        }
        match timeout_at(deadline.into(), rx.recv()).await {
            Ok(Some(op)) => {
                let action_count = op.actions.len();
                let is_compact = matches!(op.actions.first(), Some(WriteAction::Compact));
                tracing::debug!(
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
                            &lww_map,
                            &vector_ctx,
                        )
                        .await?;
                    }
                    compact_segments(&index, &tasks, &op.task_id, &mut writer, &tenant_id)?;
                    deadline = Instant::now() + Duration::from_millis(100);
                    continue;
                }

                pending.push(op);
                if pending.len() >= 10 {
                    tracing::debug!(
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
                        &lww_map,
                        &vector_ctx,
                    )
                    .await?;
                    deadline = Instant::now() + Duration::from_millis(100);
                }
            }
            Ok(None) => {
                tracing::info!(
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
                        &lww_map,
                        &vector_ctx,
                    )
                    .await?;
                }
                break;
            }
            Err(_timeout) => {
                if !pending.is_empty() {
                    tracing::debug!(
                        "[WQ {}] timeout, flushing {} pending",
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
                        &lww_map,
                        &vector_ctx,
                    )
                    .await?;
                }
                deadline = Instant::now() + Duration::from_millis(100);
            }
        }
    }
    Ok(())
}

/// Extract, validate, and strip `_vectors` from a document before Tantivy conversion.
/// Returns Ok(cleaned vectors) or Err(rejection failure).
/// Strips `_vectors` from `doc.fields` so Tantivy doesn't index large float arrays.
#[cfg(feature = "vector-search")]
fn process_doc_vectors(
    doc: &mut Document,
    doc_json: &serde_json::Value,
    embedder_configs: &[(String, crate::vector::config::EmbedderConfig)],
) -> Result<Option<std::collections::HashMap<String, Vec<f32>>>, DocFailure> {
    use crate::vector::vectors_field::{extract_vectors, strip_vectors_from_document};

    let extracted = match extract_vectors(doc_json) {
        Ok(vecs) => vecs,
        Err(e) => {
            return Err(DocFailure {
                doc_id: doc.id.clone(),
                error: "invalid_vectors".to_string(),
                message: e.to_string(),
            });
        }
    };

    let clean_vectors = if let Some(map) = extracted {
        let mut clean = std::collections::HashMap::new();
        for (emb_name, result) in map {
            // Only validate vectors for configured embedders
            if let Some((_, cfg)) = embedder_configs.iter().find(|(n, _)| n == &emb_name) {
                match result {
                    Err(e) => {
                        return Err(DocFailure {
                            doc_id: doc.id.clone(),
                            error: "invalid_vectors".to_string(),
                            message: format!("embedder '{}': {}", emb_name, e),
                        });
                    }
                    Ok(vec) => {
                        if let Some(expected) = cfg.dimensions {
                            if vec.len() != expected {
                                return Err(DocFailure {
                                    doc_id: doc.id.clone(),
                                    error: "dimension_mismatch".to_string(),
                                    message: format!(
                                        "embedder '{}': expected {} dimensions, got {}",
                                        emb_name,
                                        expected,
                                        vec.len()
                                    ),
                                });
                            }
                        }
                        clean.insert(emb_name, vec);
                    }
                }
            }
            // Vectors for unconfigured embedders are silently ignored
        }
        if clean.is_empty() {
            None
        } else {
            Some(clean)
        }
    } else {
        None
    };

    // Strip _vectors from doc.fields BEFORE to_tantivy
    strip_vectors_from_document(doc);

    Ok(clean_vectors)
}

#[allow(unused_mut, unused_variables)]
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
    lww_map: &Arc<dashmap::DashMap<String, dashmap::DashMap<String, (u64, String)>>>,
    vector_ctx: &VectorWriteContext,
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

    // Pre-parse embedder configs from settings (used for _vectors validation and embedding).
    #[cfg(feature = "vector-search")]
    let embedder_configs: Vec<(String, crate::vector::config::EmbedderConfig)> = settings
        .as_ref()
        .and_then(|s| s.embedders.as_ref())
        .map(|emb_map| {
            emb_map
                .iter()
                .filter_map(|(name, json)| {
                    if json.is_null() {
                        return None;
                    }
                    match serde_json::from_value::<crate::vector::config::EmbedderConfig>(
                        json.clone(),
                    ) {
                        Ok(config) => Some((name.clone(), config)),
                        Err(e) => {
                            tracing::warn!(
                                "[WQ {}] failed to parse embedder '{}': {}",
                                tenant_id,
                                name,
                                e
                            );
                            None
                        }
                    }
                })
                .collect()
        })
        .unwrap_or_default();

    for op in ops.drain(..) {
        tasks.alter(&op.task_id, |_, mut task| {
            task.status = TaskStatus::Processing;
            task
        });

        let mut valid_docs = Vec::new();
        let mut rejected = Vec::new();
        let mut deleted_ids: Vec<String> = Vec::new();
        // Track which writes are primary (should update lww_map) vs replicated (already tracked).
        let mut primary_upsert_ids: Vec<String> = Vec::new();
        let mut primary_delete_ids: Vec<String> = Vec::new();
        // Track extracted vectors parallel to valid_docs (one entry per valid doc).
        #[cfg(feature = "vector-search")]
        let mut doc_vectors: Vec<Option<std::collections::HashMap<String, Vec<f32>>>> = Vec::new();
        #[cfg(feature = "vector-search")]
        let mut vectors_modified = false;

        for action in op.actions {
            match action {
                WriteAction::Delete(object_id) => {
                    let term = tantivy::Term::from_field_text(id_field, &object_id);
                    writer.delete_term(term);
                    primary_delete_ids.push(object_id.clone());
                    deleted_ids.push(object_id);
                }
                WriteAction::DeleteNoLwwUpdate(object_id) => {
                    // Replicated delete: lww already recorded by apply_ops_to_manager.
                    let term = tantivy::Term::from_field_text(id_field, &object_id);
                    writer.delete_term(term);
                    deleted_ids.push(object_id);
                }
                WriteAction::Add(mut doc) => {
                    let doc_json = doc.to_json();
                    #[cfg(feature = "vector-search")]
                    let vectors = match process_doc_vectors(&mut doc, &doc_json, &embedder_configs)
                    {
                        Ok(v) => v,
                        Err(failure) => {
                            rejected.push(failure);
                            continue;
                        }
                    };
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
                            primary_upsert_ids.push(doc.id.clone());
                            valid_docs.push((doc.id.clone(), doc_json, tantivy_doc));
                            #[cfg(feature = "vector-search")]
                            doc_vectors.push(vectors);
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
                WriteAction::Upsert(mut doc) => {
                    let doc_json = doc.to_json();
                    #[cfg(feature = "vector-search")]
                    let vectors = match process_doc_vectors(&mut doc, &doc_json, &embedder_configs)
                    {
                        Ok(v) => v,
                        Err(failure) => {
                            rejected.push(failure);
                            continue;
                        }
                    };
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
                            primary_upsert_ids.push(doc.id.clone());
                            valid_docs.push((doc.id.clone(), doc_json, tantivy_doc));
                            #[cfg(feature = "vector-search")]
                            doc_vectors.push(vectors);
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
                WriteAction::UpsertNoLwwUpdate(mut doc) => {
                    // Replicated upsert: lww already recorded by apply_ops_to_manager.
                    let doc_json = doc.to_json();
                    #[cfg(feature = "vector-search")]
                    let vectors = match process_doc_vectors(&mut doc, &doc_json, &embedder_configs)
                    {
                        Ok(v) => v,
                        Err(failure) => {
                            rejected.push(failure);
                            continue;
                        }
                    };
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
                            #[cfg(feature = "vector-search")]
                            doc_vectors.push(vectors);
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

        // Phase 2+3: Embed documents and update VectorIndex.
        #[cfg(feature = "vector-search")]
        if !embedder_configs.is_empty() {
            use crate::vector::config::EmbedderSource;

            // Collect computed vectors for oplog injection: doc_id → embedder_name → vector.
            let mut computed_vectors: std::collections::HashMap<
                String,
                std::collections::HashMap<String, Vec<f32>>,
            > = std::collections::HashMap::new();

            for (embedder_name, config) in &embedder_configs {
                // Separate docs with user-provided vectors from those needing embedding.
                let mut vectors_to_store: Vec<(String, Vec<f32>)> = Vec::new();
                let mut docs_needing_embed: Vec<(String, String)> = Vec::new(); // (doc_id, rendered_text)
                let template = config.document_template();

                for (i, (doc_id, doc_json, _tantivy_doc)) in valid_docs.iter().enumerate() {
                    if let Some(ref user_vecs) = doc_vectors[i] {
                        if let Some(vec) = user_vecs.get(embedder_name) {
                            // User provided a vector for this embedder — use directly.
                            vectors_to_store.push((doc_id.clone(), vec.clone()));
                            continue;
                        }
                    }
                    // No user-provided vector for this embedder.
                    if config.source == EmbedderSource::UserProvided {
                        // UserProvided can't generate embeddings — skip.
                        continue;
                    }
                    // Render through document template for embedding.
                    let text = template.render(doc_json);
                    docs_needing_embed.push((doc_id.clone(), text));
                }

                // Embed documents that need it.
                if !docs_needing_embed.is_empty() {
                    let embedder = match crate::vector::embedder::create_embedder(config) {
                        Ok(e) => e,
                        Err(e) => {
                            tracing::warn!(
                                "[WQ {}] failed to create embedder '{}': {}",
                                tenant_id,
                                embedder_name,
                                e
                            );
                            continue;
                        }
                    };

                    let texts: Vec<&str> =
                        docs_needing_embed.iter().map(|(_, t)| t.as_str()).collect();

                    // Sub-batch into groups of 50 if >100 texts.
                    let embeddings = if texts.len() > 100 {
                        let mut all_vecs = Vec::new();
                        let mut failed = false;
                        for chunk in texts.chunks(50) {
                            match embedder.embed_documents(chunk).await {
                                Ok(batch) => all_vecs.extend(batch),
                                Err(e) => {
                                    tracing::warn!(
                                        "[WQ {}] embedding sub-batch failed for '{}': {}",
                                        tenant_id,
                                        embedder_name,
                                        e
                                    );
                                    failed = true;
                                    break;
                                }
                            }
                        }
                        if failed {
                            continue; // Skip vector update for this embedder
                        }
                        Ok(all_vecs)
                    } else {
                        embedder.embed_documents(&texts).await
                    };

                    match embeddings {
                        Ok(vecs) => {
                            for ((doc_id, _), vec) in
                                docs_needing_embed.iter().zip(vecs.into_iter())
                            {
                                vectors_to_store.push((doc_id.clone(), vec));
                            }
                        }
                        Err(e) => {
                            tracing::warn!(
                                "[WQ {}] embedding failed for '{}': {}",
                                tenant_id,
                                embedder_name,
                                e
                            );
                            // Don't block Tantivy — skip vector update for this embedder.
                            continue;
                        }
                    }
                }

                // Phase 3: Update VectorIndex.
                if !vectors_to_store.is_empty() {
                    // Record vectors for oplog injection before consuming them.
                    for (doc_id, vec) in &vectors_to_store {
                        computed_vectors
                            .entry(doc_id.clone())
                            .or_default()
                            .insert(embedder_name.clone(), vec.clone());
                    }

                    let first_dim = vectors_to_store[0].1.len();
                    let vi = get_or_create_vector_index(
                        &vector_ctx.vector_indices,
                        tenant_id,
                        first_dim,
                    );
                    // Bind the Result so it's dropped before `vi` (reverse decl order).
                    let write_result = vi.write();
                    if let Ok(mut guard) = write_result {
                        for (doc_id, vec) in &vectors_to_store {
                            match guard.add(doc_id, vec) {
                                Ok(()) => vectors_modified = true,
                                Err(e) => tracing::warn!(
                                    "[WQ {}] failed to add vector for '{}': {}",
                                    tenant_id,
                                    doc_id,
                                    e
                                ),
                            }
                        }
                    } else {
                        tracing::error!(
                            "[WQ {}] VectorIndex write lock poisoned for embedder '{}'",
                            tenant_id,
                            embedder_name
                        );
                    }
                }
            }

            // Handle deletes in VectorIndex.
            if !deleted_ids.is_empty() {
                if let Some(vi_ref) = vector_ctx.vector_indices.get(tenant_id) {
                    let write_result = vi_ref.write();
                    if let Ok(mut guard) = write_result {
                        for did in &deleted_ids {
                            if guard.remove(did).is_ok() {
                                vectors_modified = true;
                            }
                        }
                    } else {
                        tracing::error!(
                            "[WQ {}] VectorIndex write lock poisoned for delete",
                            tenant_id
                        );
                    }
                }
            }

            // Inject all vectors (user-provided + API-generated) into doc_json for oplog
            // persistence. insert() overwrites any existing _vectors entries, ensuring
            // recovery can rebuild without calling external APIs.
            if !computed_vectors.is_empty() {
                for (doc_id, doc_json, _) in &mut valid_docs {
                    if let Some(embedder_vecs) = computed_vectors.get(doc_id.as_str()) {
                        let vectors_obj = doc_json
                            .as_object_mut()
                            .unwrap()
                            .entry("_vectors")
                            .or_insert_with(|| serde_json::json!({}));
                        if let Some(obj) = vectors_obj.as_object_mut() {
                            for (emb_name, vec) in embedder_vecs {
                                let json_vec: Vec<serde_json::Value> = vec
                                    .iter()
                                    .map(|&v| serde_json::Value::from(v as f64))
                                    .collect();
                                obj.insert(emb_name.clone(), serde_json::Value::Array(json_vec));
                            }
                        }
                    }
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

        // Save VectorIndex and fingerprint to disk after successful Tantivy commit.
        #[cfg(feature = "vector-search")]
        if vectors_modified {
            let vectors_dir = base_path.join(tenant_id).join("vectors");
            if let Some(vi_ref) = vector_ctx.vector_indices.get(tenant_id) {
                if let Ok(guard) = vi_ref.read() {
                    if let Err(e) = guard.save(&vectors_dir) {
                        tracing::error!("[WQ {}] failed to save vector index: {}", tenant_id, e);
                    } else if !embedder_configs.is_empty() {
                        let fp = crate::vector::config::EmbedderFingerprint::from_configs(
                            &embedder_configs,
                            guard.dimensions(),
                        );
                        if let Err(e) = fp.save(&vectors_dir) {
                            tracing::error!(
                                "[WQ {}] failed to save embedder fingerprint: {}",
                                tenant_id,
                                e
                            );
                        }
                    }
                }
            }
        }

        // Update LWW map for primary writes (Upsert/Delete/Add) only.
        // Replicated writes (UpsertNoLwwUpdate/DeleteNoLwwUpdate) skip this block because
        // apply_ops_to_manager already recorded the correct op timestamp in lww_map.
        if !primary_upsert_ids.is_empty() || !primary_delete_ids.is_empty() {
            let now_ts = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64;
            let node_id =
                std::env::var("FLAPJACK_NODE_ID").unwrap_or_else(|_| "unknown".to_string());
            let tenant_map = lww_map
                .entry(tenant_id.to_string())
                .or_insert_with(dashmap::DashMap::new);
            for doc_id in &primary_upsert_ids {
                tenant_map.insert(doc_id.clone(), (now_ts, node_id.clone()));
            }
            for did in &primary_delete_ids {
                tenant_map.insert(did.clone(), (now_ts, node_id.clone()));
            }
        }

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

/// Get or create a VectorIndex for a tenant. Uses actual vector length for dimensions.
/// If the entry already exists in the DashMap, returns it. Otherwise creates a new one.
#[cfg(feature = "vector-search")]
fn get_or_create_vector_index(
    vector_indices: &dashmap::DashMap<
        String,
        Arc<std::sync::RwLock<crate::vector::index::VectorIndex>>,
    >,
    tenant_id: &str,
    dimensions: usize,
) -> Arc<std::sync::RwLock<crate::vector::index::VectorIndex>> {
    if let Some(existing) = vector_indices.get(tenant_id) {
        return Arc::clone(&existing);
    }
    let vi = crate::vector::index::VectorIndex::new(dimensions, usearch::ffi::MetricKind::Cos)
        .expect("failed to create VectorIndex");
    let arc = Arc::new(std::sync::RwLock::new(vi));
    vector_indices.insert(tenant_id.to_string(), Arc::clone(&arc));
    arc
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// Helper to create a write queue with all dependencies for testing.
    fn setup_write_queue(
        tmp: &tempfile::TempDir,
        tenant_id: &str,
    ) -> (
        WriteQueue,
        tokio::task::JoinHandle<crate::error::Result<()>>,
        Arc<dashmap::DashMap<String, TaskInfo>>,
    ) {
        let tenant_path = tmp.path().join(tenant_id);
        std::fs::create_dir_all(&tenant_path).unwrap();
        let schema = crate::index::schema::Schema::builder().build();
        let index = Arc::new(crate::index::Index::create(&tenant_path, schema).unwrap());

        let writers = Arc::new(dashmap::DashMap::new());
        let tasks: Arc<dashmap::DashMap<String, TaskInfo>> = Arc::new(dashmap::DashMap::new());
        let facet_cache = Arc::new(dashmap::DashMap::new());
        let lww_map = Arc::new(dashmap::DashMap::new());

        #[cfg(feature = "vector-search")]
        let vector_ctx = VectorWriteContext::new(Arc::new(dashmap::DashMap::new()));
        #[cfg(not(feature = "vector-search"))]
        let vector_ctx = VectorWriteContext::new();

        let (tx, handle) = create_write_queue(
            tenant_id.to_string(),
            index,
            writers,
            Arc::clone(&tasks),
            tmp.path().to_path_buf(),
            None,
            facet_cache,
            lww_map,
            vector_ctx,
        );

        (tx, handle, tasks)
    }

    #[tokio::test]
    async fn test_commit_batch_basic_add() {
        let tmp = tempfile::TempDir::new().unwrap();
        let (tx, handle, tasks) = setup_write_queue(&tmp, "test_tenant");

        let task_id = "test_task_1".to_string();
        let task = TaskInfo::new(task_id.clone(), 1, 2);
        tasks.insert(task_id.clone(), task);

        let doc1 = crate::types::Document {
            id: "doc1".to_string(),
            fields: HashMap::from([(
                "name".to_string(),
                crate::types::FieldValue::Text("Alice".to_string()),
            )]),
        };
        let doc2 = crate::types::Document {
            id: "doc2".to_string(),
            fields: HashMap::from([(
                "name".to_string(),
                crate::types::FieldValue::Text("Bob".to_string()),
            )]),
        };

        tx.send(WriteOp {
            task_id: task_id.clone(),
            actions: vec![WriteAction::Add(doc1), WriteAction::Add(doc2)],
        })
        .await
        .unwrap();

        drop(tx);
        handle.await.unwrap().unwrap();

        let final_task = tasks.get(&task_id).unwrap();
        assert!(
            matches!(final_task.status, crate::types::TaskStatus::Succeeded),
            "task should succeed, got: {:?}",
            final_task.status
        );
        assert_eq!(final_task.indexed_documents, 2);
    }

    #[tokio::test]
    async fn test_commit_batch_upsert() {
        let tmp = tempfile::TempDir::new().unwrap();
        let (tx, handle, tasks) = setup_write_queue(&tmp, "upsert_tenant");

        // Add a document first
        let task_id_1 = "upsert_task_1".to_string();
        tasks.insert(task_id_1.clone(), TaskInfo::new(task_id_1.clone(), 1, 1));
        let doc = crate::types::Document {
            id: "doc1".to_string(),
            fields: HashMap::from([(
                "name".to_string(),
                crate::types::FieldValue::Text("Alice".to_string()),
            )]),
        };
        tx.send(WriteOp {
            task_id: task_id_1.clone(),
            actions: vec![WriteAction::Add(doc)],
        })
        .await
        .unwrap();

        // Give the write queue time to process
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

        // Upsert the same doc with updated content
        let task_id_2 = "upsert_task_2".to_string();
        tasks.insert(task_id_2.clone(), TaskInfo::new(task_id_2.clone(), 2, 1));
        let doc_updated = crate::types::Document {
            id: "doc1".to_string(),
            fields: HashMap::from([(
                "name".to_string(),
                crate::types::FieldValue::Text("Alice Updated".to_string()),
            )]),
        };
        tx.send(WriteOp {
            task_id: task_id_2.clone(),
            actions: vec![WriteAction::Upsert(doc_updated)],
        })
        .await
        .unwrap();

        drop(tx);
        handle.await.unwrap().unwrap();

        let final_task = tasks.get(&task_id_2).unwrap();
        assert!(
            matches!(final_task.status, crate::types::TaskStatus::Succeeded),
            "upsert task should succeed, got: {:?}",
            final_task.status
        );
        assert_eq!(final_task.indexed_documents, 1);
    }

    #[tokio::test]
    async fn test_commit_batch_delete() {
        let tmp = tempfile::TempDir::new().unwrap();
        let (tx, handle, tasks) = setup_write_queue(&tmp, "delete_tenant");

        // Add a document first
        let task_id_1 = "del_task_1".to_string();
        tasks.insert(task_id_1.clone(), TaskInfo::new(task_id_1.clone(), 1, 1));
        let doc = crate::types::Document {
            id: "doc1".to_string(),
            fields: HashMap::from([(
                "name".to_string(),
                crate::types::FieldValue::Text("Alice".to_string()),
            )]),
        };
        tx.send(WriteOp {
            task_id: task_id_1.clone(),
            actions: vec![WriteAction::Add(doc)],
        })
        .await
        .unwrap();

        // Give the write queue time to process
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

        // Delete the doc
        let task_id_2 = "del_task_2".to_string();
        tasks.insert(task_id_2.clone(), TaskInfo::new(task_id_2.clone(), 2, 1));
        tx.send(WriteOp {
            task_id: task_id_2.clone(),
            actions: vec![WriteAction::Delete("doc1".to_string())],
        })
        .await
        .unwrap();

        drop(tx);
        handle.await.unwrap().unwrap();

        let final_task = tasks.get(&task_id_2).unwrap();
        assert!(
            matches!(final_task.status, crate::types::TaskStatus::Succeeded),
            "delete task should succeed, got: {:?}",
            final_task.status
        );
        // Delete counts as 1 indexed document (it's a successful write operation)
        assert_eq!(final_task.indexed_documents, 1);
    }

    #[cfg(feature = "vector-search")]
    #[tokio::test]
    async fn test_vector_write_context_shares_dashmap() {
        // Verify that VectorWriteContext properly shares the same DashMap instance
        let vector_indices: Arc<
            dashmap::DashMap<String, Arc<std::sync::RwLock<crate::vector::index::VectorIndex>>>,
        > = Arc::new(dashmap::DashMap::new());

        let ctx = VectorWriteContext::new(Arc::clone(&vector_indices));

        // Insert into the shared DashMap
        let vi = crate::vector::index::VectorIndex::new(3, usearch::ffi::MetricKind::Cos).unwrap();
        vector_indices.insert(
            "test_tenant".to_string(),
            Arc::new(std::sync::RwLock::new(vi)),
        );

        // The context should see the same data (same Arc)
        assert!(ctx.vector_indices.contains_key("test_tenant"));
        assert_eq!(ctx.vector_indices.len(), 1);
    }

    #[cfg(feature = "vector-search")]
    #[tokio::test]
    async fn test_create_write_queue_with_vector_indices() {
        let tmp = tempfile::TempDir::new().unwrap();
        let tenant_id = "vec_tenant";
        let tenant_path = tmp.path().join(tenant_id);
        std::fs::create_dir_all(&tenant_path).unwrap();
        let schema = crate::index::schema::Schema::builder().build();
        let index = Arc::new(crate::index::Index::create(&tenant_path, schema).unwrap());

        let writers = Arc::new(dashmap::DashMap::new());
        let tasks: Arc<dashmap::DashMap<String, TaskInfo>> = Arc::new(dashmap::DashMap::new());
        let facet_cache = Arc::new(dashmap::DashMap::new());
        let lww_map = Arc::new(dashmap::DashMap::new());
        let vector_indices: Arc<
            dashmap::DashMap<String, Arc<std::sync::RwLock<crate::vector::index::VectorIndex>>>,
        > = Arc::new(dashmap::DashMap::new());

        let vector_ctx = VectorWriteContext::new(vector_indices);

        let (tx, handle) = create_write_queue(
            tenant_id.to_string(),
            index,
            writers,
            Arc::clone(&tasks),
            tmp.path().to_path_buf(),
            None,
            facet_cache,
            lww_map,
            vector_ctx,
        );

        let task_id = "vec_task_1".to_string();
        let task = TaskInfo::new(task_id.clone(), 1, 1);
        tasks.insert(task_id.clone(), task);

        let doc = crate::types::Document {
            id: "doc1".to_string(),
            fields: HashMap::from([(
                "title".to_string(),
                crate::types::FieldValue::Text("Hello vectors".to_string()),
            )]),
        };

        tx.send(WriteOp {
            task_id: task_id.clone(),
            actions: vec![WriteAction::Add(doc)],
        })
        .await
        .unwrap();

        drop(tx);
        handle.await.unwrap().unwrap();

        let final_task = tasks.get(&task_id).unwrap();
        assert!(
            matches!(final_task.status, crate::types::TaskStatus::Succeeded),
            "task should succeed with vector_indices plumbing, got: {:?}",
            final_task.status
        );
        assert_eq!(final_task.indexed_documents, 1);
    }

    // ── Auto-embedding integration tests (7.11) ──

    #[cfg(feature = "vector-search")]
    mod auto_embed_tests {
        use super::*;
        use crate::types::FieldValue;
        use wiremock::matchers::method;
        use wiremock::{Mock, MockServer, ResponseTemplate};

        type VectorIndicesMap = Arc<
            dashmap::DashMap<String, Arc<std::sync::RwLock<crate::vector::index::VectorIndex>>>,
        >;

        /// Helper to create a write queue with embedder settings and shared vector indices.
        fn setup_write_queue_with_embedder(
            tmp: &tempfile::TempDir,
            tenant_id: &str,
            embedder_settings: Option<HashMap<String, serde_json::Value>>,
        ) -> (
            WriteQueue,
            tokio::task::JoinHandle<crate::error::Result<()>>,
            Arc<dashmap::DashMap<String, TaskInfo>>,
            VectorIndicesMap,
        ) {
            let tenant_path = tmp.path().join(tenant_id);
            std::fs::create_dir_all(&tenant_path).unwrap();

            // Write settings with embedder config
            let settings = crate::index::settings::IndexSettings {
                embedders: embedder_settings,
                ..Default::default()
            };
            let settings_json = serde_json::to_string_pretty(&settings).unwrap();
            std::fs::write(tenant_path.join("settings.json"), settings_json).unwrap();

            let schema = crate::index::schema::Schema::builder().build();
            let index = Arc::new(crate::index::Index::create(&tenant_path, schema).unwrap());

            let writers = Arc::new(dashmap::DashMap::new());
            let tasks: Arc<dashmap::DashMap<String, TaskInfo>> = Arc::new(dashmap::DashMap::new());
            let facet_cache = Arc::new(dashmap::DashMap::new());
            let lww_map = Arc::new(dashmap::DashMap::new());
            let vector_indices: VectorIndicesMap = Arc::new(dashmap::DashMap::new());

            let vector_ctx = VectorWriteContext::new(Arc::clone(&vector_indices));

            let (tx, handle) = create_write_queue(
                tenant_id.to_string(),
                index,
                writers,
                Arc::clone(&tasks),
                tmp.path().to_path_buf(),
                None,
                facet_cache,
                lww_map,
                vector_ctx,
            );

            (tx, handle, tasks, vector_indices)
        }

        /// Create REST embedder config JSON (single-input template).
        fn rest_embedder_config(server_uri: &str, dimensions: usize) -> serde_json::Value {
            serde_json::json!({
                "source": "rest",
                "url": format!("{}/embed", server_uri),
                "request": {"input": "{{text}}"},
                "response": {"embedding": "{{embedding}}"},
                "dimensions": dimensions
            })
        }

        /// Create batch REST embedder config JSON.
        fn rest_embedder_batch_config(server_uri: &str, dimensions: usize) -> serde_json::Value {
            serde_json::json!({
                "source": "rest",
                "url": format!("{}/embed", server_uri),
                "request": {"inputs": ["{{text}}", "{{..}}"]},
                "response": {"embeddings": ["{{embedding}}", "{{..}}"]},
                "dimensions": dimensions
            })
        }

        // ── Add/Upsert tests ──

        #[tokio::test]
        async fn test_auto_embed_on_add() {
            let server = MockServer::start().await;
            Mock::given(method("POST"))
                .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "embedding": [0.1, 0.2, 0.3]
                })))
                .mount(&server)
                .await;

            let tmp = tempfile::TempDir::new().unwrap();
            let mut embedders = HashMap::new();
            embedders.insert(
                "default".to_string(),
                rest_embedder_config(&server.uri(), 3),
            );

            let (tx, handle, tasks, vector_indices) =
                setup_write_queue_with_embedder(&tmp, "embed_t", Some(embedders));

            let task_id = "embed_add_task".to_string();
            tasks.insert(task_id.clone(), TaskInfo::new(task_id.clone(), 1, 1));

            tx.send(WriteOp {
                task_id: task_id.clone(),
                actions: vec![WriteAction::Add(crate::types::Document {
                    id: "doc1".to_string(),
                    fields: HashMap::from([(
                        "title".to_string(),
                        FieldValue::Text("Hello vectors".to_string()),
                    )]),
                })],
            })
            .await
            .unwrap();

            drop(tx);
            handle.await.unwrap().unwrap();

            let final_task = tasks.get(&task_id).unwrap();
            assert!(
                matches!(final_task.status, TaskStatus::Succeeded),
                "task should succeed, got: {:?}",
                final_task.status
            );

            // Verify vector index was auto-created and has the document
            assert!(
                vector_indices.contains_key("embed_t"),
                "vector index should be auto-created"
            );
            let vi_lock = vector_indices.get("embed_t").unwrap();
            let vi = vi_lock.read().unwrap();
            assert_eq!(vi.len(), 1, "vector index should have 1 document");

            let results = vi.search(&[0.1, 0.2, 0.3], 1).unwrap();
            assert_eq!(results.len(), 1);
            assert_eq!(results[0].doc_id, "doc1");
        }

        #[tokio::test]
        async fn test_auto_embed_on_upsert_replaces_vector() {
            use wiremock::matchers::body_string_contains;

            let server = MockServer::start().await;
            // Use body content matching to return different vectors for
            // each request — deterministic, no reliance on mock ordering.
            Mock::given(method("POST"))
                .and(body_string_contains("first version"))
                .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "embedding": [1.0, 0.0, 0.0]
                })))
                .mount(&server)
                .await;
            Mock::given(method("POST"))
                .and(body_string_contains("updated version"))
                .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "embedding": [0.0, 0.0, 1.0]
                })))
                .mount(&server)
                .await;

            let tmp = tempfile::TempDir::new().unwrap();
            let mut embedders = HashMap::new();
            embedders.insert(
                "default".to_string(),
                rest_embedder_config(&server.uri(), 3),
            );

            let (tx, handle, tasks, vector_indices) =
                setup_write_queue_with_embedder(&tmp, "upsert_t", Some(embedders));

            // Add initial doc — body contains "first version" → gets [1,0,0]
            let task1 = "upsert_vec_t1".to_string();
            tasks.insert(task1.clone(), TaskInfo::new(task1.clone(), 1, 1));
            tx.send(WriteOp {
                task_id: task1.clone(),
                actions: vec![WriteAction::Add(crate::types::Document {
                    id: "doc1".to_string(),
                    fields: HashMap::from([(
                        "title".to_string(),
                        FieldValue::Text("first version".into()),
                    )]),
                })],
            })
            .await
            .unwrap();

            tokio::time::sleep(std::time::Duration::from_millis(200)).await;

            // Verify initial vector is [1,0,0]
            {
                let vi_lock = vector_indices.get("upsert_t").unwrap();
                let vi = vi_lock.read().unwrap();
                assert_eq!(vi.len(), 1);
                let results = vi.search(&[1.0, 0.0, 0.0], 1).unwrap();
                assert_eq!(results[0].doc_id, "doc1");
                assert!(
                    results[0].distance < 0.01,
                    "initial vector should be close to [1,0,0], distance={}",
                    results[0].distance
                );
            }

            // Upsert same doc — body contains "updated version" → gets [0,0,1]
            let task2 = "upsert_vec_t2".to_string();
            tasks.insert(task2.clone(), TaskInfo::new(task2.clone(), 2, 1));
            tx.send(WriteOp {
                task_id: task2.clone(),
                actions: vec![WriteAction::Upsert(crate::types::Document {
                    id: "doc1".to_string(),
                    fields: HashMap::from([(
                        "title".to_string(),
                        FieldValue::Text("updated version".into()),
                    )]),
                })],
            })
            .await
            .unwrap();

            drop(tx);
            handle.await.unwrap().unwrap();

            let vi_lock = vector_indices.get("upsert_t").unwrap();
            let vi = vi_lock.read().unwrap();
            assert_eq!(vi.len(), 1, "should still have just 1 document");

            // Vector should now be [0,0,1] — verify it actually changed
            let results = vi.search(&[0.0, 0.0, 1.0], 1).unwrap();
            assert_eq!(results[0].doc_id, "doc1");
            assert!(
                results[0].distance < 0.01,
                "upserted vector should be close to [0,0,1], distance={}",
                results[0].distance
            );
        }

        #[tokio::test]
        async fn test_batch_embed_multiple_docs() {
            let server = MockServer::start().await;
            Mock::given(method("POST"))
                .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "embeddings": [
                        [0.1, 0.0, 0.0],
                        [0.0, 0.2, 0.0],
                        [0.0, 0.0, 0.3],
                        [0.4, 0.0, 0.0],
                        [0.0, 0.5, 0.0]
                    ]
                })))
                .expect(1) // Exactly 1 HTTP request for all 5 docs
                .mount(&server)
                .await;

            let tmp = tempfile::TempDir::new().unwrap();
            let mut embedders = HashMap::new();
            embedders.insert(
                "default".to_string(),
                rest_embedder_batch_config(&server.uri(), 3),
            );

            let (tx, handle, tasks, vector_indices) =
                setup_write_queue_with_embedder(&tmp, "batch_t", Some(embedders));

            let task_id = "batch_task".to_string();
            tasks.insert(task_id.clone(), TaskInfo::new(task_id.clone(), 1, 5));

            let actions: Vec<WriteAction> = (1..=5)
                .map(|i| {
                    WriteAction::Add(crate::types::Document {
                        id: format!("doc{i}"),
                        fields: HashMap::from([(
                            "title".to_string(),
                            FieldValue::Text(format!("Document {i}")),
                        )]),
                    })
                })
                .collect();

            tx.send(WriteOp {
                task_id: task_id.clone(),
                actions,
            })
            .await
            .unwrap();

            drop(tx);
            handle.await.unwrap().unwrap();

            let vi_lock = vector_indices.get("batch_t").unwrap();
            let vi = vi_lock.read().unwrap();
            assert_eq!(vi.len(), 5, "all 5 docs should be in vector index");
        }

        #[tokio::test]
        async fn test_vector_index_auto_created() {
            let server = MockServer::start().await;
            Mock::given(method("POST"))
                .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "embedding": [0.1, 0.2, 0.3]
                })))
                .mount(&server)
                .await;

            let tmp = tempfile::TempDir::new().unwrap();
            let mut embedders = HashMap::new();
            embedders.insert(
                "default".to_string(),
                rest_embedder_config(&server.uri(), 3),
            );

            let (tx, handle, tasks, vector_indices) =
                setup_write_queue_with_embedder(&tmp, "autocreate_t", Some(embedders));

            // No VectorIndex exists yet
            assert!(!vector_indices.contains_key("autocreate_t"));

            let task_id = "autocreate_task".to_string();
            tasks.insert(task_id.clone(), TaskInfo::new(task_id.clone(), 1, 1));

            tx.send(WriteOp {
                task_id: task_id.clone(),
                actions: vec![WriteAction::Add(crate::types::Document {
                    id: "doc1".to_string(),
                    fields: HashMap::from([(
                        "title".to_string(),
                        FieldValue::Text("first doc".into()),
                    )]),
                })],
            })
            .await
            .unwrap();

            drop(tx);
            handle.await.unwrap().unwrap();

            assert!(
                vector_indices.contains_key("autocreate_t"),
                "VectorIndex should be auto-created on first doc"
            );
            let vi_lock = vector_indices.get("autocreate_t").unwrap();
            let vi = vi_lock.read().unwrap();
            assert_eq!(vi.dimensions(), 3, "dimensions should match embedding size");
            assert_eq!(vi.len(), 1);
        }

        // ── User-provided vector tests ──

        #[tokio::test]
        async fn test_vectors_field_used_directly() {
            let server = MockServer::start().await;
            // Zero HTTP requests expected for userProvided
            Mock::given(method("POST"))
                .respond_with(ResponseTemplate::new(200))
                .expect(0)
                .mount(&server)
                .await;

            let tmp = tempfile::TempDir::new().unwrap();
            let mut embedders = HashMap::new();
            embedders.insert(
                "default".to_string(),
                serde_json::json!({
                    "source": "userProvided",
                    "dimensions": 3
                }),
            );

            let (tx, handle, tasks, vector_indices) =
                setup_write_queue_with_embedder(&tmp, "userprov_t", Some(embedders));

            let task_id = "userprov_task".to_string();
            tasks.insert(task_id.clone(), TaskInfo::new(task_id.clone(), 1, 1));

            let mut fields = HashMap::new();
            fields.insert("title".to_string(), FieldValue::Text("Hello".to_string()));
            let mut vectors_map = HashMap::new();
            vectors_map.insert(
                "default".to_string(),
                FieldValue::Array(vec![
                    FieldValue::Float(0.1),
                    FieldValue::Float(0.2),
                    FieldValue::Float(0.3),
                ]),
            );
            fields.insert("_vectors".to_string(), FieldValue::Object(vectors_map));

            tx.send(WriteOp {
                task_id: task_id.clone(),
                actions: vec![WriteAction::Add(crate::types::Document {
                    id: "doc1".to_string(),
                    fields,
                })],
            })
            .await
            .unwrap();

            drop(tx);
            handle.await.unwrap().unwrap();

            // Vector should be stored directly from _vectors
            assert!(vector_indices.contains_key("userprov_t"));
            let vi_lock = vector_indices.get("userprov_t").unwrap();
            let vi = vi_lock.read().unwrap();
            assert_eq!(vi.len(), 1);
            let results = vi.search(&[0.1, 0.2, 0.3], 1).unwrap();
            assert_eq!(results[0].doc_id, "doc1");
        }

        #[tokio::test]
        async fn test_vectors_field_wrong_dimensions_rejected() {
            let tmp = tempfile::TempDir::new().unwrap();
            let mut embedders = HashMap::new();
            embedders.insert(
                "default".to_string(),
                serde_json::json!({
                    "source": "userProvided",
                    "dimensions": 3
                }),
            );

            let (tx, handle, tasks, vector_indices) =
                setup_write_queue_with_embedder(&tmp, "wrongdim_t", Some(embedders));

            let task_id = "wrongdim_task".to_string();
            tasks.insert(task_id.clone(), TaskInfo::new(task_id.clone(), 1, 2));

            // Good doc: correct dimensions
            let mut fields_ok = HashMap::new();
            fields_ok.insert(
                "title".to_string(),
                FieldValue::Text("Good doc".to_string()),
            );
            let mut vectors_ok = HashMap::new();
            vectors_ok.insert(
                "default".to_string(),
                FieldValue::Array(vec![
                    FieldValue::Float(0.1),
                    FieldValue::Float(0.2),
                    FieldValue::Float(0.3),
                ]),
            );
            fields_ok.insert("_vectors".to_string(), FieldValue::Object(vectors_ok));

            // Bad doc: wrong dimensions (2 instead of 3)
            let mut fields_bad = HashMap::new();
            fields_bad.insert("title".to_string(), FieldValue::Text("Bad doc".to_string()));
            let mut vectors_bad = HashMap::new();
            vectors_bad.insert(
                "default".to_string(),
                FieldValue::Array(vec![FieldValue::Float(0.1), FieldValue::Float(0.2)]),
            );
            fields_bad.insert("_vectors".to_string(), FieldValue::Object(vectors_bad));

            tx.send(WriteOp {
                task_id: task_id.clone(),
                actions: vec![
                    WriteAction::Add(crate::types::Document {
                        id: "good".to_string(),
                        fields: fields_ok,
                    }),
                    WriteAction::Add(crate::types::Document {
                        id: "bad".to_string(),
                        fields: fields_bad,
                    }),
                ],
            })
            .await
            .unwrap();

            drop(tx);
            handle.await.unwrap().unwrap();

            let final_task = tasks.get(&task_id).unwrap();
            assert!(matches!(final_task.status, TaskStatus::Succeeded));

            // Good doc should be in vector index
            let vi_lock = vector_indices.get("wrongdim_t").unwrap();
            let vi = vi_lock.read().unwrap();
            assert_eq!(vi.len(), 1, "only good doc should be in vector index");

            // Bad doc should be rejected
            assert!(
                !final_task.rejected_documents.is_empty(),
                "bad doc should be rejected"
            );
        }

        // ── Fallback/error tests ──

        #[tokio::test]
        async fn test_no_embed_without_embedder_config() {
            let tmp = tempfile::TempDir::new().unwrap();

            let (tx, handle, tasks, vector_indices) =
                setup_write_queue_with_embedder(&tmp, "noembed_t", None);

            let task_id = "noembed_task".to_string();
            tasks.insert(task_id.clone(), TaskInfo::new(task_id.clone(), 1, 1));

            tx.send(WriteOp {
                task_id: task_id.clone(),
                actions: vec![WriteAction::Add(crate::types::Document {
                    id: "doc1".to_string(),
                    fields: HashMap::from([(
                        "title".to_string(),
                        FieldValue::Text("no embedder".into()),
                    )]),
                })],
            })
            .await
            .unwrap();

            drop(tx);
            handle.await.unwrap().unwrap();

            let final_task = tasks.get(&task_id).unwrap();
            assert!(matches!(final_task.status, TaskStatus::Succeeded));
            assert_eq!(final_task.indexed_documents, 1);

            // No VectorIndex should be created
            assert!(
                !vector_indices.contains_key("noembed_t"),
                "no vector index without embedder config"
            );
        }

        #[tokio::test]
        async fn test_embed_failure_does_not_block_tantivy() {
            let server = MockServer::start().await;
            // Server returns 500 — embedding fails
            Mock::given(method("POST"))
                .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
                .mount(&server)
                .await;

            let tmp = tempfile::TempDir::new().unwrap();
            let mut embedders = HashMap::new();
            embedders.insert(
                "default".to_string(),
                rest_embedder_config(&server.uri(), 3),
            );

            let (tx, handle, tasks, vector_indices) =
                setup_write_queue_with_embedder(&tmp, "fail_t", Some(embedders));

            let task_id = "fail_task".to_string();
            tasks.insert(task_id.clone(), TaskInfo::new(task_id.clone(), 1, 1));

            tx.send(WriteOp {
                task_id: task_id.clone(),
                actions: vec![WriteAction::Add(crate::types::Document {
                    id: "doc1".to_string(),
                    fields: HashMap::from([(
                        "title".to_string(),
                        FieldValue::Text("failing embed".into()),
                    )]),
                })],
            })
            .await
            .unwrap();

            drop(tx);
            handle.await.unwrap().unwrap();

            // Document should still be indexed in Tantivy
            let final_task = tasks.get(&task_id).unwrap();
            assert!(
                matches!(final_task.status, TaskStatus::Succeeded),
                "task should succeed despite embed failure"
            );
            assert_eq!(
                final_task.indexed_documents, 1,
                "doc should be indexed in Tantivy"
            );

            // VectorIndex should NOT have the doc
            let vi_count = vector_indices
                .get("fail_t")
                .map(|r| r.read().unwrap().len())
                .unwrap_or(0);
            assert_eq!(
                vi_count, 0,
                "vector index should be empty after embed failure"
            );
        }

        #[tokio::test]
        async fn test_user_provided_source_no_vectors_field_skipped() {
            let tmp = tempfile::TempDir::new().unwrap();
            let mut embedders = HashMap::new();
            embedders.insert(
                "default".to_string(),
                serde_json::json!({
                    "source": "userProvided",
                    "dimensions": 3
                }),
            );

            let (tx, handle, tasks, vector_indices) =
                setup_write_queue_with_embedder(&tmp, "novec_t", Some(embedders));

            let task_id = "novec_task".to_string();
            tasks.insert(task_id.clone(), TaskInfo::new(task_id.clone(), 1, 1));

            // Document without _vectors field + userProvided source
            tx.send(WriteOp {
                task_id: task_id.clone(),
                actions: vec![WriteAction::Add(crate::types::Document {
                    id: "doc1".to_string(),
                    fields: HashMap::from([(
                        "title".to_string(),
                        FieldValue::Text("no vectors".into()),
                    )]),
                })],
            })
            .await
            .unwrap();

            drop(tx);
            handle.await.unwrap().unwrap();

            let final_task = tasks.get(&task_id).unwrap();
            assert!(matches!(final_task.status, TaskStatus::Succeeded));
            assert_eq!(final_task.indexed_documents, 1);

            // No vector stored
            let vi_count = vector_indices
                .get("novec_t")
                .map(|r| r.read().unwrap().len())
                .unwrap_or(0);
            assert_eq!(vi_count, 0, "no vectors should be stored");
        }

        // ── Delete tests ──

        #[tokio::test]
        async fn test_delete_removes_from_vector_index() {
            let server = MockServer::start().await;
            Mock::given(method("POST"))
                .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "embedding": [0.5, 0.5, 0.5]
                })))
                .mount(&server)
                .await;

            let tmp = tempfile::TempDir::new().unwrap();
            let mut embedders = HashMap::new();
            embedders.insert(
                "default".to_string(),
                rest_embedder_config(&server.uri(), 3),
            );

            let (tx, handle, tasks, vector_indices) =
                setup_write_queue_with_embedder(&tmp, "del_vec_t", Some(embedders));

            // Add a document
            let task1 = "del_vec_t1".to_string();
            tasks.insert(task1.clone(), TaskInfo::new(task1.clone(), 1, 1));
            tx.send(WriteOp {
                task_id: task1.clone(),
                actions: vec![WriteAction::Add(crate::types::Document {
                    id: "doc1".to_string(),
                    fields: HashMap::from([(
                        "title".to_string(),
                        FieldValue::Text("to be deleted".into()),
                    )]),
                })],
            })
            .await
            .unwrap();

            tokio::time::sleep(std::time::Duration::from_millis(200)).await;

            // Delete the document
            let task2 = "del_vec_t2".to_string();
            tasks.insert(task2.clone(), TaskInfo::new(task2.clone(), 2, 1));
            tx.send(WriteOp {
                task_id: task2.clone(),
                actions: vec![WriteAction::Delete("doc1".to_string())],
            })
            .await
            .unwrap();

            drop(tx);
            handle.await.unwrap().unwrap();

            let vi_lock = vector_indices.get("del_vec_t").unwrap();
            let vi = vi_lock.read().unwrap();
            assert_eq!(
                vi.len(),
                0,
                "doc should be removed from vector index after delete"
            );
        }

        #[tokio::test]
        async fn test_delete_nonexistent_in_vector_index_silent() {
            let tmp = tempfile::TempDir::new().unwrap();
            let mut embedders = HashMap::new();
            embedders.insert(
                "default".to_string(),
                serde_json::json!({
                    "source": "userProvided",
                    "dimensions": 3
                }),
            );

            let (tx, handle, tasks, _vector_indices) =
                setup_write_queue_with_embedder(&tmp, "delnone_t", Some(embedders));

            // Delete a doc that was never added
            let task_id = "delnone_task".to_string();
            tasks.insert(task_id.clone(), TaskInfo::new(task_id.clone(), 1, 1));
            tx.send(WriteOp {
                task_id: task_id.clone(),
                actions: vec![WriteAction::Delete("nonexistent".to_string())],
            })
            .await
            .unwrap();

            drop(tx);
            handle.await.unwrap().unwrap();

            let final_task = tasks.get(&task_id).unwrap();
            assert!(
                matches!(final_task.status, TaskStatus::Succeeded),
                "delete should succeed even for nonexistent doc"
            );
        }

        // ── Stripping test ──

        #[tokio::test]
        async fn test_vectors_field_stripped_from_tantivy() {
            let tmp = tempfile::TempDir::new().unwrap();
            let tenant_id = "strip_t";
            let tenant_path = tmp.path().join(tenant_id);
            std::fs::create_dir_all(&tenant_path).unwrap();

            let mut embedders = HashMap::new();
            embedders.insert(
                "default".to_string(),
                serde_json::json!({
                    "source": "userProvided",
                    "dimensions": 3
                }),
            );
            let settings = crate::index::settings::IndexSettings {
                embedders: Some(embedders),
                ..Default::default()
            };
            std::fs::write(
                tenant_path.join("settings.json"),
                serde_json::to_string_pretty(&settings).unwrap(),
            )
            .unwrap();

            let schema = crate::index::schema::Schema::builder().build();
            let index = Arc::new(crate::index::Index::create(&tenant_path, schema).unwrap());

            let writers = Arc::new(dashmap::DashMap::new());
            let tasks: Arc<dashmap::DashMap<String, TaskInfo>> = Arc::new(dashmap::DashMap::new());
            let facet_cache = Arc::new(dashmap::DashMap::new());
            let lww_map = Arc::new(dashmap::DashMap::new());
            let vector_indices: VectorIndicesMap = Arc::new(dashmap::DashMap::new());
            let vector_ctx = VectorWriteContext::new(Arc::clone(&vector_indices));

            let (tx, handle) = create_write_queue(
                tenant_id.to_string(),
                Arc::clone(&index),
                writers,
                Arc::clone(&tasks),
                tmp.path().to_path_buf(),
                None,
                facet_cache,
                lww_map,
                vector_ctx,
            );

            let task_id = "strip_task".to_string();
            tasks.insert(task_id.clone(), TaskInfo::new(task_id.clone(), 1, 1));

            let mut fields = HashMap::new();
            fields.insert(
                "title".to_string(),
                FieldValue::Text("test stripping".to_string()),
            );
            let mut vectors_map = HashMap::new();
            vectors_map.insert(
                "default".to_string(),
                FieldValue::Array(vec![
                    FieldValue::Float(0.1),
                    FieldValue::Float(0.2),
                    FieldValue::Float(0.3),
                ]),
            );
            fields.insert("_vectors".to_string(), FieldValue::Object(vectors_map));

            tx.send(WriteOp {
                task_id: task_id.clone(),
                actions: vec![WriteAction::Add(crate::types::Document {
                    id: "doc1".to_string(),
                    fields,
                })],
            })
            .await
            .unwrap();

            drop(tx);
            handle.await.unwrap().unwrap();

            // Vector should be in VectorIndex
            assert!(vector_indices.contains_key(tenant_id));

            // Read back from Tantivy — _vectors should NOT be stored
            index.reader().reload().unwrap();
            let searcher = index.reader().searcher();
            let top_docs = searcher
                .search(
                    &tantivy::query::AllQuery,
                    &tantivy::collector::TopDocs::with_limit(10),
                )
                .unwrap();
            assert_eq!(top_docs.len(), 1, "should have 1 document in Tantivy");

            let doc: tantivy::TantivyDocument = searcher.doc(top_docs[0].1).unwrap();
            let tantivy_schema = index.inner().schema();
            // Import the Document trait for to_json()
            use tantivy::schema::document::Document as TantivyDocTrait;
            let doc_json_str = doc.to_json(&tantivy_schema);
            assert!(
                !doc_json_str.contains("_vectors"),
                "_vectors should be stripped from Tantivy document, got: {doc_json_str}"
            );
        }

        // ── Vector index disk persistence tests (8.1) ──

        #[tokio::test]
        async fn test_vector_index_saved_after_commit() {
            let server = MockServer::start().await;
            Mock::given(method("POST"))
                .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "embedding": [0.1, 0.2, 0.3]
                })))
                .mount(&server)
                .await;

            let tmp = tempfile::TempDir::new().unwrap();
            let mut embedders = HashMap::new();
            embedders.insert(
                "default".to_string(),
                rest_embedder_config(&server.uri(), 3),
            );

            let (tx, handle, tasks, _vector_indices) =
                setup_write_queue_with_embedder(&tmp, "save_t", Some(embedders));

            let task_id = "save_task".to_string();
            tasks.insert(task_id.clone(), TaskInfo::new(task_id.clone(), 1, 2));

            tx.send(WriteOp {
                task_id: task_id.clone(),
                actions: vec![
                    WriteAction::Add(crate::types::Document {
                        id: "doc1".to_string(),
                        fields: HashMap::from([(
                            "title".to_string(),
                            FieldValue::Text("First document".to_string()),
                        )]),
                    }),
                    WriteAction::Add(crate::types::Document {
                        id: "doc2".to_string(),
                        fields: HashMap::from([(
                            "title".to_string(),
                            FieldValue::Text("Second document".to_string()),
                        )]),
                    }),
                ],
            })
            .await
            .unwrap();

            drop(tx);
            handle.await.unwrap().unwrap();

            // Verify vector files exist on disk
            let vectors_dir = tmp.path().join("save_t").join("vectors");
            assert!(
                vectors_dir.join("index.usearch").exists(),
                "index.usearch should exist on disk after commit"
            );
            assert!(
                vectors_dir.join("id_map.json").exists(),
                "id_map.json should exist on disk after commit"
            );

            // Load from disk and verify searchable with correct dimensions
            let loaded = crate::vector::index::VectorIndex::load(
                &vectors_dir,
                usearch::ffi::MetricKind::Cos,
            )
            .unwrap();
            assert_eq!(loaded.len(), 2);
            assert_eq!(loaded.dimensions(), 3);

            let results = loaded.search(&[0.1, 0.2, 0.3], 2).unwrap();
            assert_eq!(results.len(), 2);
        }

        #[tokio::test]
        async fn test_vector_index_save_reflects_deletes() {
            let server = MockServer::start().await;
            Mock::given(method("POST"))
                .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "embedding": [0.5, 0.5, 0.5]
                })))
                .mount(&server)
                .await;

            let tmp = tempfile::TempDir::new().unwrap();
            let mut embedders = HashMap::new();
            embedders.insert(
                "default".to_string(),
                rest_embedder_config(&server.uri(), 3),
            );

            let (tx, handle, tasks, _vector_indices) =
                setup_write_queue_with_embedder(&tmp, "savedel_t", Some(embedders));

            // Add two docs
            let task1 = "savedel_t1".to_string();
            tasks.insert(task1.clone(), TaskInfo::new(task1.clone(), 1, 2));
            tx.send(WriteOp {
                task_id: task1.clone(),
                actions: vec![
                    WriteAction::Add(crate::types::Document {
                        id: "doc1".to_string(),
                        fields: HashMap::from([(
                            "title".to_string(),
                            FieldValue::Text("First".to_string()),
                        )]),
                    }),
                    WriteAction::Add(crate::types::Document {
                        id: "doc2".to_string(),
                        fields: HashMap::from([(
                            "title".to_string(),
                            FieldValue::Text("Second".to_string()),
                        )]),
                    }),
                ],
            })
            .await
            .unwrap();

            tokio::time::sleep(std::time::Duration::from_millis(200)).await;

            // Delete one doc
            let task2 = "savedel_t2".to_string();
            tasks.insert(task2.clone(), TaskInfo::new(task2.clone(), 2, 1));
            tx.send(WriteOp {
                task_id: task2.clone(),
                actions: vec![WriteAction::Delete("doc1".to_string())],
            })
            .await
            .unwrap();

            drop(tx);
            handle.await.unwrap().unwrap();

            // Load from disk and verify doc1 is not in the index
            let vectors_dir = tmp.path().join("savedel_t").join("vectors");
            let loaded = crate::vector::index::VectorIndex::load(
                &vectors_dir,
                usearch::ffi::MetricKind::Cos,
            )
            .unwrap();
            assert_eq!(loaded.len(), 1, "only doc2 should remain after delete");

            let results = loaded.search(&[0.5, 0.5, 0.5], 1).unwrap();
            assert_eq!(results[0].doc_id, "doc2");
        }

        #[tokio::test]
        async fn test_vector_save_skipped_when_no_vector_changes() {
            let tmp = tempfile::TempDir::new().unwrap();
            // No embedder configured
            let (tx, handle, tasks, _vector_indices) =
                setup_write_queue_with_embedder(&tmp, "novec_save_t", None);

            let task_id = "novec_save_task".to_string();
            tasks.insert(task_id.clone(), TaskInfo::new(task_id.clone(), 1, 1));

            tx.send(WriteOp {
                task_id: task_id.clone(),
                actions: vec![WriteAction::Add(crate::types::Document {
                    id: "doc1".to_string(),
                    fields: HashMap::from([(
                        "title".to_string(),
                        FieldValue::Text("no vectors".into()),
                    )]),
                })],
            })
            .await
            .unwrap();

            drop(tx);
            handle.await.unwrap().unwrap();

            // No vectors/ directory should exist
            let vectors_dir = tmp.path().join("novec_save_t").join("vectors");
            assert!(
                !vectors_dir.exists(),
                "vectors/ directory should not be created without embedder"
            );
        }

        #[tokio::test]
        async fn test_vector_index_save_reflects_upserts() {
            let server = MockServer::start().await;
            let call_count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
            let call_count_clone = call_count.clone();

            Mock::given(method("POST"))
                .respond_with(move |_req: &wiremock::Request| {
                    let n = call_count_clone.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    // First call returns [0.1, 0.2, 0.3], second returns [0.9, 0.8, 0.7]
                    let vec = if n == 0 {
                        vec![0.1, 0.2, 0.3]
                    } else {
                        vec![0.9, 0.8, 0.7]
                    };
                    ResponseTemplate::new(200).set_body_json(serde_json::json!({
                        "embedding": vec
                    }))
                })
                .mount(&server)
                .await;

            let tmp = tempfile::TempDir::new().unwrap();
            let mut embedders = HashMap::new();
            embedders.insert(
                "default".to_string(),
                rest_embedder_config(&server.uri(), 3),
            );

            let (tx, handle, tasks, _vector_indices) =
                setup_write_queue_with_embedder(&tmp, "upsert_save_t", Some(embedders));

            // Add doc1
            let task1 = "upsert_t1".to_string();
            tasks.insert(task1.clone(), TaskInfo::new(task1.clone(), 1, 1));
            tx.send(WriteOp {
                task_id: task1.clone(),
                actions: vec![WriteAction::Add(crate::types::Document {
                    id: "doc1".to_string(),
                    fields: HashMap::from([(
                        "title".to_string(),
                        FieldValue::Text("original".to_string()),
                    )]),
                })],
            })
            .await
            .unwrap();

            tokio::time::sleep(std::time::Duration::from_millis(200)).await;

            // Upsert doc1 with new content (gets new embedding)
            let task2 = "upsert_t2".to_string();
            tasks.insert(task2.clone(), TaskInfo::new(task2.clone(), 2, 1));
            tx.send(WriteOp {
                task_id: task2.clone(),
                actions: vec![WriteAction::Upsert(crate::types::Document {
                    id: "doc1".to_string(),
                    fields: HashMap::from([(
                        "title".to_string(),
                        FieldValue::Text("updated".to_string()),
                    )]),
                })],
            })
            .await
            .unwrap();

            drop(tx);
            handle.await.unwrap().unwrap();

            // Load from disk and verify only 1 doc with updated vector
            let vectors_dir = tmp.path().join("upsert_save_t").join("vectors");
            let loaded = crate::vector::index::VectorIndex::load(
                &vectors_dir,
                usearch::ffi::MetricKind::Cos,
            )
            .unwrap();
            assert_eq!(loaded.len(), 1, "upsert should replace, not duplicate");

            let results = loaded.search(&[0.9, 0.8, 0.7], 1).unwrap();
            assert_eq!(results[0].doc_id, "doc1");
        }

        // ── Oplog vector storage tests (8.7) ──

        /// Helper that creates a write queue with an oplog (unlike setup_write_queue_with_embedder
        /// which passes None for oplog). Returns the oplog Arc so tests can read entries back.
        fn setup_write_queue_with_oplog(
            tmp: &tempfile::TempDir,
            tenant_id: &str,
            embedder_settings: Option<HashMap<String, serde_json::Value>>,
        ) -> (
            WriteQueue,
            tokio::task::JoinHandle<crate::error::Result<()>>,
            Arc<dashmap::DashMap<String, TaskInfo>>,
            VectorIndicesMap,
            Arc<crate::index::oplog::OpLog>,
        ) {
            let tenant_path = tmp.path().join(tenant_id);
            std::fs::create_dir_all(&tenant_path).unwrap();

            let settings = crate::index::settings::IndexSettings {
                embedders: embedder_settings,
                ..Default::default()
            };
            let settings_json = serde_json::to_string_pretty(&settings).unwrap();
            std::fs::write(tenant_path.join("settings.json"), settings_json).unwrap();

            let schema = crate::index::schema::Schema::builder().build();
            let index = Arc::new(crate::index::Index::create(&tenant_path, schema).unwrap());

            let writers = Arc::new(dashmap::DashMap::new());
            let tasks: Arc<dashmap::DashMap<String, TaskInfo>> = Arc::new(dashmap::DashMap::new());
            let facet_cache = Arc::new(dashmap::DashMap::new());
            let lww_map = Arc::new(dashmap::DashMap::new());
            let vector_indices: VectorIndicesMap = Arc::new(dashmap::DashMap::new());
            let vector_ctx = VectorWriteContext::new(Arc::clone(&vector_indices));

            let oplog_dir = tenant_path.join("oplog");
            let oplog = Arc::new(
                crate::index::oplog::OpLog::open(&oplog_dir, tenant_id, "test_node").unwrap(),
            );

            let (tx, handle) = create_write_queue(
                tenant_id.to_string(),
                index,
                writers,
                Arc::clone(&tasks),
                tmp.path().to_path_buf(),
                Some(Arc::clone(&oplog)),
                facet_cache,
                lww_map,
                vector_ctx,
            );

            (tx, handle, tasks, vector_indices, oplog)
        }

        #[tokio::test]
        async fn test_computed_vectors_stored_in_oplog() {
            let server = MockServer::start().await;
            Mock::given(method("POST"))
                .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "embedding": [0.1, 0.2, 0.3]
                })))
                .mount(&server)
                .await;

            let tmp = tempfile::TempDir::new().unwrap();
            let mut embedders = HashMap::new();
            embedders.insert(
                "default".to_string(),
                rest_embedder_config(&server.uri(), 3),
            );

            let (tx, handle, tasks, _vi, oplog) =
                setup_write_queue_with_oplog(&tmp, "oplog_vec_t", Some(embedders));

            let task_id = "oplog_vec_task".to_string();
            tasks.insert(task_id.clone(), TaskInfo::new(task_id.clone(), 1, 1));

            tx.send(WriteOp {
                task_id: task_id.clone(),
                actions: vec![WriteAction::Add(crate::types::Document {
                    id: "doc1".to_string(),
                    fields: HashMap::from([(
                        "title".to_string(),
                        FieldValue::Text("test oplog vectors".to_string()),
                    )]),
                })],
            })
            .await
            .unwrap();

            drop(tx);
            handle.await.unwrap().unwrap();

            // Read oplog and verify computed vectors are stored
            let entries = oplog.read_since(0).unwrap();
            let upsert = entries
                .iter()
                .find(|e| e.op_type == "upsert")
                .expect("should have an upsert entry");

            let body = upsert.payload.get("body").expect("upsert should have body");
            let vectors = body
                .get("_vectors")
                .expect("body should contain _vectors after embedding");
            let default_vec = vectors
                .get("default")
                .expect("_vectors should have 'default' embedder");

            let vec_array: Vec<f64> = default_vec
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_f64().unwrap())
                .collect();
            assert_eq!(vec_array.len(), 3);
            assert!((vec_array[0] - 0.1).abs() < 0.01);
            assert!((vec_array[1] - 0.2).abs() < 0.01);
            assert!((vec_array[2] - 0.3).abs() < 0.01);
        }

        #[tokio::test]
        async fn test_user_provided_vectors_preserved_in_oplog() {
            let tmp = tempfile::TempDir::new().unwrap();
            let mut embedders = HashMap::new();
            embedders.insert(
                "default".to_string(),
                serde_json::json!({
                    "source": "userProvided",
                    "dimensions": 3
                }),
            );

            let (tx, handle, tasks, _vi, oplog) =
                setup_write_queue_with_oplog(&tmp, "oplog_user_t", Some(embedders));

            let task_id = "oplog_user_task".to_string();
            tasks.insert(task_id.clone(), TaskInfo::new(task_id.clone(), 1, 1));

            let mut fields = HashMap::new();
            fields.insert(
                "title".to_string(),
                FieldValue::Text("user vectors".to_string()),
            );
            let mut vectors_map = HashMap::new();
            vectors_map.insert(
                "default".to_string(),
                FieldValue::Array(vec![
                    FieldValue::Float(1.0),
                    FieldValue::Float(0.0),
                    FieldValue::Float(0.0),
                ]),
            );
            fields.insert("_vectors".to_string(), FieldValue::Object(vectors_map));

            tx.send(WriteOp {
                task_id: task_id.clone(),
                actions: vec![WriteAction::Add(crate::types::Document {
                    id: "doc1".to_string(),
                    fields,
                })],
            })
            .await
            .unwrap();

            drop(tx);
            handle.await.unwrap().unwrap();

            // Read oplog and verify user-provided vectors are preserved
            let entries = oplog.read_since(0).unwrap();
            let upsert = entries
                .iter()
                .find(|e| e.op_type == "upsert")
                .expect("should have an upsert entry");

            let body = upsert.payload.get("body").unwrap();
            let vectors = body.get("_vectors").expect("body should preserve _vectors");
            let default_vec = vectors
                .get("default")
                .expect("_vectors should have 'default'");

            let vec_array: Vec<f64> = default_vec
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_f64().unwrap())
                .collect();
            assert_eq!(vec_array, vec![1.0, 0.0, 0.0]);
        }

        #[tokio::test]
        async fn test_oplog_vectors_contain_all_embedder_results() {
            let server = MockServer::start().await;
            Mock::given(method("POST"))
                .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "embedding": [0.5, 0.5, 0.5]
                })))
                .mount(&server)
                .await;

            let tmp = tempfile::TempDir::new().unwrap();
            let mut embedders = HashMap::new();
            // Two REST embedders with different names
            embedders.insert(
                "embedder_a".to_string(),
                rest_embedder_config(&server.uri(), 3),
            );
            embedders.insert(
                "embedder_b".to_string(),
                rest_embedder_config(&server.uri(), 3),
            );

            let (tx, handle, tasks, _vi, oplog) =
                setup_write_queue_with_oplog(&tmp, "oplog_multi_t", Some(embedders));

            let task_id = "oplog_multi_task".to_string();
            tasks.insert(task_id.clone(), TaskInfo::new(task_id.clone(), 1, 1));

            tx.send(WriteOp {
                task_id: task_id.clone(),
                actions: vec![WriteAction::Add(crate::types::Document {
                    id: "doc1".to_string(),
                    fields: HashMap::from([(
                        "title".to_string(),
                        FieldValue::Text("multi embedder doc".to_string()),
                    )]),
                })],
            })
            .await
            .unwrap();

            drop(tx);
            handle.await.unwrap().unwrap();

            // Read oplog and verify both embedders' vectors are present
            let entries = oplog.read_since(0).unwrap();
            let upsert = entries
                .iter()
                .find(|e| e.op_type == "upsert")
                .expect("should have an upsert entry");

            let body = upsert.payload.get("body").unwrap();
            let vectors = body
                .get("_vectors")
                .expect("body should contain _vectors with all embedders");

            assert!(
                vectors.get("embedder_a").is_some(),
                "_vectors should contain embedder_a"
            );
            assert!(
                vectors.get("embedder_b").is_some(),
                "_vectors should contain embedder_b"
            );

            // Both should have 3-dimensional vectors
            let vec_a: Vec<f64> = vectors["embedder_a"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_f64().unwrap())
                .collect();
            assert_eq!(vec_a.len(), 3);

            let vec_b: Vec<f64> = vectors["embedder_b"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_f64().unwrap())
                .collect();
            assert_eq!(vec_b.len(), 3);
        }

        #[tokio::test]
        async fn test_fingerprint_saved_alongside_vector_index() {
            let server = MockServer::start().await;
            Mock::given(method("POST"))
                .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "embedding": [0.1, 0.2, 0.3]
                })))
                .mount(&server)
                .await;

            let tmp = tempfile::TempDir::new().unwrap();
            let mut embedders = HashMap::new();
            embedders.insert(
                "default".to_string(),
                rest_embedder_config(&server.uri(), 3),
            );

            let (tx, handle, tasks, _vi, _oplog) =
                setup_write_queue_with_oplog(&tmp, "fp_save_t", Some(embedders));

            let task_id = "fp_save_task".to_string();
            tasks.insert(task_id.clone(), TaskInfo::new(task_id.clone(), 1, 1));

            tx.send(WriteOp {
                task_id: task_id.clone(),
                actions: vec![WriteAction::Add(crate::types::Document {
                    id: "doc1".to_string(),
                    fields: HashMap::from([(
                        "title".to_string(),
                        FieldValue::Text("fingerprint test".to_string()),
                    )]),
                })],
            })
            .await
            .unwrap();

            drop(tx);
            handle.await.unwrap().unwrap();

            // Verify fingerprint.json exists alongside vector files
            let vectors_dir = tmp.path().join("fp_save_t").join("vectors");
            assert!(
                vectors_dir.join("index.usearch").exists(),
                "index.usearch should exist"
            );
            assert!(
                vectors_dir.join("fingerprint.json").exists(),
                "fingerprint.json should exist alongside vector files"
            );

            // Load and verify fingerprint content
            let fp = crate::vector::config::EmbedderFingerprint::load(&vectors_dir).unwrap();
            assert_eq!(fp.version, 1);
            assert_eq!(fp.embedders.len(), 1);
            assert_eq!(fp.embedders[0].name, "default");
            assert_eq!(
                fp.embedders[0].source,
                crate::vector::config::EmbedderSource::Rest
            );
            assert_eq!(fp.embedders[0].dimensions, 3);
        }

        // ── FastEmbed integration tests (9.16) ──

        #[cfg(feature = "vector-search-local")]
        #[tokio::test]
        async fn test_fastembed_auto_embed_on_add() {
            let tmp = tempfile::TempDir::new().unwrap();
            let mut embedders = HashMap::new();
            embedders.insert(
                "default".to_string(),
                serde_json::json!({ "source": "fastEmbed" }),
            );

            let (tx, handle, tasks, vector_indices) =
                setup_write_queue_with_embedder(&tmp, "fe_embed_t", Some(embedders));

            let task_id = "fe_embed_task".to_string();
            tasks.insert(task_id.clone(), TaskInfo::new(task_id.clone(), 1, 1));

            tx.send(WriteOp {
                task_id: task_id.clone(),
                actions: vec![WriteAction::Add(crate::types::Document {
                    id: "doc1".to_string(),
                    fields: HashMap::from([(
                        "title".to_string(),
                        FieldValue::Text("Hello local embedding".to_string()),
                    )]),
                })],
            })
            .await
            .unwrap();

            drop(tx);
            handle.await.unwrap().unwrap();

            let final_task = tasks.get(&task_id).unwrap();
            assert!(
                matches!(final_task.status, TaskStatus::Succeeded),
                "task should succeed, got: {:?}",
                final_task.status
            );

            // Verify vector index was auto-created with correct dimensions
            assert!(
                vector_indices.contains_key("fe_embed_t"),
                "vector index should be auto-created for fastembed"
            );
            let vi_lock = vector_indices.get("fe_embed_t").unwrap();
            let vi = vi_lock.read().unwrap();
            assert_eq!(vi.len(), 1, "vector index should have 1 document");
            assert_eq!(
                vi.dimensions(),
                384,
                "BGESmallENV15 default model should produce 384-dim vectors"
            );
        }

        #[cfg(feature = "vector-search-local")]
        #[tokio::test]
        async fn test_fastembed_vectors_in_oplog() {
            let tmp = tempfile::TempDir::new().unwrap();
            let mut embedders = HashMap::new();
            embedders.insert(
                "default".to_string(),
                serde_json::json!({ "source": "fastEmbed" }),
            );

            let (tx, handle, tasks, _vi, oplog) =
                setup_write_queue_with_oplog(&tmp, "fe_oplog_t", Some(embedders));

            let task_id = "fe_oplog_task".to_string();
            tasks.insert(task_id.clone(), TaskInfo::new(task_id.clone(), 1, 1));

            tx.send(WriteOp {
                task_id: task_id.clone(),
                actions: vec![WriteAction::Add(crate::types::Document {
                    id: "doc1".to_string(),
                    fields: HashMap::from([(
                        "title".to_string(),
                        FieldValue::Text("oplog fastembed test".to_string()),
                    )]),
                })],
            })
            .await
            .unwrap();

            drop(tx);
            handle.await.unwrap().unwrap();

            // Read oplog and verify computed vectors are stored
            let entries = oplog.read_since(0).unwrap();
            let upsert = entries
                .iter()
                .find(|e| e.op_type == "upsert")
                .expect("should have an upsert entry");

            let body = upsert.payload.get("body").expect("upsert should have body");
            let vectors = body
                .get("_vectors")
                .expect("body should contain _vectors after fastembed embedding");
            let default_vec = vectors
                .get("default")
                .expect("_vectors should have 'default' embedder");

            let vec_array: Vec<f64> = default_vec
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_f64().unwrap())
                .collect();
            assert_eq!(
                vec_array.len(),
                384,
                "fastembed BGESmallENV15 should produce 384-dim vectors in oplog"
            );
        }
    }
}
