use std::num::NonZeroUsize;
use std::sync::{Arc, Mutex};

use dashmap::DashMap;
use flapjack::error::FlapjackError;
use flapjack::index::settings::IndexSettings;
use flapjack::vector::config::EmbedderConfig;
use flapjack::vector::embedder::{create_embedder, Embedder};

/// Application-level cache for instantiated embedders.
///
/// Keyed by (tenant_id, embedder_name). Avoids re-creating HTTP clients
/// and parsing config on every search request.
pub struct EmbedderStore {
    cache: DashMap<(String, String), Arc<Embedder>>,
    pub query_cache: QueryEmbeddingCache,
}

impl EmbedderStore {
    pub fn new() -> Self {
        Self {
            cache: DashMap::new(),
            query_cache: QueryEmbeddingCache::new(1000),
        }
    }

    /// Get or create an embedder for the given tenant and embedder name.
    ///
    /// Checks the cache first; on miss, parses the embedder config from
    /// `settings.embedders`, creates the embedder, caches it, and returns it.
    pub fn get_or_create(
        &self,
        tenant_id: &str,
        embedder_name: &str,
        settings: &IndexSettings,
    ) -> Result<Arc<Embedder>, FlapjackError> {
        let key = (tenant_id.to_string(), embedder_name.to_string());

        // Cache hit
        if let Some(cached) = self.cache.get(&key) {
            return Ok(Arc::clone(&cached));
        }

        // Cache miss — parse config and create embedder
        let embedders_map = settings.embedders.as_ref().ok_or_else(|| {
            FlapjackError::InvalidQuery(format!(
                "no embedders configured for tenant '{}'",
                tenant_id
            ))
        })?;

        let raw_config = embedders_map.get(embedder_name).ok_or_else(|| {
            FlapjackError::InvalidQuery(format!(
                "embedder '{}' not found in settings for tenant '{}'",
                embedder_name, tenant_id
            ))
        })?;

        let config: EmbedderConfig = serde_json::from_value(raw_config.clone()).map_err(|e| {
            FlapjackError::InvalidQuery(format!(
                "invalid embedder config for '{}': {}",
                embedder_name, e
            ))
        })?;

        let embedder = create_embedder(&config).map_err(|e| {
            FlapjackError::InvalidQuery(format!(
                "failed to create embedder '{}': {}",
                embedder_name, e
            ))
        })?;

        let arc = Arc::new(embedder);
        self.cache.insert(key, Arc::clone(&arc));
        Ok(arc)
    }

    /// Remove all cached embedders for a tenant.
    ///
    /// Called when settings change to ensure the next search picks up
    /// the new embedder configuration.
    pub fn invalidate(&self, tenant_id: &str) {
        self.cache.retain(|(tid, _), _| tid != tenant_id);
    }
}

/// LRU cache for query embedding vectors.
///
/// Prevents re-embedding identical queries (typeahead, pagination, repeated searches).
/// Keyed by (embedder_name, query_text).
pub struct QueryEmbeddingCache {
    inner: Mutex<lru::LruCache<(String, String), Vec<f32>>>,
}

impl QueryEmbeddingCache {
    pub fn new(capacity: usize) -> Self {
        Self {
            inner: Mutex::new(lru::LruCache::new(
                NonZeroUsize::new(capacity).unwrap_or(NonZeroUsize::new(1).unwrap()),
            )),
        }
    }

    /// Look up a cached query vector.
    pub fn get(&self, embedder: &str, query: &str) -> Option<Vec<f32>> {
        let key = (embedder.to_string(), query.to_string());
        self.inner.lock().ok()?.get(&key).cloned()
    }

    /// Insert a query vector into the cache.
    pub fn insert(&self, embedder: &str, query: &str, vector: Vec<f32>) {
        let key = (embedder.to_string(), query.to_string());
        if let Ok(mut cache) = self.inner.lock() {
            cache.put(key, vector);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flapjack::vector::config::EmbedderSource;
    use std::collections::HashMap;

    fn settings_with_user_provided_embedder(name: &str, dims: usize) -> IndexSettings {
        let mut embedders = HashMap::new();
        embedders.insert(
            name.to_string(),
            serde_json::json!({
                "source": "userProvided",
                "dimensions": dims
            }),
        );
        IndexSettings {
            embedders: Some(embedders),
            ..Default::default()
        }
    }

    // ── EmbedderStore tests (6.7) ──

    #[test]
    fn test_embedder_store_create_from_config() {
        let store = EmbedderStore::new();
        let settings = settings_with_user_provided_embedder("default", 384);

        let embedder = store
            .get_or_create("tenant1", "default", &settings)
            .unwrap();
        assert_eq!(embedder.dimensions(), 384);
        assert_eq!(embedder.source(), EmbedderSource::UserProvided);
    }

    #[test]
    fn test_embedder_store_returns_cached() {
        let store = EmbedderStore::new();
        let settings = settings_with_user_provided_embedder("default", 768);

        let e1 = store.get_or_create("t1", "default", &settings).unwrap();
        let e2 = store.get_or_create("t1", "default", &settings).unwrap();
        // Same Arc pointer
        assert!(Arc::ptr_eq(&e1, &e2));
    }

    #[test]
    fn test_embedder_store_missing_embedder_returns_error() {
        let store = EmbedderStore::new();
        let settings = settings_with_user_provided_embedder("default", 384);

        let result = store.get_or_create("t1", "nonexistent", &settings);
        assert!(result.is_err());
    }

    #[test]
    fn test_embedder_store_missing_settings_returns_error() {
        let store = EmbedderStore::new();
        let settings = IndexSettings::default(); // no embedders

        let result = store.get_or_create("t1", "default", &settings);
        assert!(result.is_err());
    }

    #[test]
    fn test_embedder_store_invalidate_clears_cache() {
        let store = EmbedderStore::new();
        let settings = settings_with_user_provided_embedder("default", 384);

        let e1 = store.get_or_create("t1", "default", &settings).unwrap();
        store.invalidate("t1");
        let e2 = store.get_or_create("t1", "default", &settings).unwrap();
        // After invalidation, should be a fresh embedder (different Arc)
        assert!(!Arc::ptr_eq(&e1, &e2));
    }

    // ── QueryEmbeddingCache tests (6.14) ──

    #[test]
    fn test_query_cache_hit() {
        let cache = QueryEmbeddingCache::new(10);
        cache.insert("emb1", "hello world", vec![0.1, 0.2, 0.3]);
        let result = cache.get("emb1", "hello world");
        assert_eq!(result, Some(vec![0.1, 0.2, 0.3]));
    }

    #[test]
    fn test_query_cache_miss() {
        let cache = QueryEmbeddingCache::new(10);
        cache.insert("emb1", "hello world", vec![0.1, 0.2, 0.3]);
        let result = cache.get("emb1", "different query");
        assert!(result.is_none());
    }

    #[test]
    fn test_query_cache_eviction() {
        let cache = QueryEmbeddingCache::new(2);
        cache.insert("emb", "q1", vec![1.0]);
        cache.insert("emb", "q2", vec![2.0]);
        cache.insert("emb", "q3", vec![3.0]); // evicts q1

        assert!(cache.get("emb", "q1").is_none()); // evicted
        assert_eq!(cache.get("emb", "q2"), Some(vec![2.0]));
        assert_eq!(cache.get("emb", "q3"), Some(vec![3.0]));
    }
}
