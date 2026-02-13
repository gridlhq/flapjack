use crate::error::{FlapjackError, Result};
use std::env;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

#[derive(Clone)]
pub struct MemoryBudgetConfig {
    pub max_buffer_mb: usize,
    pub max_concurrent_writers: usize,
    pub max_doc_mb: usize,
}

impl Default for MemoryBudgetConfig {
    fn default() -> Self {
        MemoryBudgetConfig {
            max_buffer_mb: 31,
            max_concurrent_writers: 40,
            max_doc_mb: 3,
        }
    }
}

impl MemoryBudgetConfig {
    pub fn from_env() -> Self {
        MemoryBudgetConfig {
            max_buffer_mb: env::var("FLAPJACK_MAX_BUFFER_MB")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(31),
            max_concurrent_writers: env::var("FLAPJACK_MAX_CONCURRENT_WRITERS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(40),
            max_doc_mb: env::var("FLAPJACK_MAX_DOC_MB")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(3),
        }
    }

    pub fn to_bytes(&self) -> (usize, usize, usize) {
        (
            self.max_buffer_mb * 1024 * 1024,
            self.max_concurrent_writers,
            self.max_doc_mb * 1024 * 1024,
        )
    }
}

pub struct MemoryBudget {
    max_buffer_size_bytes: usize,
    max_concurrent_writers: usize,
    max_document_size_bytes: usize,
    active_writers: Arc<AtomicUsize>,
}

impl MemoryBudget {
    pub fn new(config: MemoryBudgetConfig) -> Self {
        let (max_buffer, max_writers, max_doc) = config.to_bytes();
        MemoryBudget {
            max_buffer_size_bytes: max_buffer,
            max_concurrent_writers: max_writers,
            max_document_size_bytes: max_doc,
            active_writers: Arc::new(AtomicUsize::new(0)),
        }
    }

    pub fn acquire_writer(&self) -> Result<WriterGuard> {
        let current = self.active_writers.fetch_add(1, Ordering::SeqCst);

        if current >= self.max_concurrent_writers {
            self.active_writers.fetch_sub(1, Ordering::SeqCst);
            return Err(FlapjackError::TooManyConcurrentWrites {
                current: current + 1,
                max: self.max_concurrent_writers,
            });
        }

        Ok(WriterGuard {
            active_writers: Arc::clone(&self.active_writers),
        })
    }

    pub fn validate_buffer_size(&self, requested_size: usize) -> Result<usize> {
        if requested_size > self.max_buffer_size_bytes {
            return Err(FlapjackError::BufferSizeExceeded {
                requested: requested_size,
                max: self.max_buffer_size_bytes,
            });
        }
        Ok(requested_size)
    }

    pub fn validate_document_size(&self, doc_size_bytes: usize) -> Result<()> {
        if doc_size_bytes > self.max_document_size_bytes {
            return Err(FlapjackError::DocumentTooLarge {
                size: doc_size_bytes,
                max: self.max_document_size_bytes,
            });
        }
        Ok(())
    }

    pub fn active_writers(&self) -> usize {
        self.active_writers.load(Ordering::SeqCst)
    }

    pub fn max_concurrent_writers(&self) -> usize {
        self.max_concurrent_writers
    }

    pub fn reset_for_test(&self) {
        self.active_writers.store(0, Ordering::SeqCst);
    }
}

impl Clone for MemoryBudget {
    fn clone(&self) -> Self {
        MemoryBudget {
            max_buffer_size_bytes: self.max_buffer_size_bytes,
            max_concurrent_writers: self.max_concurrent_writers,
            max_document_size_bytes: self.max_document_size_bytes,
            active_writers: Arc::clone(&self.active_writers),
        }
    }
}

pub struct WriterGuard {
    active_writers: Arc<AtomicUsize>,
}

impl Drop for WriterGuard {
    fn drop(&mut self) {
        self.active_writers.fetch_sub(1, Ordering::SeqCst);
    }
}
