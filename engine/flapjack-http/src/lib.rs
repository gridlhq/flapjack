pub mod analytics_cluster;
pub mod auth;
pub mod dto;
pub mod filter_parser;
pub mod handlers;
pub mod memory_middleware;
pub mod middleware;
pub mod openapi;
pub mod pause_registry;
pub mod rollup_broadcaster;
pub mod server;
pub mod startup_catchup;
pub mod usage_middleware;

#[cfg(feature = "vector-search")]
pub mod embedder_store;
#[cfg(feature = "vector-search")]
pub mod fusion;

pub use server::serve;
