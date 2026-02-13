pub mod config;
pub mod manager;
pub mod acme;

use once_cell::sync::OnceCell;
use std::sync::Arc;

pub use config::SslConfig;
pub use manager::SslManager;
pub use acme::AcmeClient;

static GLOBAL_SSL_MANAGER: OnceCell<Arc<manager::SslManager>> = OnceCell::new();

pub fn set_global_manager(manager: Arc<manager::SslManager>) {
    let _ = GLOBAL_SSL_MANAGER.set(manager);
}

pub fn get_global_manager() -> Option<Arc<manager::SslManager>> {
    GLOBAL_SSL_MANAGER.get().map(Arc::clone)
}
