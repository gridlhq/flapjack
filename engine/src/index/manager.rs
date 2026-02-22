use crate::error::{FlapjackError, Result};
use crate::index::oplog::OpLog;
use crate::index::relevance::RelevanceConfig;
use crate::index::rules::RuleStore;
use crate::index::settings::IndexSettings;
use crate::index::synonyms::SynonymStore;
use crate::index::task_queue::TaskQueue;
use crate::index::utils::copy_dir_recursive;
use crate::index::write_queue::{
    create_write_queue, VectorWriteContext, WriteAction, WriteOp, WriteQueue,
};
use crate::index::Index;
use crate::query::{QueryExecutor, QueryParser};
use crate::types::{
    Document, FacetRequest, Filter, SearchResult, Sort, TaskInfo, TaskStatus, TenantId,
};
use dashmap::DashMap;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::task::JoinHandle;

const MAX_TASKS_PER_TENANT: usize = 1000;

/// Multi-tenant index manager.
///
/// `IndexManager` owns a collection of [`Index`] instances (one per tenant),
/// handles lazy loading from disk, background write queues, facet caching,
/// oplog recovery, and query execution with synonyms/rules.
///
/// Create one with [`IndexManager::new`], which returns `Arc<IndexManager>`
/// (it is `Send + Sync` and designed to be shared).
///
/// # Examples
///
/// ```rust,no_run
/// use flapjack::IndexManager;
///
/// # fn main() -> flapjack::Result<()> {
/// let manager = IndexManager::new("./data");
/// manager.create_tenant("products")?;
/// let results = manager.search("products", "laptop", None, None, 10)?;
/// # Ok(())
/// # }
/// ```
pub struct IndexManager {
    pub base_path: PathBuf,
    pub(crate) loaded: DashMap<TenantId, Arc<Index>>,
    pub(crate) writers:
        Arc<DashMap<TenantId, Arc<tokio::sync::Mutex<crate::index::ManagedIndexWriter>>>>,
    pub(crate) write_queues: DashMap<TenantId, WriteQueue>,
    pub(crate) write_task_handles: DashMap<TenantId, JoinHandle<Result<()>>>,
    pub(crate) oplogs: DashMap<TenantId, Arc<OpLog>>,
    tasks: Arc<DashMap<String, TaskInfo>>,
    task_queue: TaskQueue,
    settings_cache: DashMap<TenantId, Arc<IndexSettings>>,
    rules_cache: DashMap<TenantId, Arc<RuleStore>>,
    synonyms_cache: DashMap<TenantId, Arc<SynonymStore>>,
    pub facet_cache: Arc<
        DashMap<
            String,
            Arc<(
                std::time::Instant,
                usize,
                HashMap<String, Vec<crate::types::FacetCount>>,
            )>,
        >,
    >,
    pub facet_cache_cap: std::sync::atomic::AtomicUsize,
    /// LWW (last-writer-wins) tracking for replicated ops.
    /// Maps tenant_id -> (object_id -> (timestamp_ms, node_id)).
    /// Shared with write_queue so primary writes also populate LWW state.
    pub(crate) lww_map: Arc<DashMap<TenantId, DashMap<String, (u64, String)>>>,
    /// Vector indices per tenant. Uses std::sync::RwLock (not tokio) because
    /// vector search is called from spawn_blocking. Read lock for search,
    /// write lock for add/remove (stage 7). Wrapped in Arc for sharing with
    /// the write queue (commit_batch needs access for auto-embedding).
    #[cfg(feature = "vector-search")]
    vector_indices:
        Arc<DashMap<TenantId, Arc<std::sync::RwLock<crate::vector::index::VectorIndex>>>>,
}

const DEFAULT_FACET_CACHE_CAP: usize = 500;

impl IndexManager {
    /// Create a new IndexManager with the given base directory.
    ///
    /// Each tenant's index will be stored in `{base_path}/{tenant_id}/`.
    pub fn new<P: AsRef<Path>>(base_path: P) -> Arc<Self> {
        Arc::new_cyclic(|weak| {
            let tasks = Arc::new(DashMap::new());
            IndexManager {
                base_path: base_path.as_ref().to_path_buf(),
                loaded: DashMap::new(),
                writers: Arc::new(DashMap::new()),
                write_queues: DashMap::new(),
                write_task_handles: DashMap::new(),
                oplogs: DashMap::new(),
                tasks: tasks.clone(),
                task_queue: TaskQueue::new(weak.clone(), tasks),
                settings_cache: DashMap::new(),
                rules_cache: DashMap::new(),
                synonyms_cache: DashMap::new(),
                facet_cache: Arc::new(DashMap::new()),
                facet_cache_cap: std::sync::atomic::AtomicUsize::new(DEFAULT_FACET_CACHE_CAP),
                lww_map: Arc::new(DashMap::new()),
                #[cfg(feature = "vector-search")]
                vector_indices: Arc::new(DashMap::new()),
            }
        })
    }

    /// Get the oplog for a tenant (for external access)
    pub fn get_oplog(&self, tenant_id: &str) -> Option<Arc<OpLog>> {
        self.oplogs.get(tenant_id).map(|r| Arc::clone(&r))
    }

    /// Get the LWW (last-writer-wins) state for a specific document.
    /// Returns (timestamp_ms, node_id) of the highest-priority op seen so far, or None.
    pub fn get_lww(&self, tenant_id: &str, object_id: &str) -> Option<(u64, String)> {
        self.lww_map
            .get(tenant_id)
            .and_then(|m| m.get(object_id).map(|v| v.clone()))
    }

    /// Record that an op for (tenant_id, object_id) with the given (timestamp_ms, node_id)
    /// was applied. Used by apply_ops_to_manager to track LWW state.
    pub fn record_lww(&self, tenant_id: &str, object_id: &str, ts: u64, node_id: String) {
        self.lww_map
            .entry(tenant_id.to_string())
            .or_insert_with(DashMap::new)
            .insert(object_id.to_string(), (ts, node_id));
    }

    /// Remove a tenant from the loaded cache (for external access)
    pub fn unload_tenant(&self, tenant_id: &str) {
        self.loaded.remove(tenant_id);
    }

    pub fn invalidate_facet_cache(&self, tenant_id: &str) {
        let prefix = format!("{}:", tenant_id);
        self.facet_cache.retain(|k, _| !k.starts_with(&prefix));
    }

    pub fn get_settings(&self, tenant_id: &str) -> Option<Arc<IndexSettings>> {
        if let Some(cached) = self.settings_cache.get(tenant_id) {
            return Some(Arc::clone(&cached));
        }
        let path = self.base_path.join(tenant_id).join("settings.json");
        if path.exists() {
            if let Ok(s) = IndexSettings::load(&path) {
                let arc = Arc::new(s);
                self.settings_cache
                    .insert(tenant_id.to_string(), Arc::clone(&arc));
                return Some(arc);
            }
        }
        None
    }

    pub fn get_rules(&self, tenant_id: &str) -> Option<Arc<RuleStore>> {
        if let Some(cached) = self.rules_cache.get(tenant_id) {
            return Some(Arc::clone(&cached));
        }
        let path = self.base_path.join(tenant_id).join("rules.json");
        if path.exists() {
            if let Ok(s) = RuleStore::load(&path) {
                let arc = Arc::new(s);
                self.rules_cache
                    .insert(tenant_id.to_string(), Arc::clone(&arc));
                return Some(arc);
            }
        }
        None
    }

    pub fn get_synonyms(&self, tenant_id: &str) -> Option<Arc<SynonymStore>> {
        if let Some(cached) = self.synonyms_cache.get(tenant_id) {
            return Some(Arc::clone(&cached));
        }
        let path = self.base_path.join(tenant_id).join("synonyms.json");
        if path.exists() {
            if let Ok(s) = SynonymStore::load(&path) {
                let arc = Arc::new(s);
                self.synonyms_cache
                    .insert(tenant_id.to_string(), Arc::clone(&arc));
                return Some(arc);
            }
        }
        None
    }

    pub fn invalidate_settings_cache(&self, tenant_id: &str) {
        self.settings_cache.remove(tenant_id);
    }

    pub fn invalidate_rules_cache(&self, tenant_id: &str) {
        self.rules_cache.remove(tenant_id);
    }

    pub fn invalidate_synonyms_cache(&self, tenant_id: &str) {
        self.synonyms_cache.remove(tenant_id);
    }

    pub fn get_task(&self, task_id: &str) -> Result<TaskInfo> {
        self.tasks
            .get(task_id)
            .map(|task| task.clone())
            .ok_or_else(|| FlapjackError::TaskNotFound(task_id.to_string()))
    }

    /// Count tasks in Enqueued or Processing state for a given tenant.
    pub fn pending_task_count(&self, tenant_id: &str) -> usize {
        let prefix = format!("task_{}_", tenant_id);
        self.tasks
            .iter()
            .filter(|entry| {
                entry.key().starts_with(&prefix)
                    && matches!(
                        entry.value().status,
                        TaskStatus::Enqueued | TaskStatus::Processing
                    )
            })
            .count()
    }

    pub fn evict_old_tasks(&self, tenant_id: &str, max_tasks: usize) {
        let prefix = format!("task_{}_{}", tenant_id, "");
        let mut tenant_tasks: Vec<_> = self
            .tasks
            .iter()
            .filter(|entry| entry.key().starts_with(&prefix))
            .map(|entry| {
                (
                    entry.key().clone(),
                    entry.value().numeric_id,
                    entry.value().created_at,
                )
            })
            .collect();

        if tenant_tasks.len() >= max_tasks {
            tenant_tasks.sort_by_key(|(_, _, created_at)| *created_at);
            for (task_id, numeric_id, _) in
                tenant_tasks.iter().take(tenant_tasks.len() - max_tasks + 1)
            {
                self.tasks.remove(task_id);
                // Also remove the numeric_id alias key
                self.tasks.remove(&numeric_id.to_string());
            }
        }
    }

    pub fn create_tenant(&self, tenant_id: &str) -> Result<()> {
        if self.loaded.contains_key(tenant_id) {
            return Ok(());
        }

        let path = self.base_path.join(tenant_id);
        if path.exists() {
            let index = Arc::new(Index::open(&path)?);
            let _ = index.searchable_paths();
            self.loaded.insert(tenant_id.to_string(), index);
            #[cfg(feature = "vector-search")]
            self.load_vector_index(tenant_id, &path);
            return Ok(());
        }

        std::fs::create_dir_all(&path)?;
        let schema = crate::index::schema::Schema::builder().build();
        let index = Arc::new(Index::create(&path, schema)?);
        self.loaded.insert(tenant_id.to_string(), index);

        let settings_path = path.join("settings.json");
        if !settings_path.exists() {
            let default_settings = IndexSettings::default();
            default_settings.save(&settings_path)?;
        }

        Ok(())
    }

    pub fn get_or_load(&self, tenant_id: &str) -> Result<Arc<Index>> {
        if let Some(index) = self.loaded.get(tenant_id) {
            return Ok(Arc::clone(&index));
        }

        let path = self.base_path.join(tenant_id);
        if !path.exists() {
            return Err(FlapjackError::TenantNotFound(tenant_id.to_string()));
        }

        let index = match Index::open(&path) {
            Ok(idx) => Arc::new(idx),
            Err(e) => {
                let oplog_dir = path.join("oplog");
                if oplog_dir.exists() {
                    tracing::warn!("[RECOVERY {}] Index::open failed ({}), but oplog exists — creating fresh index for replay", tenant_id, e);
                    let cs_path = path.join("committed_seq");
                    if cs_path.exists() {
                        tracing::info!(
                            "[RECOVERY {}] Resetting committed_seq to 0 for full replay",
                            tenant_id
                        );
                        let _ = std::fs::write(&cs_path, "0");
                    }
                    let schema = crate::index::schema::Schema::builder().build();
                    Arc::new(Index::create(&path, schema)?)
                } else {
                    return Err(e);
                }
            }
        };
        self.recover_from_oplog(tenant_id, &index, &path)?;
        #[cfg(feature = "vector-search")]
        self.load_vector_index(tenant_id, &path);
        let _ = index.searchable_paths();
        self.loaded
            .insert(tenant_id.to_string(), Arc::clone(&index));
        Ok(index)
    }

    fn recover_from_oplog(
        &self,
        tenant_id: &str,
        index: &Arc<Index>,
        tenant_path: &std::path::Path,
    ) -> Result<()> {
        let oplog_dir = tenant_path.join("oplog");
        if !oplog_dir.exists() {
            return Ok(());
        }
        let committed_seq_path = tenant_path.join("committed_seq");
        let committed_seq: u64 = if committed_seq_path.exists() {
            std::fs::read_to_string(&committed_seq_path)
                .unwrap_or_default()
                .trim()
                .parse()
                .unwrap_or(0)
        } else {
            0
        };

        let node_id = std::env::var("FLAPJACK_NODE_ID").unwrap_or_else(|_| "unknown".to_string());
        let oplog = OpLog::open(&oplog_dir, tenant_id, &node_id)?;

        // P3: Rebuild lww_map from ALL retained oplog entries (read from seq=0).
        // This runs on every startup — crash or normal — so that stale replicated ops
        // arriving after any restart are correctly rejected by the LWW check in
        // apply_ops_to_manager.  We track the highest (timestamp_ms, node_id) per
        // object so out-of-order oplog entries (clock skew / replication) are handled.
        {
            let all_ops = oplog.read_since(0)?;
            for entry in &all_ops {
                let obj_id = match entry.op_type.as_str() {
                    "upsert" | "delete" => entry.payload.get("objectID").and_then(|v| v.as_str()),
                    _ => None,
                };
                if let Some(obj_id) = obj_id {
                    let incoming = (entry.timestamp_ms, entry.node_id.clone());
                    if self
                        .get_lww(tenant_id, obj_id)
                        .map_or(true, |existing| incoming > existing)
                    {
                        self.record_lww(
                            tenant_id,
                            obj_id,
                            entry.timestamp_ms,
                            entry.node_id.clone(),
                        );
                    }
                }
            }
            if !all_ops.is_empty() {
                tracing::info!(
                    "[RECOVERY {}] rebuilt lww_map from {} oplog entries",
                    tenant_id,
                    all_ops.len()
                );
            }
        }

        let ops = oplog.read_since(committed_seq)?;
        if ops.is_empty() {
            return Ok(());
        }

        tracing::info!(
            "[RECOVERY {}] replaying {} ops from seq {} (committed_seq={})",
            tenant_id,
            ops.len(),
            ops[0].seq,
            committed_seq
        );

        // Phase 1: Replay settings/synonyms/rules ops first to rebuild config files
        for entry in &ops {
            match entry.op_type.as_str() {
                "settings" => {
                    let sp = tenant_path.join("settings.json");
                    let _ = std::fs::write(
                        &sp,
                        serde_json::to_string_pretty(&entry.payload).unwrap_or_default(),
                    );
                    tracing::info!("[RECOVERY {}] restored settings.json from oplog", tenant_id);
                }
                op if op.starts_with("save_synonym") || op == "clear_synonyms" => {
                    // Synonyms handled by dedicated endpoints, reconstruct from current state
                    // For now, skip - proper implementation needs synonym aggregation
                }
                op if op.starts_with("save_rule") || op == "clear_rules" => {
                    // Rules handled by dedicated endpoints, reconstruct from current state
                    // For now, skip - proper implementation needs rules aggregation
                }
                _ => {}
            }
        }

        // Phase 2: Load settings after config ops have restored files
        let settings_path = tenant_path.join("settings.json");
        let settings = if settings_path.exists() {
            Some(crate::index::settings::IndexSettings::load(&settings_path)?)
        } else {
            tracing::warn!(
                "[RECOVERY {}] no settings.json after config phase - using defaults",
                tenant_id
            );
            None
        };

        // Phase 3: Replay document ops
        let mut writer = index.writer()?;
        let schema = index.inner().schema();
        let id_field = schema.get_field("_id").unwrap();
        let mut replayed = 0usize;
        let mut failed = 0usize;

        for entry in &ops {
            match entry.op_type.as_str() {
                "upsert" => {
                    if let Some(obj_id) = entry.payload.get("objectID").and_then(|v| v.as_str()) {
                        let term = tantivy::Term::from_field_text(id_field, obj_id);
                        writer.delete_term(term);
                        if let Some(body) = entry.payload.get("body") {
                            match crate::types::Document::from_json(body) {
                                Ok(doc) => {
                                    match index.converter().to_tantivy(&doc, settings.as_ref()) {
                                        Ok(tantivy_doc) => {
                                            writer.add_document(tantivy_doc)?;
                                            replayed += 1;
                                        }
                                        Err(e) => {
                                            tracing::warn!(
                                                "[RECOVERY {}] failed to_tantivy for {}: {}",
                                                tenant_id,
                                                obj_id,
                                                e
                                            );
                                            failed += 1;
                                        }
                                    }
                                }
                                Err(e) => {
                                    tracing::warn!(
                                        "[RECOVERY {}] failed to parse doc {}: {}",
                                        tenant_id,
                                        obj_id,
                                        e
                                    );
                                    failed += 1;
                                }
                            }
                        }
                    }
                }
                "delete" => {
                    if let Some(obj_id) = entry.payload.get("objectID").and_then(|v| v.as_str()) {
                        let term = tantivy::Term::from_field_text(id_field, obj_id);
                        writer.delete_term(term);
                        replayed += 1;
                    }
                }
                "settings" | "synonyms" | "rules" => {
                    // Already processed in phase 1
                    replayed += 1;
                }
                "clear" => {
                    writer.delete_all_documents()?;
                    replayed += 1;
                }
                _ => {
                    tracing::warn!(
                        "[RECOVERY {}] unknown op_type '{}' at seq {}, skipping",
                        tenant_id,
                        entry.op_type,
                        entry.seq
                    );
                }
            }
        }

        if replayed > 0 {
            writer.commit()?;
            index.reader().reload()?;
            index.invalidate_searchable_paths_cache();
            let final_seq = ops.last().map(|o| o.seq).unwrap_or(committed_seq);
            let _ = std::fs::write(&committed_seq_path, final_seq.to_string());
            if failed > 0 {
                tracing::warn!("[RECOVERY {}] replayed {}/{} ops successfully ({} failed), new committed_seq={}",
                    tenant_id, replayed, ops.len(), failed, final_seq);
            } else {
                tracing::info!(
                    "[RECOVERY {}] replayed {} ops, new committed_seq={}",
                    tenant_id,
                    replayed,
                    final_seq
                );
            }
        }

        // Phase 4: Rebuild VectorIndex from oplog _vectors data.
        #[cfg(feature = "vector-search")]
        {
            let mut vector_index: Option<crate::vector::index::VectorIndex> = None;
            let mut vectors_modified = false;

            for entry in &ops {
                match entry.op_type.as_str() {
                    "upsert" => {
                        if let Some(body) = entry.payload.get("body") {
                            if let Some(vectors_obj) = body.get("_vectors") {
                                if let Some(vecs_map) = vectors_obj.as_object() {
                                    for (_emb_name, vec_val) in vecs_map {
                                        if let Some(arr) = vec_val.as_array() {
                                            let vec: Vec<f32> = arr
                                                .iter()
                                                .filter_map(|v| v.as_f64().map(|f| f as f32))
                                                .collect();
                                            if vec.len() != arr.len() || vec.is_empty() {
                                                continue;
                                            }
                                            let vi = vector_index.get_or_insert_with(|| {
                                                crate::vector::index::VectorIndex::new(
                                                    vec.len(),
                                                    usearch::ffi::MetricKind::Cos,
                                                )
                                                .expect(
                                                    "failed to create VectorIndex during recovery",
                                                )
                                            });
                                            if let Some(obj_id) = entry
                                                .payload
                                                .get("objectID")
                                                .and_then(|v| v.as_str())
                                            {
                                                // add() handles upsert (removes old key if exists)
                                                match vi.add(obj_id, &vec) {
                                                    Ok(()) => vectors_modified = true,
                                                    Err(e) => tracing::warn!(
                                                        "[RECOVERY {}] failed to add vector for '{}': {}",
                                                        tenant_id, obj_id, e
                                                    ),
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    "delete" => {
                        if let Some(vi) = vector_index.as_mut() {
                            if let Some(obj_id) =
                                entry.payload.get("objectID").and_then(|v| v.as_str())
                            {
                                if vi.remove(obj_id).is_ok() {
                                    vectors_modified = true;
                                }
                                // DocumentNotFound is expected (pre-stage-8 entries without vectors)
                            }
                        }
                    }
                    "clear" => {
                        if let Some(vi) = vector_index.as_ref() {
                            let dims = vi.dimensions();
                            vector_index = Some(
                                crate::vector::index::VectorIndex::new(
                                    dims,
                                    usearch::ffi::MetricKind::Cos,
                                )
                                .expect("failed to create VectorIndex during recovery clear"),
                            );
                            vectors_modified = true;
                        }
                    }
                    _ => {}
                }
            }

            if let Some(vi) = vector_index {
                if vectors_modified {
                    let vectors_dir = tenant_path.join("vectors");
                    if let Err(e) = vi.save(&vectors_dir) {
                        tracing::warn!(
                            "[RECOVERY {}] failed to save recovered vector index: {}",
                            tenant_id,
                            e
                        );
                    }
                    let count = vi.len();
                    self.set_vector_index(tenant_id, vi);
                    tracing::info!(
                        "[RECOVERY {}] rebuilt vector index from oplog ({} vectors)",
                        tenant_id,
                        count
                    );
                }
            }
        }

        Ok(())
    }

    /// Search within a tenant's index.
    ///
    /// # Arguments
    /// * `tenant_id` - Tenant identifier
    /// * `query_text` - Search query string
    /// * `filter` - Optional filter to apply
    /// * `sort` - Optional sort specification
    /// * `limit` - Maximum number of results
    pub fn search(
        &self,
        tenant_id: &str,
        query_text: &str,
        filter: Option<&Filter>,
        sort: Option<&Sort>,
        limit: usize,
    ) -> Result<SearchResult> {
        self.search_with_facets(tenant_id, query_text, filter, sort, limit, 0, None)
    }

    pub fn search_with_facets(
        &self,
        tenant_id: &str,
        query_text: &str,
        filter: Option<&Filter>,
        sort: Option<&Sort>,
        limit: usize,
        offset: usize,
        facets: Option<&[FacetRequest]>,
    ) -> Result<SearchResult> {
        self.search_with_facets_and_distinct(
            tenant_id, query_text, filter, sort, limit, offset, facets, None,
        )
    }

    pub fn search_with_facets_and_distinct(
        &self,
        tenant_id: &str,
        query_text: &str,
        filter: Option<&Filter>,
        sort: Option<&Sort>,
        limit: usize,
        offset: usize,
        facets: Option<&[FacetRequest]>,
        distinct: Option<u32>,
    ) -> Result<SearchResult> {
        self.search_full(
            tenant_id, query_text, filter, sort, limit, offset, facets, distinct, None,
        )
    }

    pub fn search_full(
        &self,
        tenant_id: &str,
        query_text: &str,
        filter: Option<&Filter>,
        sort: Option<&Sort>,
        limit: usize,
        offset: usize,
        facets: Option<&[FacetRequest]>,
        distinct: Option<u32>,
        max_values_per_facet: Option<usize>,
    ) -> Result<SearchResult> {
        self.search_full_with_stop_words(
            tenant_id,
            query_text,
            filter,
            sort,
            limit,
            offset,
            facets,
            distinct,
            max_values_per_facet,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        )
    }

    pub fn search_full_with_stop_words(
        &self,
        tenant_id: &str,
        query_text: &str,
        filter: Option<&Filter>,
        sort: Option<&Sort>,
        limit: usize,
        offset: usize,
        facets: Option<&[FacetRequest]>,
        distinct: Option<u32>,
        max_values_per_facet: Option<usize>,
        remove_stop_words_override: Option<&crate::query::stopwords::RemoveStopWordsValue>,
        ignore_plurals_override: Option<&crate::query::plurals::IgnorePluralsValue>,
        query_languages_override: Option<&Vec<String>>,
        query_type_override: Option<&str>,
        typo_tolerance_override: Option<bool>,
        advanced_syntax_override: Option<bool>,
        remove_words_override: Option<&str>,
        optional_filter_specs: Option<&[(String, String, f32)]>,
        enable_synonyms: Option<bool>,
        enable_rules: Option<bool>,
        rule_contexts: Option<&[String]>,
        restrict_searchable_attrs: Option<&[String]>,
    ) -> Result<SearchResult> {
        let t0 = std::time::Instant::now();
        let index = self.get_or_load(tenant_id)?;
        let t1 = t0.elapsed();
        let reader = index.reader();
        let t2 = t0.elapsed();
        let searcher = reader.searcher();

        let settings = self.get_settings(tenant_id);
        if let Some(ref s) = settings {
            tracing::debug!("[SEARCH] Loaded settings query_type={}", s.query_type);
        }
        let relevance_config = RelevanceConfig {
            searchable_attributes: settings
                .as_ref()
                .and_then(|s| s.searchable_attributes.clone()),
            attribute_weights: HashMap::new(),
        };

        let qt = query_type_override.unwrap_or_else(|| {
            settings
                .as_ref()
                .map(|s| s.query_type.as_str())
                .unwrap_or("prefixLast")
        });
        let effective_stop_words =
            remove_stop_words_override.or(settings.as_ref().map(|s| &s.remove_stop_words));
        let query_text_stopped = match effective_stop_words {
            Some(sw) => crate::query::stopwords::remove_stop_words(query_text, sw, qt),
            None => query_text.to_string(),
        };
        let plural_map: Option<std::collections::HashMap<String, Vec<String>>> = {
            let effective_ignore_plurals =
                ignore_plurals_override.or(settings.as_ref().map(|s| &s.ignore_plurals));
            let effective_query_languages = query_languages_override
                .map(|v| v.as_slice())
                .or(settings.as_ref().map(|s| s.query_languages.as_slice()))
                .unwrap_or(&[]);
            match effective_ignore_plurals {
                Some(ip) if *ip != crate::query::plurals::IgnorePluralsValue::Disabled => {
                    let langs = crate::query::plurals::resolve_plural_languages(
                        ip,
                        effective_query_languages,
                    );
                    if crate::query::plurals::should_expand_english(&langs) {
                        let words: Vec<&str> = query_text_stopped.split_whitespace().collect();
                        let mut map = std::collections::HashMap::new();
                        for w in words {
                            let lower = w.to_lowercase();
                            let forms = crate::query::plurals::expand_plurals(&lower);
                            if forms.len() > 1 {
                                map.insert(lower, forms);
                            }
                        }
                        if map.is_empty() {
                            None
                        } else {
                            Some(map)
                        }
                    } else {
                        None
                    }
                }
                _ => None,
            }
        };
        let query_text = &query_text_stopped;

        let rules_enabled = enable_rules.unwrap_or(true);
        let rule_ctx = rule_contexts.and_then(|c| c.first().map(|s| s.as_str()));
        let (query_text_rewritten, rule_effects) = if rules_enabled {
            if let Some(store) = self.get_rules(tenant_id) {
                let rewritten = store
                    .apply_query_rewrite(query_text, rule_ctx)
                    .unwrap_or_else(|| query_text.to_string());
                let effects = Some(store.apply_rules(&rewritten, rule_ctx));
                (rewritten, effects)
            } else {
                (query_text.to_string(), None)
            }
        } else {
            (query_text.to_string(), None)
        };
        let synonyms_enabled = enable_synonyms.unwrap_or(true);
        let expanded_queries = if synonyms_enabled {
            if let Some(store) = self.get_synonyms(tenant_id) {
                store.expand_query(&query_text_rewritten)
            } else {
                vec![query_text_rewritten.clone()]
            }
        } else {
            vec![query_text_rewritten.clone()]
        };
        let schema = index.inner().schema();

        let json_search_field = schema
            .get_field("_json_search")
            .map_err(|_| FlapjackError::FieldNotFound("_json_search".to_string()))?;

        let searchable_fields = vec![json_search_field];

        let t3 = t0.elapsed();
        let all_searchable_paths: Vec<String> = index.searchable_paths();
        let t4 = t0.elapsed();

        let (searchable_paths, field_weights): (Vec<String>, Vec<f32>) =
            match &relevance_config.searchable_attributes {
                Some(attrs) => {
                    let _attr_set: std::collections::HashSet<&str> =
                        attrs.iter().map(|s| s.as_str()).collect();
                    let mut weighted: Vec<(String, f32)> = Vec::new();
                    let mut unweighted: Vec<String> = Vec::new();

                    for path in &all_searchable_paths {
                        if let Some(pos) = attrs.iter().position(|a| a == path) {
                            weighted.push((path.clone(), 100_f32.powi(-(pos as i32))));
                        } else {
                            unweighted.push(path.clone());
                        }
                    }

                    weighted.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
                    let mut paths: Vec<String> = weighted.iter().map(|(p, _)| p.clone()).collect();
                    let mut weights: Vec<f32> = weighted.iter().map(|(_, w)| *w).collect();

                    let min_weight = weights.last().copied().unwrap_or(1.0) * 0.01;
                    for p in unweighted {
                        paths.push(p);
                        weights.push(min_weight);
                    }

                    (paths, weights)
                }
                None => {
                    let weights = vec![1.0; all_searchable_paths.len()];
                    (all_searchable_paths.clone(), weights)
                }
            };

        // restrictSearchableAttributes: filter to only the specified attributes
        let (searchable_paths, field_weights) = if let Some(restrict) = restrict_searchable_attrs {
            let mut filtered_paths = Vec::new();
            let mut filtered_weights = Vec::new();
            for (i, path) in searchable_paths.iter().enumerate() {
                if restrict.iter().any(|r| r == path) {
                    filtered_paths.push(path.clone());
                    filtered_weights.push(field_weights[i]);
                }
            }
            if filtered_paths.is_empty() {
                (searchable_paths, field_weights)
            } else {
                (filtered_paths, filtered_weights)
            }
        } else {
            (searchable_paths, field_weights)
        };

        let json_exact_field = schema
            .get_field("_json_exact")
            .map_err(|_| FlapjackError::FieldNotFound("_json_exact".to_string()))?;

        // Split/concat alternatives are deferred: only generated if the primary
        // query + synonyms don't return enough results (see loop below).
        let mut expanded_queries = expanded_queries;

        let default_sort_owned = if sort.is_none() && query_text.trim().is_empty() {
            Some(Sort::ByField {
                field: "objectID".to_string(),
                order: crate::types::SortOrder::Desc,
            })
        } else {
            None
        };
        let effective_sort: Option<&Sort> = sort.or(default_sort_owned.as_ref());

        let typo_enabled = typo_tolerance_override.unwrap_or(true);
        let min_word_1_typo = settings
            .as_ref()
            .map(|s| s.min_word_size_for_1_typo as usize)
            .unwrap_or(4);
        let adv_syntax = advanced_syntax_override.unwrap_or(false);

        let parser = QueryParser::new_with_weights(
            &schema,
            searchable_fields,
            field_weights.clone(),
            searchable_paths.clone(),
        )
        .with_exact_field(json_exact_field)
        .with_query_type(qt)
        .with_typo_tolerance(typo_enabled)
        .with_min_word_size_for_1_typo(min_word_1_typo)
        .with_advanced_syntax(adv_syntax)
        .with_plural_map(plural_map);

        // Time-based facet cache: key excludes query_text so consecutive
        // typeahead keystrokes share cached facets (distribution is stable
        // within a short window).  On cache miss we skip the separate
        // prescan and instead piggyback facet collection onto the main
        // search below (1 index scan instead of 2).
        let (facet_cache_key, mut facet_result) = if let Some(facet_reqs) = facets {
            let mut facet_keys: Vec<String> = facet_reqs.iter().map(|r| r.field.clone()).collect();
            facet_keys.sort();
            let filter_hash = filter.map(|f| format!("{:?}", f)).unwrap_or_default();
            let cache_key = format!("{}:{}:{}", tenant_id, filter_hash, facet_keys.join(","));
            let cached_result = self.facet_cache.get(&cache_key).and_then(|cached| {
                let (timestamp, count, facets_map) = cached.as_ref();
                if timestamp.elapsed() < std::time::Duration::from_secs(5) {
                    tracing::debug!(
                        "[FACET_CACHE] HIT ({}ms old)",
                        timestamp.elapsed().as_millis()
                    );
                    let executor = QueryExecutor::new(index.converter(), schema.clone())
                        .with_settings(settings.clone())
                        .with_query(query_text_rewritten.clone())
                        .with_max_values_per_facet(max_values_per_facet);
                    let trimmed = executor.trim_facet_counts(facets_map.clone(), facet_reqs);
                    Some((*count, trimmed))
                } else {
                    tracing::debug!(
                        "[FACET_CACHE] STALE ({}ms old)",
                        timestamp.elapsed().as_millis()
                    );
                    None
                }
            });
            (Some(cache_key), cached_result)
        } else {
            (None, None)
        };

        if limit == 0 {
            let (total, facets_map) = match facet_result {
                Some((count, facets)) => (count, facets),
                None => {
                    let primary_query = crate::types::Query {
                        text: query_text_rewritten.clone(),
                    };
                    let parsed = parser.parse(&primary_query)?;
                    let executor = QueryExecutor::new(index.converter(), schema.clone())
                        .with_settings(settings.clone())
                        .with_query(query_text_rewritten.clone())
                        .with_max_values_per_facet(max_values_per_facet);
                    let expanded = executor.expand_short_query_with_searcher(parsed, &searcher)?;
                    let final_query = executor.apply_filter(expanded, filter)?;
                    if let Some(facet_reqs) = facets {
                        let mut facet_collector =
                            tantivy::collector::FacetCollector::for_field("_facets");
                        for req in facet_reqs {
                            facet_collector.add_facet(&req.path);
                        }
                        let (count, facet_counts) = searcher.search(
                            final_query.as_ref(),
                            &(tantivy::collector::Count, facet_collector),
                        )?;
                        let facets_map = executor.extract_facet_counts(facet_counts, facet_reqs);
                        if let Some(ref key) = facet_cache_key {
                            if self.facet_cache.len()
                                >= self
                                    .facet_cache_cap
                                    .load(std::sync::atomic::Ordering::Relaxed)
                            {
                                if let Some(entry) = self.facet_cache.iter().next() {
                                    let evict_key = entry.key().clone();
                                    drop(entry);
                                    self.facet_cache.remove(&evict_key);
                                }
                            }
                            self.facet_cache.insert(
                                key.clone(),
                                Arc::new((std::time::Instant::now(), count, facets_map.clone())),
                            );
                        }
                        (count, facets_map)
                    } else {
                        let count =
                            searcher.search(final_query.as_ref(), &tantivy::collector::Count)?;
                        (count, HashMap::new())
                    }
                }
            };
            return Ok(SearchResult {
                documents: Vec::new(),
                total,
                facets: facets_map,
                user_data: Vec::new(),
                applied_rules: Vec::new(),
            });
        }

        let mut all_results = Vec::new();
        let mut seen_ids = HashSet::new();
        let mut query_totals: Vec<usize> = Vec::new();

        let effective_limit = limit + offset;
        let mut split_alternatives_generated = false;

        let mut query_idx = 0;
        while query_idx < expanded_queries.len() {
            let expanded_query = &expanded_queries[query_idx];
            let tq0 = std::time::Instant::now();
            let query = crate::types::Query {
                text: expanded_query.clone(),
            };
            let parsed_query = parser.parse(&query)?;
            let tq1 = tq0.elapsed();

            let executor = QueryExecutor::new(index.converter(), schema.clone())
                .with_settings(settings.clone())
                .with_query(expanded_query.clone())
                .with_max_values_per_facet(max_values_per_facet);

            let expanded_parsed =
                executor.expand_short_query_with_searcher(parsed_query, &searcher)?;
            let boosted_query = if let Some(specs) = optional_filter_specs {
                executor.apply_optional_boosts(expanded_parsed, specs)?
            } else {
                expanded_parsed
            };
            let tq2 = tq0.elapsed();
            tracing::debug!(
                "[QUERY_PREP] parse={:?} expand={:?} query='{}'",
                tq1,
                tq2.saturating_sub(tq1),
                expanded_query
            );
            let has_text_query = !expanded_query.trim().is_empty();

            let t5 = t0.elapsed();

            let inline_facets = if query_idx == 0 && facet_result.is_none() {
                facets
            } else {
                None
            };

            let result = executor.execute_with_facets_and_distinct(
                &searcher,
                boosted_query,
                filter,
                effective_sort,
                effective_limit,
                0,
                has_text_query,
                inline_facets,
                if query_idx == 0 { distinct } else { None },
            )?;

            let t6 = t0.elapsed();
            tracing::debug!(
                "[PERF] load={:?} reader={:?} paths={:?} pre_exec={:?} exec={:?}",
                t1,
                t2.saturating_sub(t1),
                t4.saturating_sub(t3),
                t5.saturating_sub(t4),
                t6.saturating_sub(t5)
            );
            if query_idx == 0 && facet_result.is_none() && !result.facets.is_empty() {
                // Cache inline-collected facets for subsequent keystrokes
                if let Some(ref key) = facet_cache_key {
                    if self.facet_cache.len()
                        >= self
                            .facet_cache_cap
                            .load(std::sync::atomic::Ordering::Relaxed)
                    {
                        if let Some(entry) = self.facet_cache.iter().next() {
                            let evict_key = entry.key().clone();
                            drop(entry);
                            self.facet_cache.remove(&evict_key);
                        }
                    }
                    self.facet_cache.insert(
                        key.clone(),
                        Arc::new((
                            std::time::Instant::now(),
                            result.total,
                            result.facets.clone(),
                        )),
                    );
                }
                facet_result = Some((result.total, result.facets));
            }

            // Track total from this query for final total calculation
            query_totals.push(result.total);

            for doc in result.documents {
                if seen_ids.insert(doc.document.id.clone()) {
                    all_results.push(doc);
                }
            }

            query_idx += 1;

            // Early exit: if we already have enough results, skip remaining variants.
            if all_results.len() >= effective_limit {
                break;
            }

            // Lazy split/concat alternatives: only generate if primary queries
            // (original + synonyms) didn't produce enough results.
            if query_idx == expanded_queries.len()
                && !split_alternatives_generated
                && !query_text.trim().is_empty()
                && all_results.len() < effective_limit
            {
                split_alternatives_generated = true;
                let base_queries = expanded_queries.clone();
                for eq in &base_queries {
                    let alts = crate::query::splitting::generate_alternatives(
                        eq,
                        &searcher,
                        json_exact_field,
                        &searchable_paths,
                    );
                    for alt in alts {
                        if !expanded_queries.contains(&alt) {
                            expanded_queries.push(alt);
                        }
                    }
                    if expanded_queries.len() >= 15 {
                        break;
                    }
                }
            }
        }

        all_results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let result_count = all_results.len();

        // Calculate total based on whether synonyms were expanded
        let mut total = if query_totals.len() == 1 {
            // Single query: use its total directly
            query_totals[0]
        } else {
            // Multiple queries (synonym expansion): use actual unique doc count
            // if we collected all results, otherwise estimate with max
            if result_count < effective_limit {
                // We got all unique documents
                result_count
            } else {
                // Hit the limit, might have missed some. Use max as estimate.
                // This is not perfect but better than summing (which double-counts).
                query_totals.iter().copied().max().unwrap_or(result_count)
            }
        };
        let start = offset.min(result_count);
        let end = (start + limit).min(result_count);
        let page_results = all_results[start..end].to_vec();

        let (final_docs, user_data, applied_rules) = if let Some(ref effects) = rule_effects {
            // Apply rules (pins/hides) after synonym expansion so they
            // operate on the full merged result set.
            let executor = QueryExecutor::new(index.converter(), schema.clone())
                .with_settings(settings.clone())
                .with_query(query_text.to_string())
                .with_max_values_per_facet(max_values_per_facet);
            let docs = executor.apply_rules_to_results(&searcher, page_results, effects)?;
            // Adjust total for hidden docs that matched the query
            let hidden_count = effects
                .hidden
                .iter()
                .filter(|id| all_results.iter().any(|d| &d.document.id == *id))
                .count();
            total = total.saturating_sub(hidden_count);
            (
                docs,
                effects.user_data.clone(),
                effects.applied_rules.clone(),
            )
        } else {
            (page_results, Vec::new(), Vec::new())
        };

        // removeWordsIfNoResults: retry with fewer words if we got 0 results
        let remove_strategy = remove_words_override
            .or(settings
                .as_ref()
                .map(|s| s.remove_words_if_no_results.as_str()))
            .unwrap_or("none");

        if total == 0
            && final_docs.is_empty()
            && remove_strategy != "none"
            && !query_text.trim().is_empty()
        {
            let words: Vec<&str> = query_text.split_whitespace().collect();
            if words.len() > 1 {
                let fallback_queries: Vec<String> = match remove_strategy {
                    "lastWords" => (1..words.len())
                        .map(|drop| words[..words.len() - drop].join(" "))
                        .collect(),
                    "firstWords" => (1..words.len())
                        .map(|drop| words[drop..].join(" "))
                        .collect(),
                    _ => vec![],
                };
                for fallback_q in fallback_queries {
                    if let Ok(retry) = self.search_full_with_stop_words(
                        tenant_id,
                        &fallback_q,
                        filter,
                        sort,
                        limit,
                        offset,
                        facets,
                        distinct,
                        max_values_per_facet,
                        remove_stop_words_override,
                        ignore_plurals_override,
                        query_languages_override,
                        query_type_override,
                        typo_tolerance_override,
                        advanced_syntax_override,
                        Some("none"), // prevent recursion
                        optional_filter_specs,
                        enable_synonyms,
                        enable_rules,
                        rule_contexts,
                        restrict_searchable_attrs,
                    ) {
                        if retry.total > 0 {
                            return Ok(retry);
                        }
                    }
                }
            }
        }

        let facets_map = match facet_result {
            Some((_, facets)) => facets,
            None => HashMap::new(),
        };

        Ok(SearchResult {
            documents: final_docs,
            total,
            facets: facets_map,
            user_data,
            applied_rules,
        })
    }

    /// Get or create a write queue for the given tenant.
    ///
    /// DRY helper — all write paths (add, delete, compact) go through this.
    /// Handles oplog creation, write queue spawning, and vector context setup.
    fn get_or_create_write_queue(&self, tenant_id: &str, index: &Arc<Index>) -> WriteQueue {
        self.write_queues
            .entry(tenant_id.to_string())
            .or_insert_with(|| {
                let oplog = self.get_or_create_oplog(tenant_id);
                #[cfg(feature = "vector-search")]
                let vector_ctx = VectorWriteContext::new(Arc::clone(&self.vector_indices));
                #[cfg(not(feature = "vector-search"))]
                let vector_ctx = VectorWriteContext::new();
                let (queue, handle) = create_write_queue(
                    tenant_id.to_string(),
                    Arc::clone(index),
                    Arc::clone(&self.writers),
                    Arc::clone(&self.tasks),
                    self.base_path.clone(),
                    oplog,
                    Arc::clone(&self.facet_cache),
                    Arc::clone(&self.lww_map),
                    vector_ctx,
                );
                self.write_task_handles
                    .insert(tenant_id.to_string(), handle);
                queue
            })
            .clone()
    }

    /// Add documents to a tenant's index.
    ///
    /// Creates a writer, adds documents, and commits immediately.
    /// For production, this should be batched via background commit thread.
    pub fn add_documents_insert(&self, tenant_id: &str, docs: Vec<Document>) -> Result<TaskInfo> {
        self.add_documents_inner(tenant_id, docs, false, false)
    }

    pub fn add_documents(&self, tenant_id: &str, docs: Vec<Document>) -> Result<TaskInfo> {
        self.add_documents_inner(tenant_id, docs, true, false)
    }

    /// Like `add_documents` but uses `UpsertNoLwwUpdate` so the write_queue does NOT
    /// overwrite lww_map entries — for use by replication (apply_ops_to_manager) which
    /// has already recorded the correct op timestamp in lww_map before calling this.
    pub fn add_documents_for_replication(
        &self,
        tenant_id: &str,
        docs: Vec<Document>,
    ) -> Result<TaskInfo> {
        self.add_documents_inner(tenant_id, docs, true, true)
    }

    fn add_documents_inner(
        &self,
        tenant_id: &str,
        docs: Vec<Document>,
        upsert: bool,
        no_lww_update: bool,
    ) -> Result<TaskInfo> {
        let index = self.get_or_load(tenant_id)?;

        let numeric_id = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;
        let task_id = format!("task_{}_{}", tenant_id, uuid::Uuid::new_v4());
        let task = TaskInfo::new(task_id.clone(), numeric_id, docs.len());
        self.tasks.insert(task_id.clone(), task.clone());
        self.tasks.insert(numeric_id.to_string(), task.clone());

        self.evict_old_tasks(tenant_id, MAX_TASKS_PER_TENANT);

        let tx = self.get_or_create_write_queue(tenant_id, &index);

        let actions = if upsert {
            if no_lww_update {
                docs.into_iter()
                    .map(WriteAction::UpsertNoLwwUpdate)
                    .collect()
            } else {
                docs.into_iter().map(WriteAction::Upsert).collect()
            }
        } else {
            docs.into_iter().map(WriteAction::Add).collect()
        };
        if tx
            .try_send(WriteOp {
                task_id: task_id.clone(),
                actions,
            })
            .is_err()
        {
            self.tasks.alter(&task_id, |_, mut t| {
                t.status = TaskStatus::Failed("Queue full".to_string());
                t
            });
            return Err(FlapjackError::QueueFull);
        }

        Ok(task)
    }

    pub fn delete_documents(&self, tenant_id: &str, object_ids: Vec<String>) -> Result<TaskInfo> {
        let index = self.get_or_load(tenant_id)?;

        let numeric_id = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;
        let task_id = format!("task_{}_{}", tenant_id, uuid::Uuid::new_v4());
        let task = TaskInfo::new(task_id.clone(), numeric_id, object_ids.len());
        self.tasks.insert(task_id.clone(), task.clone());
        self.tasks.insert(numeric_id.to_string(), task.clone());

        self.evict_old_tasks(tenant_id, MAX_TASKS_PER_TENANT);

        let tx = self.get_or_create_write_queue(tenant_id, &index);

        let actions = object_ids.into_iter().map(WriteAction::Delete).collect();
        if tx
            .try_send(WriteOp {
                task_id: task_id.clone(),
                actions,
            })
            .is_err()
        {
            self.tasks.alter(&task_id, |_, mut t| {
                t.status = TaskStatus::Failed("Queue full".to_string());
                t
            });
            return Err(FlapjackError::QueueFull);
        }

        Ok(task)
    }

    /// Like `delete_documents` but uses `DeleteNoLwwUpdate` — for replication paths where
    /// apply_ops_to_manager has already recorded the correct op timestamp in lww_map.
    pub fn delete_documents_for_replication(
        &self,
        tenant_id: &str,
        object_ids: Vec<String>,
    ) -> Result<TaskInfo> {
        let index = self.get_or_load(tenant_id)?;

        let numeric_id = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;
        let task_id = format!("task_{}_{}", tenant_id, uuid::Uuid::new_v4());
        let task = TaskInfo::new(task_id.clone(), numeric_id, object_ids.len());
        self.tasks.insert(task_id.clone(), task.clone());
        self.tasks.insert(numeric_id.to_string(), task.clone());

        self.evict_old_tasks(tenant_id, MAX_TASKS_PER_TENANT);

        let tx = self.get_or_create_write_queue(tenant_id, &index);

        let actions = object_ids
            .into_iter()
            .map(WriteAction::DeleteNoLwwUpdate)
            .collect();
        if tx
            .try_send(WriteOp {
                task_id: task_id.clone(),
                actions,
            })
            .is_err()
        {
            self.tasks.alter(&task_id, |_, mut t| {
                t.status = TaskStatus::Failed("Queue full".to_string());
                t
            });
            return Err(FlapjackError::QueueFull);
        }

        Ok(task)
    }

    /// Compact an index by merging all segments and garbage-collecting stale files.
    ///
    /// This reclaims disk space from deleted documents. The operation is
    /// enqueued on the write queue so it serialises with other writes.
    pub fn compact_index(&self, tenant_id: &str) -> Result<TaskInfo> {
        let index = self.get_or_load(tenant_id)?;

        let numeric_id = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;
        let task_id = format!("task_{}_{}", tenant_id, uuid::Uuid::new_v4());
        let task = TaskInfo::new(task_id.clone(), numeric_id, 0);
        self.tasks.insert(task_id.clone(), task.clone());
        self.tasks.insert(numeric_id.to_string(), task.clone());

        let tx = self.get_or_create_write_queue(tenant_id, &index);

        if tx
            .try_send(WriteOp {
                task_id: task_id.clone(),
                actions: vec![WriteAction::Compact],
            })
            .is_err()
        {
            self.tasks.alter(&task_id, |_, mut t| {
                t.status = TaskStatus::Failed("Queue full".to_string());
                t
            });
            return Err(FlapjackError::QueueFull);
        }

        Ok(task)
    }

    /// Compact an index and wait for the operation to complete.
    pub async fn compact_index_sync(&self, tenant_id: &str) -> Result<()> {
        let task = self.compact_index(tenant_id)?;

        loop {
            let status = self.get_task(&task.id)?;
            match status.status {
                TaskStatus::Enqueued | TaskStatus::Processing => {
                    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                }
                TaskStatus::Succeeded => return Ok(()),
                TaskStatus::Failed(e) => return Err(FlapjackError::Tantivy(e)),
            }
        }
    }

    /// Unload a tenant's index from memory.
    ///
    /// Removes the index from the cache, closing all file handles.
    /// Required before export/migration to ensure clean state.
    pub fn unload(&self, tenant_id: &TenantId) -> Result<()> {
        self.invalidate_facet_cache(tenant_id);
        self.write_queues.remove(tenant_id);
        self.writers.remove(tenant_id);
        self.oplogs.remove(tenant_id);
        self.loaded.remove(tenant_id);
        self.settings_cache.remove(tenant_id);
        self.rules_cache.remove(tenant_id);
        self.synonyms_cache.remove(tenant_id);
        Ok(())
    }

    pub async fn delete_tenant(&self, tenant_id: &TenantId) -> Result<()> {
        self.invalidate_facet_cache(tenant_id);
        self.write_queues.remove(tenant_id);

        if let Some((_, handle)) = self.write_task_handles.remove(tenant_id) {
            let _ = handle.await;
        }

        self.writers.remove(tenant_id);
        self.oplogs.remove(tenant_id);
        self.loaded.remove(tenant_id);
        self.settings_cache.remove(tenant_id);
        self.rules_cache.remove(tenant_id);
        self.synonyms_cache.remove(tenant_id);

        let path = self.base_path.join(tenant_id);
        if path.exists() {
            // Retry remove_dir_all to handle Tantivy merge threads that may still
            // be writing segment files after the IndexWriter is dropped. The drop
            // signals merge threads to stop but doesn't wait for them to finish.
            let mut last_err = None;
            for _ in 0..10 {
                match std::fs::remove_dir_all(&path) {
                    Ok(()) => {
                        last_err = None;
                        break;
                    }
                    Err(e) => {
                        last_err = Some(e);
                        std::thread::sleep(std::time::Duration::from_millis(50));
                    }
                }
            }
            if let Some(e) = last_err {
                return Err(e.into());
            }
        }
        Ok(())
    }

    pub fn export_tenant(&self, tenant_id: &TenantId, dest_path: PathBuf) -> Result<String> {
        let numeric_id = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;
        let task_id = format!("export_{}_{}", tenant_id, uuid::Uuid::new_v4());
        let task = TaskInfo::new(task_id.clone(), numeric_id, 0);
        self.tasks.insert(task_id.clone(), task.clone());
        self.tasks.insert(numeric_id.to_string(), task.clone());

        let tenant_id_clone = tenant_id.clone();
        let sender = self.task_queue.sender.clone();
        let task_id_clone = task_id.clone();

        tokio::spawn(async move {
            let _ = sender
                .send(crate::index::task_queue::TaskCommand::Export {
                    task_id: task_id_clone,
                    tenant_id: tenant_id_clone,
                    dest_path,
                })
                .await;
        });

        Ok(task_id)
    }

    pub async fn export_tenant_wait(&self, tenant_id: &TenantId, dest_path: PathBuf) -> Result<()> {
        let task_id = self.export_tenant(tenant_id, dest_path)?;

        loop {
            let status = self.get_task(&task_id)?;
            match status.status {
                TaskStatus::Enqueued | TaskStatus::Processing => {
                    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
                }
                TaskStatus::Succeeded => return Ok(()),
                TaskStatus::Failed(e) => return Err(FlapjackError::Tantivy(e)),
            }
        }
    }

    /// Import a tenant's index from a source path.
    ///
    /// Copies the directory to the base path under the tenant ID.
    /// Does NOT load the index (caller must call get_or_load).
    pub fn import_tenant(&self, tenant_id: &TenantId, src_path: &Path) -> Result<()> {
        let dest_path = self.base_path.join(tenant_id);
        std::fs::create_dir_all(&dest_path)?;

        copy_dir_recursive(src_path, &dest_path)?;

        Ok(())
    }

    /// Get the number of loaded indexes.
    ///
    /// Useful for monitoring and debugging.
    pub fn loaded_count(&self) -> usize {
        self.loaded.len()
    }

    /// Return the total disk usage in bytes for a single tenant's data directory.
    ///
    /// Returns 0 if the tenant directory does not exist.
    pub fn tenant_storage_bytes(&self, tenant_id: &str) -> u64 {
        let path = self.base_path.join(tenant_id);
        crate::index::storage_size::dir_size_bytes(&path).unwrap_or(0)
    }

    /// Return the document count for a loaded tenant's index.
    ///
    /// Reads Tantivy segment metadata (in-memory, fast). Returns `None` if
    /// the tenant is not currently loaded.
    pub fn tenant_doc_count(&self, tenant_id: &str) -> Option<u64> {
        let index = self.loaded.get(tenant_id)?;
        let reader = index.reader();
        let searcher = reader.searcher();
        let count: u64 = searcher
            .segment_readers()
            .iter()
            .map(|r| r.num_docs() as u64)
            .sum();
        Some(count)
    }

    /// Return the IDs of all currently loaded tenants.
    ///
    /// Needed by the metrics handler since `loaded` is `pub(crate)`.
    pub fn loaded_tenant_ids(&self) -> Vec<String> {
        self.loaded
            .iter()
            .map(|entry| entry.key().clone())
            .collect()
    }

    /// Return (tenant_id, current_oplog_seq) pairs for all tenants with a loaded oplog.
    ///
    /// Uses `get_oplog()` (not `get_or_create_oplog()`) to avoid side effects.
    pub fn all_tenant_oplog_seqs(&self) -> Vec<(String, u64)> {
        self.loaded
            .iter()
            .filter_map(|entry| {
                let tid = entry.key().clone();
                self.get_oplog(&tid).map(|oplog| (tid, oplog.current_seq()))
            })
            .collect()
    }

    /// Return disk usage in bytes for every loaded tenant.
    pub fn all_tenant_storage(&self) -> Vec<(String, u64)> {
        self.loaded
            .iter()
            .map(|entry| {
                let tid = entry.key().clone();
                let bytes = self.tenant_storage_bytes(&tid);
                (tid, bytes)
            })
            .collect()
    }

    pub async fn add_documents_insert_sync(
        &self,
        tenant_id: &str,
        docs: Vec<Document>,
    ) -> Result<()> {
        let task = self.add_documents_insert(tenant_id, docs)?;

        loop {
            let status = self.get_task(&task.id)?;
            match status.status {
                TaskStatus::Enqueued | TaskStatus::Processing => {
                    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                }
                TaskStatus::Succeeded => return Ok(()),
                TaskStatus::Failed(e) => return Err(FlapjackError::Tantivy(e)),
            }
        }
    }

    pub async fn add_documents_sync(&self, tenant_id: &str, docs: Vec<Document>) -> Result<()> {
        let task = self.add_documents(tenant_id, docs)?;

        loop {
            let status = self.get_task(&task.id)?;
            match status.status {
                TaskStatus::Enqueued | TaskStatus::Processing => {
                    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                }
                TaskStatus::Succeeded => return Ok(()),
                TaskStatus::Failed(e) => return Err(FlapjackError::Tantivy(e)),
            }
        }
    }

    pub async fn delete_documents_sync(
        &self,
        tenant_id: &str,
        object_ids: Vec<String>,
    ) -> Result<()> {
        let task = self.delete_documents(tenant_id, object_ids)?;

        loop {
            let status = self.get_task(&task.id)?;
            match status.status {
                TaskStatus::Enqueued | TaskStatus::Processing => {
                    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                }
                TaskStatus::Succeeded => return Ok(()),
                TaskStatus::Failed(e) => return Err(FlapjackError::Tantivy(e)),
            }
        }
    }

    /// Like `delete_documents_sync` but skips lww_map update in write_queue — for replication.
    pub async fn delete_documents_sync_for_replication(
        &self,
        tenant_id: &str,
        object_ids: Vec<String>,
    ) -> Result<()> {
        let task = self.delete_documents_for_replication(tenant_id, object_ids)?;

        loop {
            let status = self.get_task(&task.id)?;
            match status.status {
                TaskStatus::Enqueued | TaskStatus::Processing => {
                    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                }
                TaskStatus::Succeeded => return Ok(()),
                TaskStatus::Failed(e) => return Err(FlapjackError::Tantivy(e)),
            }
        }
    }

    pub async fn move_index(&self, source: &str, destination: &str) -> Result<TaskInfo> {
        let src_path = self.base_path.join(source);
        if !src_path.exists() {
            return self.make_noop_task(source);
        }

        self.unload(&source.to_string())?;

        if self.loaded.contains_key(destination) {
            self.delete_tenant(&destination.to_string()).await?;
        } else {
            let dest_path = self.base_path.join(destination);
            if dest_path.exists() {
                std::fs::remove_dir_all(&dest_path)?;
            }
        }

        let dest_path = self.base_path.join(destination);
        std::fs::rename(&src_path, &dest_path)?;

        self.make_noop_task(destination)
    }

    pub async fn copy_index(
        &self,
        source: &str,
        destination: &str,
        scope: Option<&[String]>,
    ) -> Result<TaskInfo> {
        let src_path = self.base_path.join(source);

        if self.loaded.contains_key(destination) {
            self.delete_tenant(&destination.to_string()).await?;
        } else {
            let dest_path = self.base_path.join(destination);
            if dest_path.exists() {
                std::fs::remove_dir_all(&dest_path)?;
            }
        }

        if !src_path.exists() {
            self.create_tenant(destination)?;
            return self.make_noop_task(destination);
        }

        let dest_path = self.base_path.join(destination);

        match scope {
            None => {
                std::fs::create_dir_all(&dest_path)?;
                copy_dir_recursive(&src_path, &dest_path)?;
            }
            Some(scopes) => {
                self.create_tenant(destination)?;
                for s in scopes {
                    let filename = match s.as_str() {
                        "settings" => "settings.json",
                        "synonyms" => "synonyms.json",
                        "rules" => "rules.json",
                        _ => continue,
                    };
                    let src_file = src_path.join(filename);
                    if src_file.exists() {
                        std::fs::copy(&src_file, dest_path.join(filename))?;
                    }
                }
            }
        }

        self.make_noop_task(destination)
    }

    pub fn make_noop_task(&self, index_name: &str) -> Result<TaskInfo> {
        let numeric_id = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;
        let task_id = format!("task_{}_{}", index_name, uuid::Uuid::new_v4());
        let mut task = TaskInfo::new(task_id.clone(), numeric_id, 0);
        task.status = TaskStatus::Succeeded;
        self.tasks.insert(task_id.clone(), task.clone());
        self.tasks.insert(numeric_id.to_string(), task.clone());
        Ok(task)
    }

    pub fn get_or_create_oplog(&self, tenant_id: &str) -> Option<Arc<OpLog>> {
        let entry = self
            .oplogs
            .entry(tenant_id.to_string())
            .or_try_insert_with(|| {
                let oplog_dir = self.base_path.join(tenant_id).join("oplog");
                let node_id =
                    std::env::var("FLAPJACK_NODE_ID").unwrap_or_else(|_| "unknown".to_string());
                OpLog::open(&oplog_dir, tenant_id, &node_id)
                    .map(Arc::new)
                    .map_err(|e| {
                        tracing::error!("[OPLOG {}] open failed: {}", tenant_id, e);
                        e
                    })
            });
        match entry {
            Ok(e) => Some(Arc::clone(&e)),
            Err(_) => None,
        }
    }

    pub fn append_oplog(&self, tenant_id: &str, op_type: &str, payload: serde_json::Value) {
        if let Some(ol) = self.get_or_create_oplog(tenant_id) {
            if let Err(e) = ol.append(op_type, payload) {
                tracing::error!("[OPLOG {}] append failed: {}", tenant_id, e);
            }
        }
    }

    pub fn get_document(&self, tenant_id: &str, object_id: &str) -> Result<Option<Document>> {
        let index = self.get_or_load(tenant_id)?;
        let reader = index.reader();
        let searcher = reader.searcher();
        let schema = index.inner().schema();

        let id_field = schema
            .get_field("_id")
            .map_err(|_| FlapjackError::FieldNotFound("_id".to_string()))?;

        let term = tantivy::Term::from_field_text(id_field, object_id);
        let term_query =
            tantivy::query::TermQuery::new(term, tantivy::schema::IndexRecordOption::Basic);

        let top_docs = searcher.search(&term_query, &tantivy::collector::TopDocs::with_limit(1))?;

        if top_docs.is_empty() {
            return Ok(None);
        }

        let doc_address = top_docs[0].1;
        let retrieved_doc = searcher.doc(doc_address)?;

        let document =
            index
                .converter()
                .from_tantivy(retrieved_doc, &schema, object_id.to_string())?;
        Ok(Some(document))
    }

    /// Gracefully shut down all write queues, flushing pending writes.
    ///
    /// Drops all write queue senders (triggering final batch flush in each
    /// write task), then awaits every write task handle to completion.
    pub async fn graceful_shutdown(&self) {
        // Drop all senders — receivers will get None and flush pending ops
        self.write_queues.clear();

        // Drain and await all write task handles
        let handles: Vec<_> = self
            .write_task_handles
            .iter()
            .map(|r| r.key().clone())
            .collect();
        for tenant_id in handles {
            if let Some((_, handle)) = self.write_task_handles.remove(&tenant_id) {
                match handle.await {
                    Ok(Ok(())) => {
                        tracing::info!("[shutdown] Write queue for '{}' drained", tenant_id);
                    }
                    Ok(Err(e)) => {
                        tracing::error!(
                            "[shutdown] Write queue for '{}' exited with error: {}",
                            tenant_id,
                            e
                        );
                    }
                    Err(e) => {
                        tracing::error!(
                            "[shutdown] Write queue task for '{}' panicked: {}",
                            tenant_id,
                            e
                        );
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn tenant_doc_count_returns_correct_count() {
        let tmp = TempDir::new().unwrap();
        let manager = IndexManager::new(tmp.path());
        manager.create_tenant("t1").unwrap();

        let docs = vec![
            Document {
                id: "d1".to_string(),
                fields: HashMap::from([(
                    "name".to_string(),
                    crate::types::FieldValue::Text("Alice".to_string()),
                )]),
            },
            Document {
                id: "d2".to_string(),
                fields: HashMap::from([(
                    "name".to_string(),
                    crate::types::FieldValue::Text("Bob".to_string()),
                )]),
            },
            Document {
                id: "d3".to_string(),
                fields: HashMap::from([(
                    "name".to_string(),
                    crate::types::FieldValue::Text("Carol".to_string()),
                )]),
            },
        ];
        manager.add_documents_sync("t1", docs).await.unwrap();

        let count = manager.tenant_doc_count("t1");
        assert_eq!(count, Some(3), "should have 3 docs after adding 3");
    }

    #[tokio::test]
    async fn tenant_doc_count_returns_none_for_unloaded() {
        let tmp = TempDir::new().unwrap();
        let manager = IndexManager::new(tmp.path());
        assert_eq!(manager.tenant_doc_count("nonexistent"), None);
    }

    #[tokio::test]
    async fn loaded_tenant_ids_returns_correct_ids() {
        let tmp = TempDir::new().unwrap();
        let manager = IndexManager::new(tmp.path());
        manager.create_tenant("alpha").unwrap();
        manager.create_tenant("beta").unwrap();

        let mut ids = manager.loaded_tenant_ids();
        ids.sort();
        assert_eq!(ids, vec!["alpha", "beta"]);
    }

    #[tokio::test]
    async fn loaded_tenant_ids_empty_when_no_tenants() {
        let tmp = TempDir::new().unwrap();
        let manager = IndexManager::new(tmp.path());
        assert!(manager.loaded_tenant_ids().is_empty());
    }

    #[tokio::test]
    async fn all_tenant_oplog_seqs_returns_seqs_after_writes() {
        let tmp = TempDir::new().unwrap();
        let manager = IndexManager::new(tmp.path());
        manager.create_tenant("t1").unwrap();

        let docs = vec![Document {
            id: "d1".to_string(),
            fields: HashMap::from([(
                "name".to_string(),
                crate::types::FieldValue::Text("Alice".to_string()),
            )]),
        }];
        manager.add_documents_sync("t1", docs).await.unwrap();

        let seqs = manager.all_tenant_oplog_seqs();
        assert!(!seqs.is_empty(), "should have at least one entry");
        let (tid, seq) = &seqs[0];
        assert_eq!(tid, "t1");
        assert!(*seq > 0, "seq should be > 0 after a write");
    }

    // ── Vector index store tests (6.11) ──

    #[cfg(feature = "vector-search")]
    #[tokio::test]
    async fn test_vector_index_store_and_retrieve() {
        use usearch::ffi::MetricKind;
        let tmp = TempDir::new().unwrap();
        let manager = IndexManager::new(tmp.path());

        let vi = crate::vector::index::VectorIndex::new(3, MetricKind::Cos).unwrap();
        manager.set_vector_index("tenant1", vi);

        let retrieved = manager.get_vector_index("tenant1");
        assert!(retrieved.is_some());
        let lock = retrieved.unwrap();
        let guard = lock.read().unwrap();
        assert_eq!(guard.dimensions(), 3);
    }

    #[cfg(feature = "vector-search")]
    #[tokio::test]
    async fn test_vector_index_missing_returns_none() {
        let tmp = TempDir::new().unwrap();
        let manager = IndexManager::new(tmp.path());
        assert!(manager.get_vector_index("nonexistent").is_none());
    }

    #[cfg(feature = "vector-search")]
    #[tokio::test]
    async fn test_vector_index_search_through_manager() {
        use usearch::ffi::MetricKind;
        let tmp = TempDir::new().unwrap();
        let manager = IndexManager::new(tmp.path());

        let mut vi = crate::vector::index::VectorIndex::new(3, MetricKind::Cos).unwrap();
        vi.add("doc1", &[1.0, 0.0, 0.0]).unwrap();
        vi.add("doc2", &[0.0, 1.0, 0.0]).unwrap();
        vi.add("doc3", &[0.0, 0.0, 1.0]).unwrap();
        manager.set_vector_index("t1", vi);

        let lock = manager.get_vector_index("t1").unwrap();
        let guard = lock.read().unwrap();
        let results = guard.search(&[1.0, 0.0, 0.0], 2).unwrap();
        assert!(!results.is_empty());
        assert_eq!(results[0].doc_id, "doc1");
    }

    // ── Multi-tenant vector isolation test ──

    #[cfg(feature = "vector-search")]
    #[tokio::test]
    async fn test_vector_tenant_isolation() {
        use usearch::ffi::MetricKind;
        let tmp = TempDir::new().unwrap();
        let manager = IndexManager::new(tmp.path());

        // Tenant A: 3-dim vectors about "cats"
        let mut vi_a = crate::vector::index::VectorIndex::new(3, MetricKind::Cos).unwrap();
        vi_a.add("cat1", &[1.0, 0.0, 0.0]).unwrap();
        vi_a.add("cat2", &[0.9, 0.1, 0.0]).unwrap();
        vi_a.add("cat3", &[0.8, 0.2, 0.0]).unwrap();
        manager.set_vector_index("tenant_a", vi_a);

        // Tenant B: 3-dim vectors about "dogs" (orthogonal direction)
        let mut vi_b = crate::vector::index::VectorIndex::new(3, MetricKind::Cos).unwrap();
        vi_b.add("dog1", &[0.0, 0.0, 1.0]).unwrap();
        vi_b.add("dog2", &[0.0, 0.1, 0.9]).unwrap();
        manager.set_vector_index("tenant_b", vi_b);

        // Search tenant A — must only return tenant A's docs
        {
            let lock = manager.get_vector_index("tenant_a").unwrap();
            let guard = lock.read().unwrap();
            let results = guard.search(&[1.0, 0.0, 0.0], 10).unwrap();
            assert_eq!(results.len(), 3, "tenant_a should have exactly 3 vectors");
            for r in &results {
                assert!(
                    r.doc_id.starts_with("cat"),
                    "tenant_a search returned '{}' which belongs to tenant_b",
                    r.doc_id
                );
            }
        }

        // Search tenant B — must only return tenant B's docs
        {
            let lock = manager.get_vector_index("tenant_b").unwrap();
            let guard = lock.read().unwrap();
            let results = guard.search(&[0.0, 0.0, 1.0], 10).unwrap();
            assert_eq!(results.len(), 2, "tenant_b should have exactly 2 vectors");
            for r in &results {
                assert!(
                    r.doc_id.starts_with("dog"),
                    "tenant_b search returned '{}' which belongs to tenant_a",
                    r.doc_id
                );
            }
        }

        // Verify tenant C (nonexistent) returns None
        assert!(
            manager.get_vector_index("tenant_c").is_none(),
            "nonexistent tenant should return None"
        );

        // Delete tenant A's index, verify tenant B is unaffected
        manager.vector_indices.remove("tenant_a");
        assert!(manager.get_vector_index("tenant_a").is_none());
        {
            let lock = manager.get_vector_index("tenant_b").unwrap();
            let guard = lock.read().unwrap();
            assert_eq!(
                guard.len(),
                2,
                "tenant_b should be unaffected by tenant_a removal"
            );
        }
    }

    #[tokio::test]
    async fn all_tenant_oplog_seqs_empty_when_no_oplogs() {
        let tmp = TempDir::new().unwrap();
        let manager = IndexManager::new(tmp.path());
        // Create tenant but don't write anything (no oplog created)
        manager.create_tenant("t1").unwrap();
        let seqs = manager.all_tenant_oplog_seqs();
        assert!(seqs.is_empty(), "no oplog loaded means empty result");
    }

    // ── Vector index load-on-open tests (8.4) ──

    #[cfg(feature = "vector-search")]
    #[tokio::test]
    async fn test_load_vector_index_on_get_or_load() {
        use usearch::ffi::MetricKind;
        let tmp = TempDir::new().unwrap();
        let tenant_id = "load_vec_t";
        let tenant_path = tmp.path().join(tenant_id);

        // Create a Tantivy index on disk
        std::fs::create_dir_all(&tenant_path).unwrap();
        {
            let schema = crate::index::schema::Schema::builder().build();
            let _ = crate::index::Index::create(&tenant_path, schema).unwrap();
        }

        // Save settings with an embedder so load_vector_index proceeds past the
        // "no embedders configured" guard (added in 8.19).
        let settings = crate::index::settings::IndexSettings {
            embedders: Some(std::collections::HashMap::from([(
                "default".to_string(),
                serde_json::json!({
                    "source": "userProvided",
                    "dimensions": 3
                }),
            )])),
            ..Default::default()
        };
        settings.save(&tenant_path.join("settings.json")).unwrap();

        // Manually save a VectorIndex with 3 docs (no fingerprint file → backward compat load)
        let mut vi = crate::vector::index::VectorIndex::new(3, MetricKind::Cos).unwrap();
        vi.add("doc1", &[1.0, 0.0, 0.0]).unwrap();
        vi.add("doc2", &[0.0, 1.0, 0.0]).unwrap();
        vi.add("doc3", &[0.0, 0.0, 1.0]).unwrap();
        vi.save(&tenant_path.join("vectors")).unwrap();

        // Create IndexManager and get_or_load
        let manager = IndexManager::new(tmp.path());
        manager.get_or_load(tenant_id).unwrap();

        // Verify VectorIndex was loaded from disk
        let vi_arc = manager.get_vector_index(tenant_id);
        assert!(vi_arc.is_some(), "VectorIndex should be loaded from disk");
        let vi_arc = vi_arc.unwrap();
        let guard = vi_arc.read().unwrap();
        assert_eq!(guard.len(), 3);
        assert_eq!(guard.dimensions(), 3);

        // Verify it's searchable
        let results = guard.search(&[1.0, 0.0, 0.0], 1).unwrap();
        assert_eq!(results[0].doc_id, "doc1");
    }

    #[cfg(feature = "vector-search")]
    #[tokio::test]
    async fn test_load_no_vectors_dir_ok() {
        let tmp = TempDir::new().unwrap();
        let tenant_id = "novecdir_t";
        let tenant_path = tmp.path().join(tenant_id);

        std::fs::create_dir_all(&tenant_path).unwrap();
        {
            let schema = crate::index::schema::Schema::builder().build();
            let _ = crate::index::Index::create(&tenant_path, schema).unwrap();
        }

        let manager = IndexManager::new(tmp.path());
        manager.get_or_load(tenant_id).unwrap();

        // No VectorIndex should be loaded
        assert!(
            manager.get_vector_index(tenant_id).is_none(),
            "get_vector_index should return None when no vectors/ dir exists"
        );
    }

    #[cfg(feature = "vector-search")]
    #[tokio::test]
    async fn test_load_corrupted_vector_index_logs_warning() {
        let tmp = TempDir::new().unwrap();
        let tenant_id = "corrupt_vec_t";
        let tenant_path = tmp.path().join(tenant_id);

        std::fs::create_dir_all(&tenant_path).unwrap();
        {
            let schema = crate::index::schema::Schema::builder().build();
            let _ = crate::index::Index::create(&tenant_path, schema).unwrap();
        }

        // Save settings with an embedder so load_vector_index actually attempts
        // VectorIndex::load (without this it returns early at the "no embedders
        // configured" guard, making the test a false positive).
        let settings = crate::index::settings::IndexSettings {
            embedders: Some(std::collections::HashMap::from([(
                "default".to_string(),
                serde_json::json!({
                    "source": "userProvided",
                    "dimensions": 3
                }),
            )])),
            ..Default::default()
        };
        settings.save(&tenant_path.join("settings.json")).unwrap();

        // Write garbage to id_map.json (no fingerprint → backward compat, proceeds to load)
        let vectors_dir = tenant_path.join("vectors");
        std::fs::create_dir_all(&vectors_dir).unwrap();
        std::fs::write(vectors_dir.join("id_map.json"), "not valid json!!!").unwrap();

        let manager = IndexManager::new(tmp.path());
        // Should not error — gracefully skip corrupted vectors
        manager.get_or_load(tenant_id).unwrap();

        // VectorIndex should not be loaded
        assert!(
            manager.get_vector_index(tenant_id).is_none(),
            "corrupted vector index should not be loaded"
        );
    }

    #[cfg(feature = "vector-search")]
    #[tokio::test]
    async fn test_create_tenant_loads_existing_vectors() {
        use usearch::ffi::MetricKind;
        let tmp = TempDir::new().unwrap();
        let tenant_id = "create_load_t";
        let tenant_path = tmp.path().join(tenant_id);

        // Create tenant dir with Tantivy index
        std::fs::create_dir_all(&tenant_path).unwrap();
        {
            let schema = crate::index::schema::Schema::builder().build();
            let _ = crate::index::Index::create(&tenant_path, schema).unwrap();
        }

        // Save settings with an embedder so load_vector_index proceeds past the
        // "no embedders configured" guard (added in 8.19).
        let settings = crate::index::settings::IndexSettings {
            embedders: Some(std::collections::HashMap::from([(
                "default".to_string(),
                serde_json::json!({
                    "source": "userProvided",
                    "dimensions": 3
                }),
            )])),
            ..Default::default()
        };
        settings.save(&tenant_path.join("settings.json")).unwrap();

        // Save VectorIndex (no fingerprint file → backward compat load)
        let mut vi = crate::vector::index::VectorIndex::new(3, MetricKind::Cos).unwrap();
        vi.add("doc1", &[1.0, 0.0, 0.0]).unwrap();
        vi.add("doc2", &[0.0, 1.0, 0.0]).unwrap();
        vi.save(&tenant_path.join("vectors")).unwrap();

        let manager = IndexManager::new(tmp.path());
        manager.create_tenant(tenant_id).unwrap();

        let vi_arc = manager.get_vector_index(tenant_id);
        assert!(
            vi_arc.is_some(),
            "VectorIndex should be loaded on create_tenant"
        );
        let vi_arc = vi_arc.unwrap();
        let guard = vi_arc.read().unwrap();
        assert_eq!(guard.len(), 2);
    }

    // ── Vector recovery from oplog tests (8.10) ──

    /// Helper: create a tenant dir with a Tantivy index and an oplog, then write oplog entries
    /// with `_vectors` in the body. Returns the tenant path.
    #[cfg(feature = "vector-search")]
    fn setup_tenant_with_oplog_vectors(
        base_path: &Path,
        tenant_id: &str,
        ops: &[(String, serde_json::Value)],
    ) -> PathBuf {
        let tenant_path = base_path.join(tenant_id);
        std::fs::create_dir_all(&tenant_path).unwrap();

        // Create a Tantivy index
        let schema = crate::index::schema::Schema::builder().build();
        let _ = crate::index::Index::create(&tenant_path, schema).unwrap();

        // Write default settings
        let settings = crate::index::settings::IndexSettings::default();
        settings.save(&tenant_path.join("settings.json")).unwrap();

        // Create oplog and write entries
        let oplog_dir = tenant_path.join("oplog");
        let oplog = OpLog::open(&oplog_dir, tenant_id, "test_node").unwrap();
        oplog.append_batch(ops).unwrap();

        // Write committed_seq=0 to force full replay
        std::fs::write(tenant_path.join("committed_seq"), "0").unwrap();

        tenant_path
    }

    #[cfg(feature = "vector-search")]
    #[tokio::test]
    async fn test_recover_vectors_from_oplog() {
        let tmp = TempDir::new().unwrap();
        let tenant_id = "rec_vec_t";

        let ops = vec![
            (
                "upsert".to_string(),
                serde_json::json!({
                    "objectID": "doc1",
                    "body": {
                        "objectID": "doc1",
                        "title": "first",
                        "_vectors": {"default": [1.0, 0.0, 0.0]}
                    }
                }),
            ),
            (
                "upsert".to_string(),
                serde_json::json!({
                    "objectID": "doc2",
                    "body": {
                        "objectID": "doc2",
                        "title": "second",
                        "_vectors": {"default": [0.0, 1.0, 0.0]}
                    }
                }),
            ),
        ];

        setup_tenant_with_oplog_vectors(tmp.path(), tenant_id, &ops);

        let manager = IndexManager::new(tmp.path());
        manager.get_or_load(tenant_id).unwrap();

        // Verify VectorIndex was rebuilt from oplog
        let vi_arc = manager.get_vector_index(tenant_id);
        assert!(vi_arc.is_some(), "VectorIndex should be rebuilt from oplog");
        let vi_arc = vi_arc.unwrap();
        let guard = vi_arc.read().unwrap();
        assert_eq!(guard.len(), 2);

        let results = guard.search(&[1.0, 0.0, 0.0], 1).unwrap();
        assert_eq!(results[0].doc_id, "doc1");
    }

    #[cfg(feature = "vector-search")]
    #[tokio::test]
    async fn test_recover_vectors_with_deletes() {
        let tmp = TempDir::new().unwrap();
        let tenant_id = "rec_del_t";

        let ops = vec![
            (
                "upsert".to_string(),
                serde_json::json!({
                    "objectID": "doc1",
                    "body": {
                        "objectID": "doc1",
                        "title": "first",
                        "_vectors": {"default": [1.0, 0.0, 0.0]}
                    }
                }),
            ),
            (
                "upsert".to_string(),
                serde_json::json!({
                    "objectID": "doc2",
                    "body": {
                        "objectID": "doc2",
                        "title": "second",
                        "_vectors": {"default": [0.0, 1.0, 0.0]}
                    }
                }),
            ),
            (
                "delete".to_string(),
                serde_json::json!({"objectID": "doc1"}),
            ),
        ];

        setup_tenant_with_oplog_vectors(tmp.path(), tenant_id, &ops);

        let manager = IndexManager::new(tmp.path());
        manager.get_or_load(tenant_id).unwrap();

        let vi_arc = manager.get_vector_index(tenant_id);
        assert!(vi_arc.is_some(), "VectorIndex should exist after recovery");
        let vi_lock = vi_arc.unwrap();
        let guard = vi_lock.read().unwrap();
        assert_eq!(guard.len(), 1, "only doc2 should remain after delete");

        let results = guard.search(&[0.0, 1.0, 0.0], 1).unwrap();
        assert_eq!(results[0].doc_id, "doc2");
    }

    #[cfg(feature = "vector-search")]
    #[tokio::test]
    async fn test_recover_no_vectors_in_old_oplog() {
        let tmp = TempDir::new().unwrap();
        let tenant_id = "rec_novec_t";

        // Oplog entries without _vectors (pre-stage-8 format)
        let ops = vec![(
            "upsert".to_string(),
            serde_json::json!({
                "objectID": "doc1",
                "body": {"objectID": "doc1", "title": "old format doc"}
            }),
        )];

        setup_tenant_with_oplog_vectors(tmp.path(), tenant_id, &ops);

        let manager = IndexManager::new(tmp.path());
        manager.get_or_load(tenant_id).unwrap();

        // No VectorIndex should be created
        assert!(
            manager.get_vector_index(tenant_id).is_none(),
            "no VectorIndex when oplog has no _vectors"
        );
    }

    #[cfg(feature = "vector-search")]
    #[tokio::test]
    async fn test_recover_vectors_after_clear_op() {
        let tmp = TempDir::new().unwrap();
        let tenant_id = "rec_clear_t";

        let ops = vec![
            (
                "upsert".to_string(),
                serde_json::json!({
                    "objectID": "doc1",
                    "body": {
                        "objectID": "doc1",
                        "title": "first",
                        "_vectors": {"default": [1.0, 0.0, 0.0]}
                    }
                }),
            ),
            (
                "upsert".to_string(),
                serde_json::json!({
                    "objectID": "doc2",
                    "body": {
                        "objectID": "doc2",
                        "title": "second",
                        "_vectors": {"default": [0.0, 1.0, 0.0]}
                    }
                }),
            ),
            ("clear".to_string(), serde_json::json!({})),
            (
                "upsert".to_string(),
                serde_json::json!({
                    "objectID": "doc3",
                    "body": {
                        "objectID": "doc3",
                        "title": "third",
                        "_vectors": {"default": [0.0, 0.0, 1.0]}
                    }
                }),
            ),
        ];

        setup_tenant_with_oplog_vectors(tmp.path(), tenant_id, &ops);

        let manager = IndexManager::new(tmp.path());
        manager.get_or_load(tenant_id).unwrap();

        let vi_arc = manager.get_vector_index(tenant_id);
        assert!(vi_arc.is_some(), "VectorIndex should exist after recovery");
        let vi_lock = vi_arc.unwrap();
        let guard = vi_lock.read().unwrap();
        assert_eq!(guard.len(), 1, "only doc3 should exist after clear + add");

        let results = guard.search(&[0.0, 0.0, 1.0], 1).unwrap();
        assert_eq!(results[0].doc_id, "doc3");
    }

    #[cfg(feature = "vector-search")]
    #[tokio::test]
    async fn test_recover_vectors_saved_to_disk() {
        let tmp = TempDir::new().unwrap();
        let tenant_id = "rec_disk_t";

        let ops = vec![(
            "upsert".to_string(),
            serde_json::json!({
                "objectID": "doc1",
                "body": {
                    "objectID": "doc1",
                    "title": "first",
                    "_vectors": {"default": [1.0, 0.0, 0.0]}
                }
            }),
        )];

        let tenant_path = setup_tenant_with_oplog_vectors(tmp.path(), tenant_id, &ops);

        let manager = IndexManager::new(tmp.path());
        manager.get_or_load(tenant_id).unwrap();

        // Verify vector files were saved to disk after recovery
        let vectors_dir = tenant_path.join("vectors");
        assert!(
            vectors_dir.join("index.usearch").exists(),
            "index.usearch should be saved after recovery"
        );
        assert!(
            vectors_dir.join("id_map.json").exists(),
            "id_map.json should be saved after recovery"
        );
    }

    #[cfg(feature = "vector-search")]
    #[tokio::test]
    async fn test_recover_vectors_upsert_same_doc_twice() {
        let tmp = TempDir::new().unwrap();
        let tenant_id = "rec_dup_t";

        // Upsert doc1 with vector A, then upsert doc1 again with vector B
        let ops = vec![
            (
                "upsert".to_string(),
                serde_json::json!({
                    "objectID": "doc1",
                    "body": {
                        "objectID": "doc1",
                        "title": "first version",
                        "_vectors": {"default": [1.0, 0.0, 0.0]}
                    }
                }),
            ),
            (
                "upsert".to_string(),
                serde_json::json!({
                    "objectID": "doc1",
                    "body": {
                        "objectID": "doc1",
                        "title": "second version",
                        "_vectors": {"default": [0.0, 1.0, 0.0]}
                    }
                }),
            ),
        ];

        setup_tenant_with_oplog_vectors(tmp.path(), tenant_id, &ops);

        let manager = IndexManager::new(tmp.path());
        manager.get_or_load(tenant_id).unwrap();

        let vi_arc = manager.get_vector_index(tenant_id);
        assert!(vi_arc.is_some(), "VectorIndex should exist after recovery");
        let vi_lock = vi_arc.unwrap();
        let guard = vi_lock.read().unwrap();
        assert_eq!(guard.len(), 1, "re-upsert should not duplicate doc1");

        // The vector should be the SECOND one (latest wins)
        let results = guard.search(&[0.0, 1.0, 0.0], 1).unwrap();
        assert_eq!(results[0].doc_id, "doc1");
    }

    #[cfg(feature = "vector-search")]
    #[tokio::test]
    async fn test_load_vector_index_skips_when_already_loaded() {
        use usearch::ffi::MetricKind;
        let tmp = TempDir::new().unwrap();
        let tenant_id = "skip_load_t";
        let tenant_path = tmp.path().join(tenant_id);

        // Create tenant on disk
        std::fs::create_dir_all(&tenant_path).unwrap();
        {
            let schema = crate::index::schema::Schema::builder().build();
            let _ = crate::index::Index::create(&tenant_path, schema).unwrap();
        }

        // Save a VectorIndex with 2 docs to disk
        let mut vi_disk = crate::vector::index::VectorIndex::new(3, MetricKind::Cos).unwrap();
        vi_disk.add("disk_doc1", &[1.0, 0.0, 0.0]).unwrap();
        vi_disk.add("disk_doc2", &[0.0, 1.0, 0.0]).unwrap();
        vi_disk.save(&tenant_path.join("vectors")).unwrap();

        let manager = IndexManager::new(tmp.path());

        // Pre-populate vector_indices with a DIFFERENT VectorIndex (1 doc)
        let mut vi_mem = crate::vector::index::VectorIndex::new(3, MetricKind::Cos).unwrap();
        vi_mem.add("mem_doc1", &[0.0, 0.0, 1.0]).unwrap();
        manager.set_vector_index(tenant_id, vi_mem);

        // Now call get_or_load — load_vector_index should skip because already populated
        manager.get_or_load(tenant_id).unwrap();

        // Verify we still have the in-memory version (1 doc), NOT the disk version (2 docs)
        let vi_arc = manager.get_vector_index(tenant_id).unwrap();
        let guard = vi_arc.read().unwrap();
        assert_eq!(
            guard.len(),
            1,
            "should keep in-memory index, not overwrite from disk"
        );
        let results = guard.search(&[0.0, 0.0, 1.0], 1).unwrap();
        assert_eq!(results[0].doc_id, "mem_doc1");
    }

    #[cfg(feature = "vector-search")]
    #[tokio::test]
    async fn test_full_crash_recovery_vectors_available() {
        use wiremock::matchers::method;
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "embedding": [0.7, 0.8, 0.9]
            })))
            .mount(&server)
            .await;

        let tmp = TempDir::new().unwrap();
        let tenant_id = "crash_rec_t";

        // Phase 1: Create manager, add docs with embedder, let commit happen
        {
            let manager = IndexManager::new(tmp.path());
            manager.create_tenant(tenant_id).unwrap();

            // Configure embedder in settings
            let tenant_path = tmp.path().join(tenant_id);
            let settings = crate::index::settings::IndexSettings {
                embedders: Some(HashMap::from([(
                    "default".to_string(),
                    serde_json::json!({
                        "source": "rest",
                        "url": format!("{}/embed", server.uri()),
                        "request": {"input": "{{text}}"},
                        "response": {"embedding": "{{embedding}}"},
                        "dimensions": 3
                    }),
                )])),
                ..Default::default()
            };
            settings.save(&tenant_path.join("settings.json")).unwrap();

            // Add docs through write queue (which creates oplog entries)
            let docs = vec![Document {
                id: "doc1".to_string(),
                fields: HashMap::from([(
                    "title".to_string(),
                    crate::types::FieldValue::Text("recovery test".to_string()),
                )]),
            }];
            manager.add_documents_sync(tenant_id, docs).await.unwrap();

            // Verify vectors exist in memory
            let vi_arc = manager.get_vector_index(tenant_id);
            assert!(vi_arc.is_some(), "vectors should be in memory after add");
        }

        // Phase 2: Simulate crash — create new IndexManager
        {
            let manager2 = IndexManager::new(tmp.path());
            manager2.get_or_load(tenant_id).unwrap();

            // Vectors should be loaded from disk (saved after commit)
            let vi_arc = manager2.get_vector_index(tenant_id);
            assert!(
                vi_arc.is_some(),
                "vectors should survive manager restart (loaded from disk)"
            );
            let vi_lock = vi_arc.unwrap();
            let guard = vi_lock.read().unwrap();
            assert_eq!(guard.len(), 1);
            assert_eq!(guard.dimensions(), 3);
        }
    }

    // ── Fingerprint integration tests (8.18) ──

    #[cfg(feature = "vector-search")]
    #[tokio::test]
    async fn test_fingerprint_match_loads_vectors() {
        use usearch::ffi::MetricKind;
        let tmp = TempDir::new().unwrap();
        let tenant_id = "fp_match_t";
        let tenant_path = tmp.path().join(tenant_id);

        std::fs::create_dir_all(&tenant_path).unwrap();
        {
            let schema = crate::index::schema::Schema::builder().build();
            let _ = crate::index::Index::create(&tenant_path, schema).unwrap();
        }

        // Save settings with a rest embedder
        let settings = crate::index::settings::IndexSettings {
            embedders: Some(std::collections::HashMap::from([(
                "default".to_string(),
                serde_json::json!({
                    "source": "rest",
                    "model": "text-embedding-3-small",
                    "dimensions": 3
                }),
            )])),
            ..Default::default()
        };
        settings.save(&tenant_path.join("settings.json")).unwrap();

        // Save VectorIndex
        let mut vi = crate::vector::index::VectorIndex::new(3, MetricKind::Cos).unwrap();
        vi.add("doc1", &[1.0, 0.0, 0.0]).unwrap();
        vi.save(&tenant_path.join("vectors")).unwrap();

        // Save matching fingerprint
        let configs = vec![(
            "default".to_string(),
            crate::vector::config::EmbedderConfig {
                source: crate::vector::config::EmbedderSource::Rest,
                model: Some("text-embedding-3-small".into()),
                dimensions: Some(3),
                ..Default::default()
            },
        )];
        let fp = crate::vector::config::EmbedderFingerprint::from_configs(&configs, 3);
        fp.save(&tenant_path.join("vectors")).unwrap();

        let manager = IndexManager::new(tmp.path());
        manager.get_or_load(tenant_id).unwrap();

        assert!(
            manager.get_vector_index(tenant_id).is_some(),
            "vectors should load when fingerprint matches"
        );
    }

    #[cfg(feature = "vector-search")]
    #[tokio::test]
    async fn test_fingerprint_mismatch_skips_vectors() {
        use usearch::ffi::MetricKind;
        let tmp = TempDir::new().unwrap();
        let tenant_id = "fp_mismatch_t";
        let tenant_path = tmp.path().join(tenant_id);

        std::fs::create_dir_all(&tenant_path).unwrap();
        {
            let schema = crate::index::schema::Schema::builder().build();
            let _ = crate::index::Index::create(&tenant_path, schema).unwrap();
        }

        // Settings with model B
        let settings = crate::index::settings::IndexSettings {
            embedders: Some(std::collections::HashMap::from([(
                "default".to_string(),
                serde_json::json!({
                    "source": "openAi",
                    "model": "text-embedding-3-large",
                    "dimensions": 3,
                    "apiKey": "sk-test"
                }),
            )])),
            ..Default::default()
        };
        settings.save(&tenant_path.join("settings.json")).unwrap();

        // Save VectorIndex
        let mut vi = crate::vector::index::VectorIndex::new(3, MetricKind::Cos).unwrap();
        vi.add("doc1", &[1.0, 0.0, 0.0]).unwrap();
        vi.save(&tenant_path.join("vectors")).unwrap();

        // Save fingerprint with model A (MISMATCH)
        let configs = vec![(
            "default".to_string(),
            crate::vector::config::EmbedderConfig {
                source: crate::vector::config::EmbedderSource::OpenAi,
                model: Some("text-embedding-3-small".into()),
                dimensions: Some(3),
                ..Default::default()
            },
        )];
        let fp = crate::vector::config::EmbedderFingerprint::from_configs(&configs, 3);
        fp.save(&tenant_path.join("vectors")).unwrap();

        let manager = IndexManager::new(tmp.path());
        manager.get_or_load(tenant_id).unwrap();

        assert!(
            manager.get_vector_index(tenant_id).is_none(),
            "vectors should NOT load when fingerprint mismatches (model changed)"
        );
    }

    #[cfg(feature = "vector-search")]
    #[tokio::test]
    async fn test_no_fingerprint_file_loads_vectors_anyway() {
        use usearch::ffi::MetricKind;
        let tmp = TempDir::new().unwrap();
        let tenant_id = "nofp_t";
        let tenant_path = tmp.path().join(tenant_id);

        std::fs::create_dir_all(&tenant_path).unwrap();
        {
            let schema = crate::index::schema::Schema::builder().build();
            let _ = crate::index::Index::create(&tenant_path, schema).unwrap();
        }

        // Save settings with embedder
        let settings = crate::index::settings::IndexSettings {
            embedders: Some(std::collections::HashMap::from([(
                "default".to_string(),
                serde_json::json!({
                    "source": "rest",
                    "model": "text-embedding-3-small",
                    "dimensions": 3
                }),
            )])),
            ..Default::default()
        };
        settings.save(&tenant_path.join("settings.json")).unwrap();

        // Save VectorIndex but NO fingerprint.json (backward compat)
        let mut vi = crate::vector::index::VectorIndex::new(3, MetricKind::Cos).unwrap();
        vi.add("doc1", &[1.0, 0.0, 0.0]).unwrap();
        vi.save(&tenant_path.join("vectors")).unwrap();

        let manager = IndexManager::new(tmp.path());
        manager.get_or_load(tenant_id).unwrap();

        assert!(
            manager.get_vector_index(tenant_id).is_some(),
            "vectors should load when no fingerprint file exists (backward compat)"
        );
    }

    #[cfg(feature = "vector-search")]
    #[tokio::test]
    async fn test_fingerprint_mismatch_template_change_skips() {
        use usearch::ffi::MetricKind;
        let tmp = TempDir::new().unwrap();
        let tenant_id = "fp_tmpl_t";
        let tenant_path = tmp.path().join(tenant_id);

        std::fs::create_dir_all(&tenant_path).unwrap();
        {
            let schema = crate::index::schema::Schema::builder().build();
            let _ = crate::index::Index::create(&tenant_path, schema).unwrap();
        }

        // Settings with NEW template
        let settings = crate::index::settings::IndexSettings {
            embedders: Some(std::collections::HashMap::from([(
                "default".to_string(),
                serde_json::json!({
                    "source": "rest",
                    "model": "model-a",
                    "dimensions": 3,
                    "documentTemplate": "{{doc.title}}"
                }),
            )])),
            ..Default::default()
        };
        settings.save(&tenant_path.join("settings.json")).unwrap();

        // Save VectorIndex
        let mut vi = crate::vector::index::VectorIndex::new(3, MetricKind::Cos).unwrap();
        vi.add("doc1", &[1.0, 0.0, 0.0]).unwrap();
        vi.save(&tenant_path.join("vectors")).unwrap();

        // Save fingerprint with OLD template (MISMATCH)
        let configs = vec![(
            "default".to_string(),
            crate::vector::config::EmbedderConfig {
                source: crate::vector::config::EmbedderSource::Rest,
                model: Some("model-a".into()),
                dimensions: Some(3),
                document_template: Some("{{doc.title}} {{doc.body}}".into()),
                ..Default::default()
            },
        )];
        let fp = crate::vector::config::EmbedderFingerprint::from_configs(&configs, 3);
        fp.save(&tenant_path.join("vectors")).unwrap();

        let manager = IndexManager::new(tmp.path());
        manager.get_or_load(tenant_id).unwrap();

        assert!(
            manager.get_vector_index(tenant_id).is_none(),
            "vectors should NOT load when document_template changed"
        );
    }

    // ── Memory accounting tests (8.21) ──

    #[cfg(feature = "vector-search")]
    #[tokio::test]
    async fn test_vector_memory_usage_with_indices() {
        use usearch::ffi::MetricKind;
        let tmp = TempDir::new().unwrap();
        let manager = IndexManager::new(tmp.path());
        manager.create_tenant("mem_t").unwrap();

        // Create a VectorIndex with some vectors
        let mut vi = crate::vector::index::VectorIndex::new(3, MetricKind::Cos).unwrap();
        vi.add("doc1", &[1.0, 0.0, 0.0]).unwrap();
        vi.add("doc2", &[0.0, 1.0, 0.0]).unwrap();
        vi.add("doc3", &[0.0, 0.0, 1.0]).unwrap();
        manager.set_vector_index("mem_t", vi);

        let usage = manager.vector_memory_usage();
        assert!(
            usage > 0,
            "vector_memory_usage should be > 0 when vectors exist, got {}",
            usage
        );
    }

    #[cfg(feature = "vector-search")]
    #[tokio::test]
    async fn test_vector_memory_usage_no_indices() {
        let tmp = TempDir::new().unwrap();
        let manager = IndexManager::new(tmp.path());

        let usage = manager.vector_memory_usage();
        assert_eq!(usage, 0, "vector_memory_usage should be 0 with no indices");
    }

    // ── HTTP integration tests (8.25) ──

    #[cfg(feature = "vector-search")]
    #[tokio::test]
    async fn test_vectors_survive_manager_restart() {
        use wiremock::matchers::method;
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "embedding": [0.5, 0.6, 0.7]
            })))
            .mount(&server)
            .await;

        let tmp = TempDir::new().unwrap();
        let tenant_id = "restart_surv_t";

        // Phase 1: Create manager, add docs with embedder, verify vectors exist
        {
            let manager = IndexManager::new(tmp.path());
            manager.create_tenant(tenant_id).unwrap();

            let tenant_path = tmp.path().join(tenant_id);
            let settings = crate::index::settings::IndexSettings {
                embedders: Some(HashMap::from([(
                    "default".to_string(),
                    serde_json::json!({
                        "source": "rest",
                        "url": format!("{}/embed", server.uri()),
                        "request": {"input": "{{text}}"},
                        "response": {"embedding": "{{embedding}}"},
                        "dimensions": 3
                    }),
                )])),
                ..Default::default()
            };
            settings.save(&tenant_path.join("settings.json")).unwrap();

            let docs = vec![
                Document {
                    id: "doc1".to_string(),
                    fields: HashMap::from([(
                        "title".to_string(),
                        crate::types::FieldValue::Text("alpha bravo".to_string()),
                    )]),
                },
                Document {
                    id: "doc2".to_string(),
                    fields: HashMap::from([(
                        "title".to_string(),
                        crate::types::FieldValue::Text("charlie delta".to_string()),
                    )]),
                },
            ];
            manager.add_documents_sync(tenant_id, docs).await.unwrap();

            // Verify vectors exist in memory
            let vi_arc = manager
                .get_vector_index(tenant_id)
                .expect("vectors should exist");
            let guard = vi_arc.read().unwrap();
            assert_eq!(guard.len(), 2, "should have 2 vectors");
            // Verify search works
            let results = guard.search(&[0.5, 0.6, 0.7], 2).unwrap();
            assert_eq!(results.len(), 2, "search should return 2 results");
        }

        // Phase 2: Restart — create new IndexManager with same base_path
        {
            let manager2 = IndexManager::new(tmp.path());
            manager2.get_or_load(tenant_id).unwrap();

            // Vectors should be loaded from disk
            let vi_arc = manager2.get_vector_index(tenant_id);
            assert!(vi_arc.is_some(), "vectors should survive manager restart");

            let vi_lock = vi_arc.unwrap();
            let guard = vi_lock.read().unwrap();
            assert_eq!(guard.len(), 2, "should still have 2 vectors after restart");
            assert_eq!(guard.dimensions(), 3);

            // Verify search still works after restart
            let results = guard.search(&[0.5, 0.6, 0.7], 2).unwrap();
            assert_eq!(
                results.len(),
                2,
                "search should return 2 results after restart"
            );
        }
    }

    #[cfg(feature = "vector-search")]
    #[tokio::test]
    async fn test_vectors_lost_when_embedder_model_changes() {
        use wiremock::matchers::method;
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "embedding": [0.1, 0.2, 0.3]
            })))
            .mount(&server)
            .await;

        let tmp = TempDir::new().unwrap();
        let tenant_id = "model_chg_t";
        let tenant_path = tmp.path().join(tenant_id);

        // Phase 1: Add docs with model A (REST embedder)
        {
            let manager = IndexManager::new(tmp.path());
            manager.create_tenant(tenant_id).unwrap();

            let settings = crate::index::settings::IndexSettings {
                embedders: Some(HashMap::from([(
                    "default".to_string(),
                    serde_json::json!({
                        "source": "rest",
                        "model": "model-a",
                        "url": format!("{}/embed", server.uri()),
                        "request": {"input": "{{text}}"},
                        "response": {"embedding": "{{embedding}}"},
                        "dimensions": 3
                    }),
                )])),
                ..Default::default()
            };
            settings.save(&tenant_path.join("settings.json")).unwrap();

            let docs = vec![Document {
                id: "doc1".to_string(),
                fields: HashMap::from([(
                    "title".to_string(),
                    crate::types::FieldValue::Text("test doc".to_string()),
                )]),
            }];
            manager.add_documents_sync(tenant_id, docs).await.unwrap();

            assert!(
                manager.get_vector_index(tenant_id).is_some(),
                "vectors should exist after Phase 1"
            );
        }

        // Phase 2: Change settings to model B, restart
        {
            let settings = crate::index::settings::IndexSettings {
                embedders: Some(HashMap::from([(
                    "default".to_string(),
                    serde_json::json!({
                        "source": "rest",
                        "model": "model-b",
                        "url": format!("{}/embed", server.uri()),
                        "request": {"input": "{{text}}"},
                        "response": {"embedding": "{{embedding}}"},
                        "dimensions": 3
                    }),
                )])),
                ..Default::default()
            };
            settings.save(&tenant_path.join("settings.json")).unwrap();

            let manager2 = IndexManager::new(tmp.path());
            manager2.get_or_load(tenant_id).unwrap();

            // Vectors should NOT be loaded — fingerprint mismatch
            assert!(
                manager2.get_vector_index(tenant_id).is_none(),
                "vectors should NOT load when embedder model changes (fingerprint mismatch)"
            );
        }
    }
}

// ── Vector index storage (behind vector-search feature) ──

#[cfg(feature = "vector-search")]
impl IndexManager {
    /// Get the vector index for a tenant, if one has been stored.
    pub fn get_vector_index(
        &self,
        tenant_id: &str,
    ) -> Option<Arc<std::sync::RwLock<crate::vector::index::VectorIndex>>> {
        self.vector_indices.get(tenant_id).map(|r| Arc::clone(&r))
    }

    /// Return total memory used by all loaded vector indices, in bytes.
    pub fn vector_memory_usage(&self) -> usize {
        let mut total = 0usize;
        for entry in self.vector_indices.iter() {
            if let Ok(guard) = entry.value().read() {
                total += guard.memory_usage();
            }
        }
        total
    }

    /// Store a vector index for a tenant, wrapping it in Arc<RwLock<_>>.
    pub fn set_vector_index(&self, tenant_id: &str, index: crate::vector::index::VectorIndex) {
        self.vector_indices.insert(
            tenant_id.to_string(),
            Arc::new(std::sync::RwLock::new(index)),
        );
    }

    /// Load a vector index from disk for a tenant if one exists.
    ///
    /// Checks for the sentinel file `{tenant_path}/vectors/id_map.json`.
    /// Skips if already loaded (e.g., by oplog recovery).
    /// Checks embedder fingerprint against current settings — skips stale vectors.
    /// Logs warning and skips on failure — tenant is BM25-only.
    fn load_vector_index(&self, tenant_id: &str, tenant_path: &Path) {
        if self.vector_indices.contains_key(tenant_id) {
            return;
        }
        let vectors_dir = tenant_path.join("vectors");
        let sentinel = vectors_dir.join("id_map.json");
        if !sentinel.exists() {
            return;
        }

        // Parse current embedder configs from settings.json.
        let settings_path = tenant_path.join("settings.json");
        let current_configs: Vec<(String, crate::vector::config::EmbedderConfig)> = settings_path
            .exists()
            .then(|| IndexSettings::load(&settings_path).ok())
            .flatten()
            .and_then(|s| {
                s.embedders.as_ref().map(|emb_map| {
                    emb_map
                        .iter()
                        .filter_map(|(name, json)| {
                            if json.is_null() {
                                return None;
                            }
                            serde_json::from_value::<crate::vector::config::EmbedderConfig>(
                                json.clone(),
                            )
                            .ok()
                            .map(|cfg| (name.clone(), cfg))
                        })
                        .collect()
                })
            })
            .unwrap_or_default();

        // If no embedders configured, vectors are orphaned — skip loading.
        if current_configs.is_empty() {
            tracing::info!(
                "[LOAD {}] no embedders configured, skipping vector index load",
                tenant_id
            );
            return;
        }

        // If fingerprint exists, verify it matches current configs.
        // If fingerprint is missing: load vectors anyway (backward compat).
        if let Ok(fp) = crate::vector::config::EmbedderFingerprint::load(&vectors_dir) {
            if !fp.matches_configs(&current_configs) {
                tracing::warn!(
                    "[LOAD {}] embedder fingerprint mismatch — vectors are stale, skipping load (BM25 fallback)",
                    tenant_id
                );
                return;
            }
        }

        match crate::vector::index::VectorIndex::load(&vectors_dir, usearch::ffi::MetricKind::Cos) {
            Ok(vi) => {
                let count = vi.len();
                self.set_vector_index(tenant_id, vi);
                tracing::info!(
                    "[LOAD {}] loaded vector index from disk ({} vectors)",
                    tenant_id,
                    count
                );
            }
            Err(e) => {
                tracing::warn!("[LOAD {}] failed to load vector index: {}", tenant_id, e);
            }
        }
    }
}

impl Drop for IndexManager {
    /// Abort all background write tasks when the manager is dropped.
    ///
    /// Without this, dropping a JoinHandle in tokio detaches the task (does not
    /// cancel it). Detached tasks continue running in the tokio runtime even after
    /// the IndexManager is gone, holding file handles briefly. Under parallel
    /// test loads this causes races with other tests that access the same runtime.
    ///
    /// In production the server always calls `graceful_shutdown()` before dropping,
    /// which drains writes cleanly. This abort-on-drop is a safety net for tests
    /// and unexpected drops.
    fn drop(&mut self) {
        for entry in self.write_task_handles.iter() {
            entry.value().abort();
        }
    }
}
