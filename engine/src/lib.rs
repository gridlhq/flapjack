//! # Flapjack
//!
//! A full-text search engine library with typo tolerance, faceting, and
//! Algolia-compatible document conventions. Built on [Tantivy](https://github.com/quickwit-oss/tantivy).
//!
//! Flapjack can be used as an embedded library in desktop apps, CLI tools, or
//! custom web servers — or run as a standalone HTTP service via the companion
//! `flapjack-server` crate.
//!
//! ## Quick start
//!
//! ```rust,no_run
//! use flapjack::index::Index;
//! use serde_json::json;
//!
//! # fn main() -> flapjack::Result<()> {
//! // Create an index (creates directory if needed)
//! let index = Index::create_in_dir("./my_index")?;
//!
//! // Add documents — accepts "objectID" (Algolia convention) or "_id"
//! index.add_documents_simple(&[
//!     json!({"objectID": "1", "title": "MacBook Pro", "price": 2399}),
//!     json!({"objectID": "2", "title": "iPhone 15", "price": 999}),
//! ])?;
//!
//! // Documents are immediately searchable after add_documents_simple
//! let reader = index.reader();
//! let searcher = reader.searcher();
//! let count: usize = searcher.segment_readers()
//!     .iter().map(|r| r.num_docs() as usize).sum();
//! assert_eq!(count, 2);
//! # Ok(())
//! # }
//! ```
//!
//! ## Multi-tenant search with [`IndexManager`]
//!
//! ```rust,no_run
//! use flapjack::IndexManager;
//! use flapjack::types::{Document, FieldValue};
//! use std::collections::HashMap;
//!
//! # fn main() -> flapjack::Result<()> {
//! let manager = IndexManager::new("./data"); // Returns Arc<IndexManager>
//! manager.create_tenant("products")?;
//!
//! let results = manager.search("products", "laptop", None, None, 10)?;
//! println!("Found {} hits", results.total);
//! # Ok(())
//! # }
//! ```
//!
//! ## Feature flags
//!
//! | Feature | Dependencies | Use case |
//! |---------|-------------|----------|
//! | `axum-support` | axum | [`FlapjackError`] implements `IntoResponse` |
//! | `s3-snapshots` | rust-s3, flate2, tar | S3 backup/restore via [`index::s3`] and [`index::snapshot`] |
//! | `openapi` | utoipa | OpenAPI schema generation |
//!
//! All features are enabled by default. Use `default-features = false` for a
//! minimal dependency footprint (24% fewer deps).
//!
//! See [LIB.md](https://github.com/stuartcrobinson/flapjack202511/blob/main/LIB.md)
//! for the full embedding guide.

pub mod error;
pub mod index;
pub mod query;
pub mod tokenizer;
pub mod types;

#[cfg(feature = "analytics")]
pub mod analytics;

pub use error::{FlapjackError, Result};
pub use index::{manager::IndexManager, Index, ManagedIndexWriter};
pub use query::{QueryExecutor, QueryParser};
pub use types::*;

pub use index::get_global_budget;
pub use index::memory::{MemoryBudget, MemoryBudgetConfig};
pub use index::memory_observer::{MemoryObserver, MemoryStats, PressureLevel};
pub use types::{FacetCount, FacetRequest};

// Re-export from flapjack-ssl
pub use flapjack_ssl::{SslConfig, SslManager};

pub use index::reset_global_budget_for_test;

/// Initialize configuration from environment variables.
///
/// Currently reads `FLAPJACK_MAX_BUFFER_MB` (and other `FLAPJACK_*` vars)
/// to pre-populate defaults. Call once at startup if desired.
pub fn init_from_env() {
    std::env::var("FLAPJACK_MAX_BUFFER_MB").ok();
}
