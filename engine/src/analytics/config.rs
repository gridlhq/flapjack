use std::path::PathBuf;

/// Configuration for the analytics subsystem, loaded from environment variables.
#[derive(Debug, Clone)]
pub struct AnalyticsConfig {
    /// Whether analytics collection is enabled.
    pub enabled: bool,
    /// Base directory for analytics Parquet files.
    pub data_dir: PathBuf,
    /// How often to flush buffered events to disk (seconds).
    pub flush_interval_secs: u64,
    /// Flush when buffer reaches this many events.
    pub flush_size: usize,
    /// Delete Parquet files older than this many days.
    pub retention_days: u32,
}

impl AnalyticsConfig {
    /// Load config from environment variables with sensible defaults.
    pub fn from_env() -> Self {
        let base_data_dir =
            std::env::var("FLAPJACK_DATA_DIR").unwrap_or_else(|_| "./data".to_string());
        let analytics_dir = std::env::var("FLAPJACK_ANALYTICS_DIR")
            .unwrap_or_else(|_| format!("{}/analytics", base_data_dir));

        Self {
            enabled: std::env::var("FLAPJACK_ANALYTICS_ENABLED")
                .map(|v| v != "false" && v != "0")
                .unwrap_or(true),
            data_dir: PathBuf::from(analytics_dir),
            flush_interval_secs: std::env::var("FLAPJACK_ANALYTICS_FLUSH_INTERVAL")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(60),
            flush_size: std::env::var("FLAPJACK_ANALYTICS_FLUSH_SIZE")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(10_000),
            retention_days: std::env::var("FLAPJACK_ANALYTICS_RETENTION_DAYS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(90),
        }
    }

    /// Config with analytics disabled (for tests).
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            data_dir: PathBuf::from("/tmp/flapjack-analytics-disabled"),
            flush_interval_secs: 3600,
            flush_size: 100_000,
            retention_days: 90,
        }
    }

    /// Path to search events for a given index.
    pub fn searches_dir(&self, index_name: &str) -> PathBuf {
        self.data_dir.join(index_name).join("searches")
    }

    /// Path to insight events for a given index.
    pub fn events_dir(&self, index_name: &str) -> PathBuf {
        self.data_dir.join(index_name).join("events")
    }
}
