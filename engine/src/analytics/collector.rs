use dashmap::DashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::Notify;

use super::aggregation::QueryAggregator;
use super::config::AnalyticsConfig;
use super::schema::{InsightEvent, SearchEvent};
use super::writer;

/// Central analytics event collector.
///
/// Buffers events in memory and flushes to Parquet files either on a timer
/// or when the buffer reaches a threshold size. Uses `std::mem::take` to
/// swap the buffer without holding the lock during I/O.
pub struct AnalyticsCollector {
    config: AnalyticsConfig,
    search_buffer: Mutex<Vec<SearchEvent>>,
    insight_buffer: Mutex<Vec<InsightEvent>>,
    aggregator: QueryAggregator,
    /// queryID -> (query, index_name, timestamp_ms) for correlating clicks with searches
    query_id_cache: DashMap<String, QueryIdEntry>,
    shutdown: Notify,
}

#[derive(Clone)]
pub struct QueryIdEntry {
    pub query: String,
    pub index_name: String,
    pub timestamp_ms: i64,
}

impl AnalyticsCollector {
    pub fn new(config: AnalyticsConfig) -> Arc<Self> {
        Arc::new(Self {
            config,
            search_buffer: Mutex::new(Vec::with_capacity(1024)),
            insight_buffer: Mutex::new(Vec::with_capacity(256)),
            aggregator: QueryAggregator::new(30),
            query_id_cache: DashMap::new(),
            shutdown: Notify::new(),
        })
    }

    pub fn config(&self) -> &AnalyticsConfig {
        &self.config
    }

    /// Record a search event. Called from the search path after results are computed.
    pub fn record_search(&self, event: SearchEvent) {
        if !self.config.enabled {
            return;
        }

        // Store queryID mapping for click correlation
        if let Some(ref qid) = event.query_id {
            self.query_id_cache.insert(
                qid.clone(),
                QueryIdEntry {
                    query: event.query.clone(),
                    index_name: event.index_name.clone(),
                    timestamp_ms: event.timestamp_ms,
                },
            );
        }

        // Check aggregation: should this count as a distinct search?
        let user_id = event
            .user_token
            .as_deref()
            .or(event.user_ip.as_deref())
            .unwrap_or("anonymous");
        let _is_new_search = self
            .aggregator
            .should_count(user_id, &event.index_name, &event.query);
        // We always store the raw event; aggregation is applied at query time.
        // The aggregator is kept for future use (e.g. deduped search count queries).

        let should_flush = {
            let mut buf = self.search_buffer.lock().unwrap();
            buf.push(event);
            buf.len() >= self.config.flush_size
        };

        if should_flush {
            self.flush_searches();
        }
    }

    /// Record an insight event (click, conversion, view).
    pub fn record_insight(&self, event: InsightEvent) {
        if !self.config.enabled {
            return;
        }

        let should_flush = {
            let mut buf = self.insight_buffer.lock().unwrap();
            buf.push(event);
            buf.len() >= self.config.flush_size
        };

        if should_flush {
            self.flush_insights();
        }
    }

    /// Look up a queryID to correlate with the original search.
    pub fn lookup_query_id(&self, query_id: &str) -> Option<QueryIdEntry> {
        self.query_id_cache.get(query_id).map(|e| e.clone())
    }

    /// Flush search events to Parquet. Swaps buffer to avoid holding lock during I/O.
    pub fn flush_searches(&self) {
        let events = {
            let mut buf = self.search_buffer.lock().unwrap();
            std::mem::take(&mut *buf)
        };
        if events.is_empty() {
            return;
        }

        // Group events by index_name for per-index Parquet files
        let mut by_index: std::collections::HashMap<String, Vec<SearchEvent>> =
            std::collections::HashMap::new();
        for event in events {
            by_index
                .entry(event.index_name.clone())
                .or_default()
                .push(event);
        }

        for (index_name, index_events) in by_index {
            let dir = self.config.searches_dir(&index_name);
            if let Err(e) = writer::flush_search_events(&index_events, &dir) {
                tracing::error!(
                    "[analytics] Failed to flush {} search events for {}: {}",
                    index_events.len(),
                    index_name,
                    e
                );
            } else {
                tracing::debug!(
                    "[analytics] Flushed {} search events for {}",
                    index_events.len(),
                    index_name
                );
            }
        }
    }

    /// Flush insight events to Parquet.
    pub fn flush_insights(&self) {
        let events = {
            let mut buf = self.insight_buffer.lock().unwrap();
            std::mem::take(&mut *buf)
        };
        if events.is_empty() {
            return;
        }

        let mut by_index: std::collections::HashMap<String, Vec<InsightEvent>> =
            std::collections::HashMap::new();
        for event in events {
            by_index.entry(event.index.clone()).or_default().push(event);
        }

        for (index_name, index_events) in by_index {
            let dir = self.config.events_dir(&index_name);
            if let Err(e) = writer::flush_insight_events(&index_events, &dir) {
                tracing::error!(
                    "[analytics] Failed to flush {} insight events for {}: {}",
                    index_events.len(),
                    index_name,
                    e
                );
            } else {
                tracing::debug!(
                    "[analytics] Flushed {} insight events for {}",
                    index_events.len(),
                    index_name
                );
            }
        }
    }

    /// Flush all buffers (called at shutdown or periodically).
    pub fn flush_all(&self) {
        self.flush_searches();
        self.flush_insights();
    }

    /// Start the background flush loop. Should be spawned as a tokio task.
    pub async fn run_flush_loop(self: Arc<Self>) {
        let interval = tokio::time::Duration::from_secs(self.config.flush_interval_secs);
        let mut ticker = tokio::time::interval(interval);
        ticker.tick().await; // skip the first immediate tick

        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    self.flush_all();
                    self.aggregator.evict_expired();
                    self.evict_old_query_ids();
                }
                _ = self.shutdown.notified() => {
                    self.flush_all();
                    tracing::info!("[analytics] Flush loop shutting down");
                    break;
                }
            }
        }
    }

    /// Signal the flush loop to stop.
    pub fn shutdown(&self) {
        self.shutdown.notify_one();
    }

    /// Evict queryID entries older than 1 hour.
    fn evict_old_query_ids(&self) {
        let cutoff = chrono::Utc::now().timestamp_millis() - 3_600_000;
        self.query_id_cache.retain(|_, v| v.timestamp_ms > cutoff);
    }
}
