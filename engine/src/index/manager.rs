use crate::error::{FlapjackError, Result};
use crate::index::oplog::OpLog;
use crate::index::relevance::RelevanceConfig;
use crate::index::rules::RuleStore;
use crate::index::settings::IndexSettings;
use crate::index::synonyms::SynonymStore;
use crate::index::task_queue::TaskQueue;
use crate::index::utils::copy_dir_recursive;
use crate::index::write_queue::{create_write_queue, WriteAction, WriteOp, WriteQueue};
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
            }
        })
    }

    /// Get the oplog for a tenant (for external access)
    pub fn get_oplog(&self, tenant_id: &str) -> Option<Arc<OpLog>> {
        self.oplogs.get(tenant_id).map(|r| Arc::clone(&r))
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
        let mut first_query_total: Option<usize> = None;

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
            match first_query_total {
                Some(prev) if result.total > prev => first_query_total = Some(result.total),
                None => first_query_total = Some(result.total),
                _ => {}
            }
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

        let mut total = first_query_total.unwrap_or(0);
        let result_count = all_results.len();
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

    /// Add documents to a tenant's index.
    ///
    /// Creates a writer, adds documents, and commits immediately.
    /// For production, this should be batched via background commit thread.
    pub fn add_documents_insert(&self, tenant_id: &str, docs: Vec<Document>) -> Result<TaskInfo> {
        self.add_documents_inner(tenant_id, docs, false)
    }

    pub fn add_documents(&self, tenant_id: &str, docs: Vec<Document>) -> Result<TaskInfo> {
        self.add_documents_inner(tenant_id, docs, true)
    }

    fn add_documents_inner(
        &self,
        tenant_id: &str,
        docs: Vec<Document>,
        upsert: bool,
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

        let tx = self
            .write_queues
            .entry(tenant_id.to_string())
            .or_insert_with(|| {
                let oplog = self.get_or_create_oplog(tenant_id);
                let (queue, handle) = create_write_queue(
                    tenant_id.to_string(),
                    Arc::clone(&index),
                    Arc::clone(&self.writers),
                    Arc::clone(&self.tasks),
                    self.base_path.clone(),
                    oplog,
                    Arc::clone(&self.facet_cache),
                );
                self.write_task_handles
                    .insert(tenant_id.to_string(), handle);
                queue
            })
            .clone();

        let actions = if upsert {
            docs.into_iter().map(WriteAction::Upsert).collect()
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

        let tx = self
            .write_queues
            .entry(tenant_id.to_string())
            .or_insert_with(|| {
                let oplog = self.get_or_create_oplog(tenant_id);
                let (queue, handle) = create_write_queue(
                    tenant_id.to_string(),
                    Arc::clone(&index),
                    Arc::clone(&self.writers),
                    Arc::clone(&self.tasks),
                    self.base_path.clone(),
                    oplog,
                    Arc::clone(&self.facet_cache),
                );
                self.write_task_handles
                    .insert(tenant_id.to_string(), handle);
                queue
            })
            .clone();

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

        let tx = self
            .write_queues
            .entry(tenant_id.to_string())
            .or_insert_with(|| {
                let oplog = self.get_or_create_oplog(tenant_id);
                let (queue, handle) = create_write_queue(
                    tenant_id.to_string(),
                    Arc::clone(&index),
                    Arc::clone(&self.writers),
                    Arc::clone(&self.tasks),
                    self.base_path.clone(),
                    oplog,
                    Arc::clone(&self.facet_cache),
                );
                self.write_task_handles
                    .insert(tenant_id.to_string(), handle);
                queue
            })
            .clone();

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
        self.write_queues.remove(tenant_id);

        if let Some((_, handle)) = self.write_task_handles.remove(tenant_id) {
            let _ = handle.await;
        }

        self.writers.remove(tenant_id);
        self.oplogs.remove(tenant_id);
        self.loaded.remove(tenant_id);

        let path = self.base_path.join(tenant_id);
        if path.exists() {
            std::fs::remove_dir_all(path)?;
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
}
