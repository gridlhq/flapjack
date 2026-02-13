//! Search analytics engine powered by DataFusion + Parquet.
//!
//! Tracks search events automatically and click/conversion events via the Insights API.
//! Data is stored in Parquet files with Hive-style date partitioning and queried
//! using DataFusion SQL for efficient analytics aggregation.

pub mod aggregation;
pub mod collector;
pub mod config;
pub mod query;
pub mod retention;
pub mod schema;
pub mod seed;
pub mod writer;

pub use collector::AnalyticsCollector;
pub use config::AnalyticsConfig;
pub use query::AnalyticsQueryEngine;

use once_cell::sync::OnceCell;
use std::sync::Arc;

static GLOBAL_COLLECTOR: OnceCell<Arc<AnalyticsCollector>> = OnceCell::new();

/// Initialize the global analytics collector. Call once at startup.
pub fn init_global_collector(collector: Arc<AnalyticsCollector>) {
    let _ = GLOBAL_COLLECTOR.set(collector);
}

/// Get the global analytics collector, if initialized.
pub fn get_global_collector() -> Option<&'static Arc<AnalyticsCollector>> {
    GLOBAL_COLLECTOR.get()
}
