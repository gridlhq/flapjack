pub mod config;
pub mod embedder;
pub mod index;
pub mod vectors_field;

use serde::{Deserialize, Serialize};
pub use usearch::ffi::MetricKind;

/// Errors from vector index operations.
#[derive(Debug, thiserror::Error)]
pub enum VectorError {
    #[error("dimension mismatch: expected {expected}, got {got}")]
    DimensionMismatch { expected: usize, got: usize },

    #[error("document not found: {doc_id}")]
    DocumentNotFound { doc_id: String },

    #[error("HNSW error: {0}")]
    HnswError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("serialization error: {0}")]
    SerializationError(String),

    #[error("invalid path: {0}")]
    InvalidPath(String),

    #[error("embedding error: {0}")]
    EmbeddingError(String),
}

/// A single result from a vector similarity search.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VectorSearchResult {
    pub doc_id: String,
    pub distance: f32,
}
