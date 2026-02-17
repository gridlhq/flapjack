pub mod acme;
pub mod config;
pub mod error;
pub mod manager;

use once_cell::sync::OnceCell;
use std::sync::Arc;

pub use acme::AcmeClient;
pub use config::SslConfig;
pub use error::{FlapjackError, Result};
pub use manager::SslManager;

static GLOBAL_SSL_MANAGER: OnceCell<Arc<manager::SslManager>> = OnceCell::new();

pub fn set_global_manager(manager: Arc<manager::SslManager>) {
    let _ = GLOBAL_SSL_MANAGER.set(manager);
}

pub fn get_global_manager() -> Option<Arc<manager::SslManager>> {
    GLOBAL_SSL_MANAGER.get().map(Arc::clone)
}
