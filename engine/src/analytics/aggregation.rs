use dashmap::DashMap;
use std::time::Instant;

/// Deduplicates rapid sequential keystrokes into a single "search" for analytics.
///
/// Algolia aggregates as-you-type queries: if the same user searches the same index
/// within 30 seconds, only the final query in that window counts as one search.
///
/// Also handles pagination dedup: same user + same index + same query + same filters
/// but different page = same search session (not a new search).
pub struct QueryAggregator {
    /// Maps (user_identifier, index_name) -> window state
    windows: DashMap<(String, String), AggWindow>,
    window_secs: u64,
}

struct AggWindow {
    last_seen: Instant,
    /// The "final" query text in this window (gets replaced on each keystroke)
    final_query: String,
    /// Filters of the last search (for pagination dedup)
    filters: Option<String>,
}

impl QueryAggregator {
    pub fn new(window_secs: u64) -> Self {
        Self {
            windows: DashMap::new(),
            window_secs,
        }
    }

    /// Returns true if this search should be counted as a new search
    /// (i.e., not a continuation of a previous as-you-type sequence or pagination).
    /// Also updates the window with the new query.
    pub fn should_count(&self, user_id: &str, index_name: &str, query: &str) -> bool {
        self.should_count_with_filters(user_id, index_name, query, None)
    }

    /// Like `should_count` but also handles pagination dedup:
    /// same user + index + query + filters within the window = same session.
    pub fn should_count_with_filters(
        &self,
        user_id: &str,
        index_name: &str,
        query: &str,
        filters: Option<&str>,
    ) -> bool {
        let key = (user_id.to_string(), index_name.to_string());
        let now = Instant::now();

        if let Some(mut entry) = self.windows.get_mut(&key) {
            let elapsed = now.duration_since(entry.last_seen).as_secs();
            if elapsed < self.window_secs {
                // Within window — check if it's typing continuation or pagination
                entry.last_seen = now;

                // Pagination dedup: same query + same filters = same session
                let same_query = entry.final_query == query;
                let same_filters = entry.filters.as_deref() == filters;
                if same_query && same_filters {
                    // Page change on same search — don't count
                    return false;
                }

                // Different query — typing continuation, update final_query
                entry.final_query = query.to_string();
                entry.filters = filters.map(|s| s.to_string());
                return false;
            }
            // Window expired — this is a new search
            entry.last_seen = now;
            entry.final_query = query.to_string();
            entry.filters = filters.map(|s| s.to_string());
            true
        } else {
            // First search from this user/index combo
            self.windows.insert(
                key,
                AggWindow {
                    last_seen: now,
                    final_query: query.to_string(),
                    filters: filters.map(|s| s.to_string()),
                },
            );
            true
        }
    }

    /// Periodic cleanup of expired windows to prevent unbounded memory growth.
    pub fn evict_expired(&self) {
        let cutoff_secs = self.window_secs * 2;
        self.windows
            .retain(|_, v| v.last_seen.elapsed().as_secs() < cutoff_secs);
    }
}
