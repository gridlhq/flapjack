use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};
use usearch::ffi::{IndexOptions, MetricKind, ScalarKind};
use usearch::Index;

use super::{VectorError, VectorSearchResult};

/// Bidirectional mapping between string document IDs and usearch u64 keys.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct IdMap {
    doc_to_key: HashMap<String, u64>,
    key_to_doc: HashMap<u64, String>,
    next_key: u64,
}

impl IdMap {
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert a doc_id, returning its u64 key. Reuses existing key if already present.
    pub fn insert(&mut self, doc_id: &str) -> u64 {
        if let Some(&existing) = self.doc_to_key.get(doc_id) {
            return existing;
        }
        let key = self.next_key;
        self.next_key += 1;
        self.doc_to_key.insert(doc_id.to_owned(), key);
        self.key_to_doc.insert(key, doc_id.to_owned());
        key
    }

    pub fn get_key(&self, doc_id: &str) -> Option<u64> {
        self.doc_to_key.get(doc_id).copied()
    }

    pub fn get_doc(&self, key: u64) -> Option<&str> {
        self.key_to_doc.get(&key).map(|s| s.as_str())
    }

    pub fn remove_by_doc(&mut self, doc_id: &str) -> Option<u64> {
        let key = self.doc_to_key.remove(doc_id)?;
        self.key_to_doc.remove(&key);
        Some(key)
    }

    pub fn remove_by_key(&mut self, key: u64) -> Option<String> {
        let doc_id = self.key_to_doc.remove(&key)?;
        self.doc_to_key.remove(&doc_id);
        Some(doc_id)
    }

    pub fn len(&self) -> usize {
        self.doc_to_key.len()
    }

    pub fn is_empty(&self) -> bool {
        self.doc_to_key.is_empty()
    }
}

/// HNSW vector index wrapping usearch with string doc ID mapping.
pub struct VectorIndex {
    inner: Index,
    id_map: IdMap,
    dimensions: usize,
}

impl VectorIndex {
    pub fn new(dimensions: usize, metric: MetricKind) -> Result<Self, VectorError> {
        let options = IndexOptions {
            dimensions,
            metric,
            quantization: ScalarKind::F32,
            connectivity: 0,
            expansion_add: 0,
            expansion_search: 0,
            multi: false,
        };
        let inner = Index::new(&options).map_err(|e| VectorError::HnswError(e.to_string()))?;
        Ok(Self {
            inner,
            id_map: IdMap::new(),
            dimensions,
        })
    }

    pub fn add(&mut self, doc_id: &str, vector: &[f32]) -> Result<(), VectorError> {
        if vector.len() != self.dimensions {
            return Err(VectorError::DimensionMismatch {
                expected: self.dimensions,
                got: vector.len(),
            });
        }

        if let Some(key) = self.id_map.get_key(doc_id) {
            // Replace: remove old vector, re-add with same key
            let _ = self
                .inner
                .remove(key)
                .map_err(|e| VectorError::HnswError(e.to_string()))?;
            self.inner
                .add(key, vector)
                .map_err(|e| VectorError::HnswError(e.to_string()))?;
        } else {
            let key = self.id_map.insert(doc_id);
            self.inner
                .reserve(self.id_map.len())
                .map_err(|e| VectorError::HnswError(e.to_string()))?;
            self.inner
                .add(key, vector)
                .map_err(|e| VectorError::HnswError(e.to_string()))?;
        }
        Ok(())
    }

    pub fn remove(&mut self, doc_id: &str) -> Result<(), VectorError> {
        let key = self
            .id_map
            .get_key(doc_id)
            .ok_or_else(|| VectorError::DocumentNotFound {
                doc_id: doc_id.to_owned(),
            })?;
        let _ = self
            .inner
            .remove(key)
            .map_err(|e| VectorError::HnswError(e.to_string()))?;
        self.id_map.remove_by_doc(doc_id);
        Ok(())
    }

    pub fn search(&self, query: &[f32], k: usize) -> Result<Vec<VectorSearchResult>, VectorError> {
        if query.len() != self.dimensions {
            return Err(VectorError::DimensionMismatch {
                expected: self.dimensions,
                got: query.len(),
            });
        }
        if self.id_map.is_empty() {
            return Ok(Vec::new());
        }
        let matches = self
            .inner
            .search(query, k)
            .map_err(|e| VectorError::HnswError(e.to_string()))?;
        let mut results = Vec::with_capacity(matches.keys.len());
        for (key, distance) in matches.keys.iter().zip(matches.distances.iter()) {
            if let Some(doc_id) = self.id_map.get_doc(*key) {
                results.push(VectorSearchResult {
                    doc_id: doc_id.to_owned(),
                    distance: *distance,
                });
            }
        }
        Ok(results)
    }

    pub fn len(&self) -> usize {
        self.id_map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.id_map.is_empty()
    }

    pub fn dimensions(&self) -> usize {
        self.dimensions
    }

    pub fn memory_usage(&self) -> usize {
        self.inner.memory_usage()
    }

    pub fn save(&self, dir: &Path) -> Result<(), VectorError> {
        std::fs::create_dir_all(dir)?;

        let index_path = dir.join("index.usearch");
        let index_path_str = index_path
            .to_str()
            .ok_or_else(|| VectorError::InvalidPath(format!("{}", index_path.display())))?;
        self.inner
            .save(index_path_str)
            .map_err(|e| VectorError::HnswError(e.to_string()))?;

        let meta = PersistenceMeta {
            id_map: &self.id_map,
            dimensions: self.dimensions,
        };
        let meta_json = serde_json::to_string_pretty(&meta)
            .map_err(|e| VectorError::SerializationError(e.to_string()))?;
        std::fs::write(dir.join("id_map.json"), meta_json)?;

        Ok(())
    }

    pub fn load(dir: &Path, metric: MetricKind) -> Result<Self, VectorError> {
        let meta_path = dir.join("id_map.json");
        let meta_json = std::fs::read_to_string(&meta_path)?;
        let meta: OwnedPersistenceMeta = serde_json::from_str(&meta_json)
            .map_err(|e| VectorError::SerializationError(e.to_string()))?;

        let options = IndexOptions {
            dimensions: meta.dimensions,
            metric,
            quantization: ScalarKind::F32,
            connectivity: 0,
            expansion_add: 0,
            expansion_search: 0,
            multi: false,
        };
        let inner = Index::new(&options).map_err(|e| VectorError::HnswError(e.to_string()))?;
        inner
            .reserve(meta.id_map.len())
            .map_err(|e| VectorError::HnswError(e.to_string()))?;

        let index_path = dir.join("index.usearch");
        let index_path_str = index_path
            .to_str()
            .ok_or_else(|| VectorError::InvalidPath(format!("{}", index_path.display())))?;

        // Only load HNSW data if the index file has content (empty index saves a 0-byte file or may not exist)
        if index_path.exists() && std::fs::metadata(&index_path)?.len() > 0 {
            inner
                .load(index_path_str)
                .map_err(|e| VectorError::HnswError(e.to_string()))?;
        }

        Ok(Self {
            inner,
            id_map: meta.id_map,
            dimensions: meta.dimensions,
        })
    }
}

/// Serialization helper for save — borrows IdMap.
#[derive(Serialize)]
struct PersistenceMeta<'a> {
    id_map: &'a IdMap,
    dimensions: usize,
}

/// Deserialization helper for load — owns IdMap.
#[derive(Deserialize)]
struct OwnedPersistenceMeta {
    id_map: IdMap,
    dimensions: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── IdMap tests (2.3) ──

    #[test]
    fn test_insert_and_lookup() {
        let mut map = IdMap::new();
        let key = map.insert("doc1");
        assert_eq!(map.get_key("doc1"), Some(key));
        assert_eq!(map.get_doc(key), Some("doc1"));
        assert_eq!(map.len(), 1);
    }

    #[test]
    fn test_remove() {
        let mut map = IdMap::new();
        let key = map.insert("doc1");
        let removed = map.remove_by_doc("doc1");
        assert_eq!(removed, Some(key));
        assert_eq!(map.get_key("doc1"), None);
        assert_eq!(map.get_doc(key), None);
        assert!(map.is_empty());

        // remove_by_key path
        let mut map2 = IdMap::new();
        let key2 = map2.insert("doc2");
        let removed_doc = map2.remove_by_key(key2);
        assert_eq!(removed_doc.as_deref(), Some("doc2"));
        assert!(map2.is_empty());
    }

    #[test]
    fn test_next_key_monotonic() {
        let mut map = IdMap::new();
        let k1 = map.insert("a");
        let k2 = map.insert("b");
        let k3 = map.insert("c");
        assert!(k1 < k2);
        assert!(k2 < k3);
    }

    #[test]
    fn test_roundtrip_serde() {
        let mut map = IdMap::new();
        map.insert("alpha");
        map.insert("beta");

        let json = serde_json::to_string(&map).unwrap();
        let restored: IdMap = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.get_key("alpha"), map.get_key("alpha"));
        assert_eq!(restored.get_key("beta"), map.get_key("beta"));
        assert_eq!(restored.len(), 2);
    }

    #[test]
    fn test_get_or_reuse_key() {
        let mut map = IdMap::new();
        let k1 = map.insert("doc1");
        // Inserting the same doc_id should return the existing key
        let k2 = map.insert("doc1");
        assert_eq!(k1, k2);
        assert_eq!(map.len(), 1);
    }

    // ── VectorIndex tests (2.6) ──

    fn cos_metric() -> MetricKind {
        MetricKind::Cos
    }

    #[test]
    fn test_new_creates_empty_index() {
        let idx = VectorIndex::new(3, cos_metric()).unwrap();
        assert!(idx.is_empty());
        assert_eq!(idx.len(), 0);
        assert_eq!(idx.dimensions(), 3);
    }

    #[test]
    fn test_add_and_search() {
        let mut idx = VectorIndex::new(3, cos_metric()).unwrap();
        idx.add("doc1", &[1.0, 0.0, 0.0]).unwrap();
        idx.add("doc2", &[0.0, 1.0, 0.0]).unwrap();
        assert_eq!(idx.len(), 2);

        let results = idx.search(&[1.0, 0.0, 0.0], 1).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].doc_id, "doc1");
    }

    #[test]
    fn test_add_duplicate_doc_replaces() {
        let mut idx = VectorIndex::new(3, cos_metric()).unwrap();
        let key1 = {
            idx.add("doc1", &[1.0, 0.0, 0.0]).unwrap();
            idx.id_map.get_key("doc1").unwrap()
        };
        // Replace with different vector, key should be reused
        idx.add("doc1", &[0.0, 1.0, 0.0]).unwrap();
        let key2 = idx.id_map.get_key("doc1").unwrap();
        assert_eq!(key1, key2, "key should be reused on replace");
        assert_eq!(idx.len(), 1);

        // Search should find updated vector
        let results = idx.search(&[0.0, 1.0, 0.0], 1).unwrap();
        assert_eq!(results[0].doc_id, "doc1");
    }

    #[test]
    fn test_remove_doc() {
        let mut idx = VectorIndex::new(3, cos_metric()).unwrap();
        idx.add("doc1", &[1.0, 0.0, 0.0]).unwrap();
        idx.remove("doc1").unwrap();
        assert!(idx.is_empty());
    }

    #[test]
    fn test_remove_nonexistent_returns_error() {
        let mut idx = VectorIndex::new(3, cos_metric()).unwrap();
        let err = idx.remove("ghost").unwrap_err();
        match err {
            VectorError::DocumentNotFound { doc_id } => assert_eq!(doc_id, "ghost"),
            other => panic!("expected DocumentNotFound, got: {other}"),
        }
    }

    #[test]
    fn test_search_empty_index() {
        let idx = VectorIndex::new(3, cos_metric()).unwrap();
        let results = idx.search(&[1.0, 0.0, 0.0], 5).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_dimension_mismatch_on_add() {
        let mut idx = VectorIndex::new(3, cos_metric()).unwrap();
        let err = idx.add("doc1", &[1.0, 0.0]).unwrap_err();
        match err {
            VectorError::DimensionMismatch { expected, got } => {
                assert_eq!(expected, 3);
                assert_eq!(got, 2);
            }
            other => panic!("expected DimensionMismatch, got: {other}"),
        }
    }

    #[test]
    fn test_dimension_mismatch_on_search() {
        let idx = VectorIndex::new(3, cos_metric()).unwrap();
        let err = idx.search(&[1.0], 1).unwrap_err();
        match err {
            VectorError::DimensionMismatch { expected, got } => {
                assert_eq!(expected, 3);
                assert_eq!(got, 1);
            }
            other => panic!("expected DimensionMismatch, got: {other}"),
        }
    }

    // ── Persistence tests (2.16) ──

    #[test]
    fn test_save_and_load_roundtrip() {
        let tmp = tempfile::TempDir::new().unwrap();
        let dir = tmp.path().join("vec_idx");

        let mut idx = VectorIndex::new(3, cos_metric()).unwrap();
        idx.add("doc1", &[1.0, 0.0, 0.0]).unwrap();
        idx.add("doc2", &[0.0, 1.0, 0.0]).unwrap();
        idx.save(&dir).unwrap();

        let loaded = VectorIndex::load(&dir, cos_metric()).unwrap();
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded.dimensions(), 3);

        let results = loaded.search(&[1.0, 0.0, 0.0], 1).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].doc_id, "doc1");
    }

    #[test]
    fn test_load_nonexistent_path_returns_error() {
        let result = VectorIndex::load(Path::new("/nonexistent/path"), cos_metric());
        assert!(result.is_err());
    }

    #[test]
    fn test_save_load_empty_index() {
        let tmp = tempfile::TempDir::new().unwrap();
        let dir = tmp.path().join("empty_idx");

        let idx = VectorIndex::new(4, cos_metric()).unwrap();
        idx.save(&dir).unwrap();

        let loaded = VectorIndex::load(&dir, cos_metric()).unwrap();
        assert!(loaded.is_empty());
        assert_eq!(loaded.dimensions(), 4);
    }
}
