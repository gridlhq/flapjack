//! Startup catch-up and periodic anti-entropy sync.
//!
//! - `spawn_startup_catchup`: runs once at boot (3s delay) to fetch missed ops.
//! - `spawn_periodic_sync`:  runs on a timer (P0) to close partition gaps without restart.
//!
//! Both use the same core logic: iterate local tenant dirs, compare local oplog
//! seq against peers, pull and apply any missing ops via LWW conflict resolution.

use crate::handlers::internal::apply_ops_to_manager;
use crate::handlers::AppState;
use std::sync::Arc;

/// Spawn a background task that catches up all local tenants from peers.
/// Returns immediately — the catch-up runs concurrently with normal traffic.
pub fn spawn_startup_catchup(state: Arc<AppState>) {
    tokio::spawn(async move {
        run_startup_catchup(state).await;
    });
}

/// Run startup catch-up synchronously. Public for testing.
pub async fn run_startup_catchup(state: Arc<AppState>) {
    if state.replication_manager.is_none() {
        return; // Standalone mode — nothing to do
    }

    // Wait briefly so the server is accepting requests before catch-up starts.
    // This avoids log noise during normal single-node startup.
    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

    tracing::info!("[REPL-catchup] Starting startup catch-up from peers");
    catchup_all_tenants(&state, "REPL-catchup").await;
    tracing::info!("[REPL-catchup] Startup catch-up complete");
}

/// Run one round of catch-up from peers for all local tenants.
/// This is the core anti-entropy sync logic used by both startup catch-up
/// and the periodic background task (P0).
/// Public for testing.
pub async fn run_periodic_catchup(state: Arc<AppState>) {
    if state.replication_manager.is_none() {
        return;
    }
    catchup_all_tenants(&state, "REPL-sync").await;
}

/// Spawn a background task that runs catch-up from peers on a timer.
/// This is the P0 fix for network partition recovery without restart.
/// Configurable via FLAPJACK_SYNC_INTERVAL_SECS (default 60).
pub fn spawn_periodic_sync(state: Arc<AppState>, interval_secs: u64) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(interval_secs));
        // If a sync cycle runs longer than the interval, skip missed ticks
        // rather than bursting back-to-back syncs.
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        interval.tick().await; // skip first immediate tick

        loop {
            interval.tick().await;
            tracing::debug!("[REPL-sync] Periodic sync starting");
            run_periodic_catchup(Arc::clone(&state)).await;
        }
    });
}

/// Core catch-up logic shared by startup and periodic sync.
/// Iterates all local tenant directories, compares local oplog sequence
/// against peers, and pulls any missed ops.
async fn catchup_all_tenants(state: &AppState, log_prefix: &str) {
    let repl_mgr = match &state.replication_manager {
        Some(r) => Arc::clone(r),
        None => return,
    };

    let entries = match std::fs::read_dir(&state.manager.base_path) {
        Ok(e) => e,
        Err(e) => {
            tracing::warn!("[{}] Cannot read data dir: {}", log_prefix, e);
            return;
        }
    };

    for entry in entries.flatten() {
        if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            continue;
        }
        let tenant_id = entry.file_name().to_string_lossy().to_string();

        // Skip hidden dirs or non-index dirs
        if tenant_id.starts_with('.') {
            continue;
        }

        let local_seq = state
            .manager
            .get_oplog(&tenant_id)
            .map(|ol| ol.current_seq())
            .unwrap_or(0);

        match repl_mgr.catch_up_from_peer(&tenant_id, local_seq).await {
            Ok(ops) if !ops.is_empty() => {
                tracing::info!(
                    "[{}] {} missed ops for tenant '{}' (local_seq={})",
                    log_prefix,
                    ops.len(),
                    tenant_id,
                    local_seq
                );
                match apply_ops_to_manager(&state.manager, &tenant_id, &ops).await {
                    Ok(applied_seq) => tracing::info!(
                        "[{}] Applied ops up to seq {} for tenant '{}'",
                        log_prefix,
                        applied_seq,
                        tenant_id
                    ),
                    Err(e) => tracing::error!(
                        "[{}] Failed to apply ops for '{}': {}",
                        log_prefix,
                        tenant_id,
                        e
                    ),
                }
            }
            Ok(_) => {
                tracing::debug!("[{}] Tenant '{}' is up-to-date", log_prefix, tenant_id);
            }
            Err(e) => {
                tracing::debug!(
                    "[{}] Could not reach peer for '{}': {}",
                    log_prefix,
                    tenant_id,
                    e
                );
            }
        }
    }
}
