pub mod config;
pub mod manager;
pub mod peer;
pub mod task;
pub mod types;

use once_cell::sync::OnceCell;
use std::sync::Arc;

static GLOBAL_REPLICATION_MANAGER: OnceCell<Arc<manager::ReplicationManager>> = OnceCell::new();

/// Set the global replication manager (called once during server startup)
pub fn set_global_manager(manager: Arc<manager::ReplicationManager>) {
    let _ = GLOBAL_REPLICATION_MANAGER.set(manager);
}

/// Get the global replication manager if replication is enabled
pub fn get_global_manager() -> Option<Arc<manager::ReplicationManager>> {
    GLOBAL_REPLICATION_MANAGER.get().map(Arc::clone)
}
