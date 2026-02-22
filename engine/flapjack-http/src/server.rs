use axum::{
    extract::DefaultBodyLimit,
    middleware,
    routing::{delete, get, post},
    Router,
};
use fs2::FileExt;
use std::fs::OpenOptions;
use std::path::Path;
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::auth::{authenticate_and_authorize, generate_admin_key, generate_hex_key, KeyStore};
use crate::handlers::dashboard::dashboard_handler;
use crate::handlers::snapshot;
use crate::handlers::{
    add_documents, add_record_auto_id, batch_search, browse_index, clear_index, clear_rules,
    clear_synonyms, compact_index, create_index, delete_by_query, delete_index, delete_object,
    delete_rule, delete_synonym, get_object, get_objects, get_rule, get_synonym, get_task,
    get_task_for_index, health, list_algolia_indexes, list_indices, migrate_from_algolia,
    operation_index, partial_update_object, put_object, save_rule, save_rules, save_synonym,
    save_synonyms, search, search_facet_values, search_rules, search_synonyms, AppState,
};
use crate::middleware::{allow_private_network, normalize_content_type};
use crate::openapi::ApiDoc;
use flapjack::experiments::store::ExperimentStore;
use flapjack::IndexManager;

pub async fn serve() -> Result<(), Box<dyn std::error::Error>> {
    let startup_start = std::time::Instant::now();

    let env_mode = std::env::var("FLAPJACK_ENV").unwrap_or_else(|_| "development".into());
    let no_auth = std::env::var("FLAPJACK_NO_AUTH")
        .ok()
        .filter(|v| v == "1")
        .is_some();

    if no_auth && env_mode == "production" {
        eprintln!("ERROR: --no-auth cannot be used in production mode.");
        std::process::exit(1);
    }

    let admin_key_env = std::env::var("FLAPJACK_ADMIN_KEY")
        .ok()
        .filter(|k| !k.is_empty());

    match (env_mode.as_str(), &admin_key_env) {
        ("production", None) => {
            let suggested = generate_hex_key();
            eprintln!("ERROR: FLAPJACK_ADMIN_KEY is required in production mode.");
            eprintln!("Suggested key: {}", suggested);
            std::process::exit(1);
        }
        ("production", Some(k)) if k.len() < 16 => {
            eprintln!("ERROR: FLAPJACK_ADMIN_KEY must be at least 16 characters in production.");
            std::process::exit(1);
        }
        _ => {}
    }

    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Log memory configuration on startup
    {
        let observer = flapjack::MemoryObserver::global();
        let stats = observer.stats();
        let budget = flapjack::get_global_budget();
        tracing::info!(
            allocator = stats.allocator,
            memory_limit_mb = stats.system_limit_bytes / (1024 * 1024),
            limit_source = %stats.limit_source,
            high_watermark_pct = stats.high_watermark_pct,
            critical_pct = stats.critical_pct,
            max_concurrent_writers = budget.max_concurrent_writers(),
            "Memory configuration loaded"
        );
    }

    let data_dir = std::env::var("FLAPJACK_DATA_DIR").unwrap_or_else(|_| "./data".to_string());
    let _data_dir_lock = match acquire_data_dir_process_lock(Path::new(&data_dir)) {
        Ok(lock) => lock,
        Err(message) => {
            eprintln!("ERROR: {}", message);
            std::process::exit(1);
        }
    };
    let requested_bind_addr =
        std::env::var("FLAPJACK_BIND_ADDR").unwrap_or_else(|_| "127.0.0.1:7700".to_string());

    // Determine effective admin key:
    //   1. --no-auth → no key
    //   2. FLAPJACK_ADMIN_KEY env var → use it (production, key rotation)
    //   3. Otherwise → use/create .admin_key file (local dev convenience)
    let admin_key_file = Path::new(&data_dir).join(".admin_key");

    let (admin_key, key_is_new) = if no_auth {
        (None, false)
    } else if let Some(key) = admin_key_env {
        // Env var takes precedence - save to file for subsequent boots
        let trimmed_key = key.trim();
        if let Err(e) = std::fs::write(&admin_key_file, trimmed_key) {
            tracing::warn!("Failed to save admin key to .admin_key: {}", e);
        }
        (Some(trimmed_key.to_string()), false)
    } else if admin_key_file.exists() {
        // Read from file (local dev mode)
        match std::fs::read_to_string(&admin_key_file) {
            Ok(key) => (Some(key.trim().to_string()), false),
            Err(e) => {
                eprintln!("❌ Error: Failed to read .admin_key file: {}", e);
                eprintln!("   Run: flapjack reset-admin-key");
                std::process::exit(1);
            }
        }
    } else {
        // First boot: auto-generate admin key and save to file
        let key = generate_admin_key();
        // Ensure data directory exists
        if let Some(parent) = admin_key_file.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                eprintln!("❌ Error: Failed to create data directory: {}", e);
                std::process::exit(1);
            }
        }
        if let Err(e) = std::fs::write(&admin_key_file, &key) {
            eprintln!("❌ Error: Failed to save admin key: {}", e);
            std::process::exit(1);
        }
        // Set restrictive permissions (Unix only)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Err(e) =
                std::fs::set_permissions(&admin_key_file, std::fs::Permissions::from_mode(0o600))
            {
                tracing::warn!("Failed to set .admin_key permissions: {}", e);
            }
        }
        (Some(key), true)
    };

    let key_store = match admin_key {
        Some(ref key) => {
            let ks = Arc::new(KeyStore::load_or_create(Path::new(&data_dir), key));
            tracing::info!("API key authentication enabled");
            Some(ks)
        }
        None => None,
    };

    let manager = IndexManager::new(&data_dir);

    // Load replication config and initialize ReplicationManager
    let node_config =
        flapjack_replication::config::NodeConfig::load_or_default(std::path::Path::new(&data_dir));

    let node_json_path = Path::new(&data_dir).join("node.json");
    if node_json_path.exists() {
        if node_config.bind_addr != requested_bind_addr {
            tracing::warn!(
                requested = %requested_bind_addr,
                node_json_bind_addr = %node_config.bind_addr,
                node_json = %node_json_path.display(),
                "bind address loaded from node.json overrides FLAPJACK_BIND_ADDR"
            );
        } else {
            tracing::info!(
                node_json_bind_addr = %node_config.bind_addr,
                node_json = %node_json_path.display(),
                "bind address loaded from node.json"
            );
        }
    }

    // Use bind_addr from node.json, falling back to env var
    let bind_addr = node_config.bind_addr.clone();

    // Initialize analytics cluster client (for HA analytics fan-out)
    if let Some(cluster_client) =
        crate::analytics_cluster::AnalyticsClusterClient::new(&node_config)
    {
        crate::analytics_cluster::set_global_cluster(cluster_client);
        tracing::info!(
            "[HA-analytics] Cluster analytics enabled: fan-out to {} peers",
            node_config.peers.len()
        );
    }

    let replication_manager = if !node_config.peers.is_empty() {
        tracing::info!("Replication enabled: {} peers", node_config.peers.len());
        let repl = flapjack_replication::manager::ReplicationManager::new(node_config);
        flapjack_replication::set_global_manager(Arc::clone(&repl));
        repl.start_health_probe(10);
        tracing::info!("[HEALTH] Background health probe started (10s interval)");
        Some(repl)
    } else {
        tracing::info!("Replication disabled (no peers in node.json)");
        None
    };

    // Initialize SSL manager
    let ssl_manager = match flapjack::SslConfig::from_env() {
        Ok(ssl_config) => {
            tracing::info!(
                "[SSL] SSL management enabled for IP: {}",
                ssl_config.public_ip
            );
            match flapjack::SslManager::new(ssl_config).await {
                Ok(mgr) => {
                    // Spawn background renewal loop
                    let mgr_clone = Arc::clone(&mgr);
                    tokio::spawn(async move {
                        mgr_clone.start_renewal_loop().await;
                    });
                    tracing::info!("[SSL] Auto-renewal enabled (checks every 24h)");
                    flapjack_ssl::set_global_manager(Arc::clone(&mgr));
                    Some(mgr)
                }
                Err(e) => {
                    tracing::error!("[SSL] Failed to initialize SSL manager: {}", e);
                    None
                }
            }
        }
        Err(e) => {
            tracing::info!("[SSL] SSL management disabled: {}", e);
            None
        }
    };

    if let Some(s3_config) = flapjack::index::s3::S3Config::from_env() {
        auto_restore_from_s3(&data_dir, &s3_config, &manager).await;
        let interval_secs: u64 = std::env::var("FLAPJACK_SNAPSHOT_INTERVAL")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);
        if interval_secs > 0 {
            let mgr = Arc::clone(&manager);
            let s3 = s3_config.clone();
            let dd = data_dir.clone();
            tokio::spawn(async move {
                scheduled_s3_backups(dd, s3, mgr, interval_secs).await;
            });
            tracing::info!("Scheduled S3 backups every {}s", interval_secs);
        }
    }

    // Initialize analytics subsystem
    let analytics_config = flapjack::analytics::AnalyticsConfig::from_env();
    let analytics_collector =
        flapjack::analytics::AnalyticsCollector::new(analytics_config.clone());
    let analytics_engine = Arc::new(flapjack::analytics::AnalyticsQueryEngine::new(
        analytics_config.clone(),
    ));

    if analytics_config.enabled {
        flapjack::analytics::init_global_collector(Arc::clone(&analytics_collector));

        // Spawn background flush loop
        let collector_for_flush = Arc::clone(&analytics_collector);
        tokio::spawn(async move {
            collector_for_flush.run_flush_loop().await;
        });

        // Spawn retention cleanup loop
        let retention_dir = analytics_config.data_dir.clone();
        let retention_days = analytics_config.retention_days;
        tokio::spawn(async move {
            flapjack::analytics::retention::run_retention_loop(retention_dir, retention_days).await;
        });

        tracing::info!(
            "[analytics] Analytics enabled (flush every {}s, retain {}d)",
            analytics_config.flush_interval_secs,
            analytics_config.retention_days
        );

        // Spawn rollup broadcaster if cluster peers are configured.
        // Pushes local analytics rollups to peers every FLAPJACK_ROLLUP_INTERVAL_SECS
        // (default 300s) so peers can serve Tier 2 cached analytics queries.
        if let Some(cluster) = crate::analytics_cluster::get_global_cluster() {
            let rollup_interval_secs: u64 = std::env::var("FLAPJACK_ROLLUP_INTERVAL_SECS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(300);
            let local_node_id = cluster.node_id().to_string();
            crate::rollup_broadcaster::spawn_rollup_broadcaster(
                Arc::clone(&analytics_engine),
                analytics_config.clone(),
                cluster,
                local_node_id,
                rollup_interval_secs,
            );
            tracing::info!(
                "[ROLLUP-BROADCAST] Broadcaster started (interval={}s)",
                rollup_interval_secs
            );
        }
    } else {
        tracing::info!("[analytics] Analytics disabled");
    }

    let metrics_state = crate::handlers::metrics::MetricsState::new();

    let usage_counters = Arc::new(dashmap::DashMap::new());

    let state = Arc::new(AppState {
        manager,
        key_store: key_store.clone(),
        replication_manager,
        ssl_manager,
        analytics_engine: Some(Arc::clone(&analytics_engine)),
        experiment_store: Some(Arc::new(ExperimentStore::new(Path::new(&data_dir))?)),
        metrics_state: Some(metrics_state.clone()),
        usage_counters: usage_counters.clone(),
        paused_indexes: crate::pause_registry::PausedIndexes::new(),
        start_time: std::time::Instant::now(),
        #[cfg(feature = "vector-search")]
        embedder_store: Arc::new(crate::embedder_store::EmbedderStore::new()),
    });

    // Startup catch-up: if replication is enabled, fetch missed ops from peers.
    // Runs in background — does not delay server startup.
    if state.replication_manager.is_some() {
        crate::startup_catchup::spawn_startup_catchup(Arc::clone(&state));

        // P0: Periodic anti-entropy sync — pulls missed ops from peers on a timer.
        // Closes the partition recovery gap: if nodes can't communicate for a while
        // but remain running, writes made during the partition are synced when
        // connectivity resumes (without requiring a restart).
        let sync_interval: u64 = std::env::var("FLAPJACK_SYNC_INTERVAL_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(60);
        crate::startup_catchup::spawn_periodic_sync(Arc::clone(&state), sync_interval);
        tracing::info!(
            "[REPL-sync] Periodic anti-entropy sync enabled (interval={}s)",
            sync_interval
        );
    }

    // Background poller: update per-tenant storage gauges every 60s
    {
        let mgr = Arc::clone(&state.manager);
        let ms = metrics_state.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(60));
            loop {
                interval.tick().await;
                let storage = mgr.all_tenant_storage();
                // Clear stale entries (tenants that were unloaded)
                ms.storage_gauges.clear();
                for (tid, bytes) in storage {
                    ms.storage_gauges.insert(tid, bytes);
                }
            }
        });
    }

    let key_routes = if let Some(ref ks) = key_store {
        Router::new()
            .route(
                "/1/keys",
                post(crate::handlers::create_key).get(crate::handlers::list_keys),
            )
            .route(
                "/1/keys/:key",
                get(crate::handlers::get_key)
                    .put(crate::handlers::update_key)
                    .delete(crate::handlers::delete_key),
            )
            .route("/1/keys/:key/restore", post(crate::handlers::restore_key))
            .route(
                "/1/keys/generateSecuredApiKey",
                post(crate::handlers::generate_secured_key),
            )
            .with_state(ks.clone())
    } else {
        Router::new()
    };

    let protected = Router::new()
        .route("/1/indexes", post(create_index))
        .route("/1/indexes", get(list_indices))
        .route("/1/indexes/:indexName/browse", post(browse_index))
        .route("/1/indexes/:indexName/clear", post(clear_index))
        .route("/1/indexes/:indexName/compact", post(compact_index))
        .route("/1/indexes/:indexName/batch", post(add_documents))
        .route("/1/indexes/:indexName/query", post(search))
        .route("/1/indexes/:indexName/deleteByQuery", post(delete_by_query))
        .route(
            "/1/indexes/:indexName/facets/:facetName/query",
            post(search_facet_values),
        )
        .route(
            "/1/indexes/:indexName/facets/:facetName/searchForFacetValues",
            post(search_facet_values),
        )
        .route("/1/indexes/:indexName/synonyms/:objectID", get(get_synonym))
        .route(
            "/1/indexes/:indexName/synonyms/:objectID",
            axum::routing::put(save_synonym),
        )
        .route(
            "/1/indexes/:indexName/synonyms/:objectID",
            delete(delete_synonym),
        )
        .route("/1/indexes/:indexName/synonyms/batch", post(save_synonyms))
        .route("/1/indexes/:indexName/synonyms/clear", post(clear_synonyms))
        .route(
            "/1/indexes/:indexName/synonyms/search",
            post(search_synonyms),
        )
        .route("/1/indexes/:indexName/rules/:objectID", get(get_rule))
        .route(
            "/1/indexes/:indexName/rules/:objectID",
            axum::routing::put(save_rule),
        )
        .route("/1/indexes/:indexName/rules/:objectID", delete(delete_rule))
        .route("/1/indexes/:indexName/rules/batch", post(save_rules))
        .route("/1/indexes/:indexName/rules/clear", post(clear_rules))
        .route("/1/indexes/:indexName/rules/search", post(search_rules))
        .route("/1/indexes/:indexName/operation", post(operation_index))
        .route(
            "/1/indexes/:indexName/export",
            get(snapshot::export_snapshot),
        )
        .route(
            "/1/indexes/:indexName/import",
            post(snapshot::import_snapshot),
        )
        .route(
            "/1/indexes/:indexName/snapshot",
            post(snapshot::snapshot_to_s3),
        )
        .route(
            "/1/indexes/:indexName/restore",
            post(snapshot::restore_from_s3),
        )
        .route(
            "/1/indexes/:indexName/snapshots",
            get(snapshot::list_s3_snapshots),
        )
        .route("/1/indexes/:indexName/queries", post(batch_search))
        .route("/1/indexes/:indexName/objects", post(get_objects))
        .route(
            "/1/indexes/:indexName/settings",
            get(crate::handlers::get_settings)
                .post(crate::handlers::set_settings)
                .put(crate::handlers::set_settings),
        )
        .route(
            "/1/indexes/:indexName/:objectID/partial",
            post(partial_update_object),
        )
        .route("/1/indexes/:indexName/:objectID", get(get_object))
        .route("/1/indexes/:indexName/:objectID", delete(delete_object))
        .route(
            "/1/indexes/:indexName/:objectID",
            axum::routing::put(put_object),
        )
        .route(
            "/1/indexes/:indexName",
            post(add_record_auto_id).delete(delete_index),
        )
        .route("/1/migrate-from-algolia", post(migrate_from_algolia))
        .route("/1/algolia-list-indexes", post(list_algolia_indexes))
        .route("/1/tasks/:task_id", get(get_task))
        .route(
            "/1/indexes/:indexName/task/:task_id",
            get(get_task_for_index),
        )
        // Query Suggestions API (Algolia-compatible paths)
        .route(
            "/1/configs",
            get(crate::handlers::query_suggestions::list_configs)
                .post(crate::handlers::query_suggestions::create_config),
        )
        .route(
            "/1/configs/:indexName",
            get(crate::handlers::query_suggestions::get_config)
                .put(crate::handlers::query_suggestions::update_config)
                .delete(crate::handlers::query_suggestions::delete_config),
        )
        .route(
            "/1/configs/:indexName/status",
            get(crate::handlers::query_suggestions::get_status),
        )
        .route(
            "/1/configs/:indexName/build",
            post(crate::handlers::query_suggestions::trigger_build),
        )
        .route(
            "/1/logs/:indexName",
            get(crate::handlers::query_suggestions::get_logs),
        )
        .with_state(state.clone());

    let usage_counters_for_mw = usage_counters.clone();
    let protected =
        protected.layer(middleware::from_fn(
            move |request: axum::extract::Request, next: middleware::Next| {
                let counters = usage_counters_for_mw.clone();
                async move {
                    crate::usage_middleware::usage_counting_layer(request, next, &counters).await
                }
            },
        ));

    let ks_for_middleware = key_store.clone();
    let auth_middleware = middleware::from_fn(
        move |mut request: axum::extract::Request, next: middleware::Next| {
            let ks = ks_for_middleware.clone();
            async move {
                if let Some(ref store) = ks {
                    request.extensions_mut().insert(store.clone());
                }
                authenticate_and_authorize(request, next).await
            }
        },
    );

    let swagger = SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi());

    // Internal replication endpoints (auth middleware applies when auth is enabled;
    // no specific ACL required beyond a valid key — relies on network isolation for peer trust)
    let internal = Router::new()
        .route(
            "/internal/replicate",
            post(crate::handlers::internal::replicate_ops),
        )
        .route("/internal/ops", get(crate::handlers::internal::get_ops))
        .route(
            "/internal/status",
            get(crate::handlers::internal::replication_status),
        )
        .route(
            "/internal/cluster/status",
            get(crate::handlers::internal::cluster_status),
        )
        .route(
            "/internal/analytics-rollup",
            post(crate::handlers::internal::receive_analytics_rollup),
        )
        .route(
            "/internal/rollup-cache",
            get(crate::handlers::internal::rollup_cache_status),
        )
        .route(
            "/internal/storage",
            get(crate::handlers::internal::storage_all),
        )
        .route(
            "/internal/storage/:indexName",
            get(crate::handlers::internal::storage_index),
        )
        .route(
            "/.well-known/acme-challenge/:token",
            get(crate::handlers::internal::acme_challenge),
        )
        .route(
            "/internal/pause/:indexName",
            post(crate::handlers::internal::pause_index),
        )
        .route(
            "/internal/resume/:indexName",
            post(crate::handlers::internal::resume_index),
        )
        .with_state(state.clone());

    // Analytics API endpoints (Algolia Analytics API v2 compatible)
    let analytics_routes = Router::new()
        .route(
            "/2/searches",
            get(crate::handlers::analytics::get_top_searches),
        )
        .route(
            "/2/searches/count",
            get(crate::handlers::analytics::get_search_count),
        )
        .route(
            "/2/searches/noResults",
            get(crate::handlers::analytics::get_no_results),
        )
        .route(
            "/2/searches/noResultRate",
            get(crate::handlers::analytics::get_no_result_rate),
        )
        .route(
            "/2/searches/noClicks",
            get(crate::handlers::analytics::get_no_clicks),
        )
        .route(
            "/2/searches/noClickRate",
            get(crate::handlers::analytics::get_no_click_rate),
        )
        .route(
            "/2/clicks/clickThroughRate",
            get(crate::handlers::analytics::get_click_through_rate),
        )
        .route(
            "/2/clicks/averageClickPosition",
            get(crate::handlers::analytics::get_average_click_position),
        )
        .route(
            "/2/clicks/positions",
            get(crate::handlers::analytics::get_click_positions),
        )
        .route(
            "/2/conversions/conversionRate",
            get(crate::handlers::analytics::get_conversion_rate),
        )
        .route("/2/hits", get(crate::handlers::analytics::get_top_hits))
        .route(
            "/2/filters",
            get(crate::handlers::analytics::get_top_filters),
        )
        .route(
            "/2/filters/noResults",
            get(crate::handlers::analytics::get_filters_no_results),
        )
        .route(
            "/2/filters/:attribute",
            get(crate::handlers::analytics::get_filter_values),
        )
        .route(
            "/2/users/count",
            get(crate::handlers::analytics::get_users_count),
        )
        .route(
            "/2/status",
            get(crate::handlers::analytics::get_analytics_status),
        )
        .route(
            "/2/devices",
            get(crate::handlers::analytics::get_device_breakdown),
        )
        .route("/2/geo", get(crate::handlers::analytics::get_geo_breakdown))
        .route(
            "/2/geo/:country",
            get(crate::handlers::analytics::get_geo_top_searches),
        )
        .route(
            "/2/geo/:country/regions",
            get(crate::handlers::analytics::get_geo_regions),
        )
        .route("/2/overview", get(crate::handlers::analytics::get_overview))
        .route(
            "/2/analytics/seed",
            post(crate::handlers::analytics::seed_analytics),
        )
        .route(
            "/2/analytics/clear",
            delete(crate::handlers::analytics::clear_analytics),
        )
        .route(
            "/2/analytics/flush",
            post(crate::handlers::analytics::flush_analytics),
        )
        .with_state(analytics_engine);

    // Analytics cleanup endpoint (needs AppState for both analytics engine + index manager)
    let analytics_cleanup_routes = Router::new()
        .route(
            "/2/analytics/cleanup",
            post(crate::handlers::analytics::cleanup_analytics),
        )
        .with_state(state.clone());

    let experiments_routes = Router::new()
        .route(
            "/2/abtests",
            post(crate::handlers::experiments::create_experiment)
                .get(crate::handlers::experiments::list_experiments),
        )
        .route(
            "/2/abtests/:id",
            get(crate::handlers::experiments::get_experiment)
                .put(crate::handlers::experiments::update_experiment)
                .delete(crate::handlers::experiments::delete_experiment),
        )
        .route(
            "/2/abtests/:id/start",
            post(crate::handlers::experiments::start_experiment),
        )
        .route(
            "/2/abtests/:id/stop",
            post(crate::handlers::experiments::stop_experiment),
        )
        .route(
            "/2/abtests/:id/conclude",
            post(crate::handlers::experiments::conclude_experiment),
        )
        .route(
            "/2/abtests/:id/results",
            get(crate::handlers::experiments::get_experiment_results),
        )
        .with_state(state.clone());

    // Insights API (event ingestion - Algolia compatible)
    let analytics_collector_for_shutdown = Arc::clone(&analytics_collector);
    let insights_routes = Router::new()
        .route("/1/events", post(crate::handlers::insights::post_events))
        .with_state(analytics_collector);

    let health_route = Router::new()
        .route("/health", get(health))
        .route("/metrics", get(crate::handlers::metrics_handler))
        .with_state(state.clone());

    let app = Router::new()
        .merge(health_route)
        .merge(swagger)
        .merge(key_routes)
        .merge(protected)
        .merge(analytics_routes)
        .merge(analytics_cleanup_routes)
        .merge(experiments_routes)
        .merge(insights_routes)
        .merge(internal);

    // Dashboard is embedded in the binary via rust-embed
    let dashboard_routes = Router::new().fallback(get(dashboard_handler));
    let app = app.nest("/dashboard", dashboard_routes);

    let max_body_mb: usize = std::env::var("FLAPJACK_MAX_BODY_MB")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(100);
    let mgr_for_pressure = Arc::clone(&state.manager);
    let default_facet_cache_cap = state
        .manager
        .facet_cache_cap
        .load(std::sync::atomic::Ordering::Relaxed);
    let memory_middleware = middleware::from_fn(
        move |request: axum::extract::Request, next: middleware::Next| {
            let mgr = mgr_for_pressure.clone();
            async move {
                crate::memory_middleware::memory_pressure_guard(
                    request,
                    next,
                    &mgr,
                    default_facet_cache_cap,
                )
                .await
            }
        },
    );
    let app = app.layer(auth_middleware);
    let app = app
        .layer(memory_middleware)
        .layer(DefaultBodyLimit::max(max_body_mb * 1024 * 1024))
        .layer(middleware::from_fn(normalize_content_type))
        .layer(CorsLayer::very_permissive().max_age(std::time::Duration::from_secs(86400)))
        .layer(middleware::from_fn(allow_private_network));

    let listener = tokio::net::TcpListener::bind(&bind_addr).await?;
    let resolved_bind_addr = listener.local_addr()?.to_string();

    // Determine auth status for banner display
    let auth_status = if no_auth {
        AuthStatus::Disabled
    } else if key_is_new {
        // First boot - show the generated key prominently
        AuthStatus::NewKey(admin_key.clone().unwrap())
    } else {
        // Subsequent boots - don't show the key (it's in .admin_key file)
        AuthStatus::KeyInFile
    };

    print_startup_banner(
        &resolved_bind_addr,
        auth_status,
        startup_start.elapsed().as_millis(),
        &data_dir,
    );

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    // --- Graceful shutdown sequence ---
    tracing::info!("[shutdown] Server stopped accepting connections, cleaning up...");

    // 1. Flush analytics buffers
    if analytics_config.enabled {
        analytics_collector_for_shutdown.shutdown(); // signals flush loop to do final flush_all() and exit
        tracing::info!("[shutdown] Analytics buffers flushed");
    }

    // 2. Drain write queues (flushes pending index writes)
    state.manager.graceful_shutdown().await;
    tracing::info!("[shutdown] All write queues drained");

    tracing::info!("[shutdown] Clean shutdown complete");
    Ok(())
}

/// Wait for SIGINT (Ctrl+C) or SIGTERM, whichever comes first.
async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to listen for ctrl+c");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to listen for SIGTERM")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => tracing::info!("[shutdown] Received SIGINT (Ctrl+C)"),
        _ = terminate => tracing::info!("[shutdown] Received SIGTERM"),
    }
}

struct DataDirProcessLock {
    file: std::fs::File,
}

impl Drop for DataDirProcessLock {
    fn drop(&mut self) {
        let _ = self.file.unlock();
    }
}

fn acquire_data_dir_process_lock(data_dir: &Path) -> Result<DataDirProcessLock, String> {
    std::fs::create_dir_all(data_dir).map_err(|e| {
        format!(
            "Failed to create data directory {}: {}",
            data_dir.display(),
            e
        )
    })?;

    let lock_path = data_dir.join(".process.lock");
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&lock_path)
        .map_err(|e| {
            format!(
                "Failed to open process lock file {}: {}",
                lock_path.display(),
                e
            )
        })?;

    match file.try_lock_exclusive() {
        Ok(()) => Ok(DataDirProcessLock { file }),
        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => Err(format!(
            "Data directory already in use: {}. Use unique --data-dir per instance.",
            lock_path.display()
        )),
        Err(e) => Err(format!(
            "Failed to acquire process lock {}: {}",
            lock_path.display(),
            e
        )),
    }
}

enum AuthStatus {
    NewKey(String),
    KeyInFile,
    Disabled,
}

fn print_startup_banner(bind_addr: &str, auth: AuthStatus, startup_ms: u128, data_dir: &str) {
    use colored::Colorize;

    let url = format!("http://{}", bind_addr);
    let version = format!("v{}", env!("CARGO_PKG_VERSION"));
    let timing = format!("ready in {}ms", startup_ms);

    println!();
    println!(
        "  {} {}  {}",
        "\u{1F95E} Flapjack".bold(),
        version.as_str().dimmed(),
        timing.as_str().dimmed(),
    );
    println!();
    println!(
        "  {}  Local:      {}",
        "\u{2192}".green(),
        url.as_str().cyan()
    );
    let dash = format!("{}/dashboard", url);
    println!(
        "  {}  Dashboard:  {}",
        "\u{2192}".green(),
        dash.as_str().cyan()
    );
    let docs = format!("{}/swagger-ui", url);
    println!(
        "  {}  API Docs:   {}",
        "\u{2192}".green(),
        docs.as_str().cyan()
    );

    match auth {
        AuthStatus::NewKey(ref key) => {
            println!();
            println!(
                "  {}",
                "! Save this API key \u{2014} it won\u{2019}t be shown again!"
                    .yellow()
                    .bold()
            );
            println!();
            println!(
                "  \u{1F511}  Admin API Key:  {}",
                key.as_str().cyan().bold().on_black()
            );
            println!();
            println!(
                "     {} Copy this key to a safe place (password manager, secrets vault)",
                "1.".cyan().bold()
            );
            println!(
                "     {} Use it to authenticate API requests:",
                "2.".cyan().bold()
            );
            println!(
                "        {}",
                format!("curl -H 'X-Algolia-API-Key: {}' \\", key).dimmed()
            );
            println!(
                "        {}",
                "     -H 'X-Algolia-Application-ID: flapjack' \\".dimmed()
            );
            println!("        {}", format!("     {}/1/indexes", url).dimmed());
            println!();
            println!(
                "     {} Stored in: {}",
                "\u{2713}".green(),
                format!("{}/.admin_key", data_dir).cyan()
            );
            println!(
                "     {} Keys hashed at rest {}",
                "\u{2713}".green(),
                "(SHA-256 + unique salt)".dimmed()
            );
            println!(
                "     {} Never commit {} to version control",
                "!".yellow(),
                ".admin_key".cyan()
            );
            println!(
                "     {} If lost: {}",
                "\u{2192}".dimmed(),
                "flapjack reset-admin-key".cyan()
            );
            println!(
                "     {} Production: set {} env var",
                "\u{2192}".dimmed(),
                "FLAPJACK_ADMIN_KEY".cyan()
            );
        }
        AuthStatus::KeyInFile => {
            let key_file = format!("{}/.admin_key", data_dir);
            println!();
            println!(
                "  {} Auth enabled  {}",
                "\u{2713}".green(),
                format!("(loaded from {})", key_file).dimmed()
            );
        }
        AuthStatus::Disabled => {
            println!();
            println!(
                "  {} {}",
                "!".yellow().bold(),
                "Auth disabled \u{2014} all routes publicly accessible".yellow()
            );
            println!(
                "    {}",
                "Only use --no-auth for local development/testing".dimmed()
            );
        }
    }
    println!();
}

async fn auto_restore_from_s3(
    data_dir: &str,
    s3_config: &flapjack::index::s3::S3Config,
    _manager: &std::sync::Arc<flapjack::IndexManager>,
) {
    let data_path = std::path::Path::new(data_dir);
    let has_tenants = data_path
        .read_dir()
        .map(|mut rd| rd.any(|e| e.ok().map(|e| e.path().is_dir()).unwrap_or(false)))
        .unwrap_or(false);
    if has_tenants {
        tracing::info!("Data dir has existing tenants, skipping S3 auto-restore");
        return;
    }

    tracing::info!("Empty data dir detected, attempting S3 auto-restore...");
    let bucket = match s3_config.clone().bucket_internal() {
        Ok(b) => b,
        Err(e) => {
            tracing::warn!("S3 auto-restore: couldn't create bucket client: {}", e);
            return;
        }
    };
    let results = match bucket
        .list("snapshots/".to_string(), Some("/".to_string()))
        .await
    {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!("S3 auto-restore: list failed: {}", e);
            return;
        }
    };
    let mut tenant_ids: Vec<String> = results
        .iter()
        .flat_map(|r| r.common_prefixes.iter().flatten())
        .filter_map(|p| {
            p.prefix
                .strip_prefix("snapshots/")
                .and_then(|s| s.strip_suffix("/"))
                .map(|s| s.to_string())
        })
        .collect();
    tenant_ids.sort();
    tenant_ids.dedup();

    if tenant_ids.is_empty() {
        tracing::info!("S3 auto-restore: no snapshots found");
        return;
    }

    tracing::info!(
        "S3 auto-restore: found {} tenants: {:?}",
        tenant_ids.len(),
        tenant_ids
    );
    for tid in &tenant_ids {
        match flapjack::index::s3::download_latest_snapshot(s3_config, tid).await {
            Ok((key, data)) => {
                let index_path = data_path.join(tid);
                if let Err(e) = flapjack::index::snapshot::import_from_bytes(&data, &index_path) {
                    tracing::error!("S3 auto-restore: failed to import {}: {}", tid, e);
                    continue;
                }
                tracing::info!(
                    "S3 auto-restore: restored {} from {} ({} bytes)",
                    tid,
                    key,
                    data.len()
                );
            }
            Err(e) => {
                tracing::warn!("S3 auto-restore: no snapshot for {}: {}", tid, e);
            }
        }
    }
}

async fn scheduled_s3_backups(
    data_dir: String,
    s3_config: flapjack::index::s3::S3Config,
    _manager: std::sync::Arc<flapjack::IndexManager>,
    interval_secs: u64,
) {
    let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(interval_secs));
    interval.tick().await;
    loop {
        interval.tick().await;
        tracing::info!("[BACKUP] Starting scheduled S3 snapshot...");
        let data_path = std::path::Path::new(&data_dir);
        let tenant_dirs: Vec<String> = match data_path.read_dir() {
            Ok(rd) => rd
                .filter_map(|e| e.ok())
                .filter(|e| e.path().is_dir())
                .filter(|e| {
                    let name = e.file_name().to_str().unwrap_or("").to_string();
                    !name.starts_with(".")
                })
                .filter_map(|e| e.file_name().into_string().ok())
                .collect(),
            Err(e) => {
                tracing::error!("[BACKUP] Failed to read data dir: {}", e);
                continue;
            }
        };
        for tid in &tenant_dirs {
            let index_path = data_path.join(tid);
            match flapjack::index::snapshot::export_to_bytes(&index_path) {
                Ok(bytes) => {
                    match flapjack::index::s3::upload_snapshot(&s3_config, tid, &bytes).await {
                        Ok(key) => {
                            let retention = std::env::var("FLAPJACK_SNAPSHOT_RETENTION")
                                .ok()
                                .and_then(|v| v.parse::<usize>().ok())
                                .unwrap_or(24);
                            let _ =
                                flapjack::index::s3::enforce_retention(&s3_config, tid, retention)
                                    .await;
                            tracing::info!("[BACKUP] {} -> {} ({} bytes)", tid, key, bytes.len());
                        }
                        Err(e) => tracing::error!("[BACKUP] upload {} failed: {}", tid, e),
                    }
                }
                Err(e) => tracing::error!("[BACKUP] export {} failed: {}", tid, e),
            }
        }
        tracing::info!(
            "[BACKUP] Scheduled snapshot complete ({} tenants)",
            tenant_dirs.len()
        );
    }
}
