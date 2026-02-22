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

#[cfg(test)]
mod tests {
    use super::*;

    // ── MemoryBudgetConfig ───────────────────────────────────────────────

    #[test]
    fn default_config_values() {
        let cfg = MemoryBudgetConfig::default();
        assert_eq!(cfg.max_buffer_mb, 31);
        assert_eq!(cfg.max_concurrent_writers, 40);
        assert_eq!(cfg.max_doc_mb, 3);
    }

    #[test]
    fn to_bytes_converts_mb() {
        let cfg = MemoryBudgetConfig {
            max_buffer_mb: 10,
            max_concurrent_writers: 5,
            max_doc_mb: 2,
        };
        let (buf, writers, doc) = cfg.to_bytes();
        assert_eq!(buf, 10 * 1024 * 1024);
        assert_eq!(writers, 5);
        assert_eq!(doc, 2 * 1024 * 1024);
    }

    // ── validate_document_size ───────────────────────────────────────────

    #[test]
    fn validate_document_size_ok() {
        let budget = MemoryBudget::new(MemoryBudgetConfig::default());
        assert!(budget.validate_document_size(1024).is_ok());
    }

    #[test]
    fn validate_document_size_at_limit() {
        let budget = MemoryBudget::new(MemoryBudgetConfig {
            max_doc_mb: 1,
            ..Default::default()
        });
        // exactly at limit
        assert!(budget.validate_document_size(1024 * 1024).is_ok());
    }

    #[test]
    fn validate_document_size_exceeds() {
        let budget = MemoryBudget::new(MemoryBudgetConfig {
            max_doc_mb: 1,
            ..Default::default()
        });
        assert!(budget.validate_document_size(1024 * 1024 + 1).is_err());
    }

    // ── validate_buffer_size ─────────────────────────────────────────────

    #[test]
    fn validate_buffer_size_ok() {
        let budget = MemoryBudget::new(MemoryBudgetConfig::default());
        let result = budget.validate_buffer_size(1024);
        assert_eq!(result.unwrap(), 1024);
    }

    #[test]
    fn validate_buffer_size_exceeds() {
        let budget = MemoryBudget::new(MemoryBudgetConfig {
            max_buffer_mb: 1,
            ..Default::default()
        });
        assert!(budget.validate_buffer_size(2 * 1024 * 1024).is_err());
    }

    // ── acquire_writer / WriterGuard ─────────────────────────────────────

    #[test]
    fn acquire_writer_increments_count() {
        let budget = MemoryBudget::new(MemoryBudgetConfig {
            max_concurrent_writers: 10,
            ..Default::default()
        });
        assert_eq!(budget.active_writers(), 0);
        let _guard = budget.acquire_writer().unwrap();
        assert_eq!(budget.active_writers(), 1);
    }

    #[test]
    fn writer_guard_drop_decrements_count() {
        let budget = MemoryBudget::new(MemoryBudgetConfig {
            max_concurrent_writers: 10,
            ..Default::default()
        });
        {
            let _guard = budget.acquire_writer().unwrap();
            assert_eq!(budget.active_writers(), 1);
        }
        assert_eq!(budget.active_writers(), 0);
    }

    #[test]
    fn acquire_writer_fails_at_limit() {
        let budget = MemoryBudget::new(MemoryBudgetConfig {
            max_concurrent_writers: 2,
            ..Default::default()
        });
        let _g1 = budget.acquire_writer().unwrap();
        let _g2 = budget.acquire_writer().unwrap();
        assert!(budget.acquire_writer().is_err());
        assert_eq!(budget.active_writers(), 2);
    }

    #[test]
    fn acquire_writer_recovers_after_drop() {
        let budget = MemoryBudget::new(MemoryBudgetConfig {
            max_concurrent_writers: 1,
            ..Default::default()
        });
        {
            let _g = budget.acquire_writer().unwrap();
            assert!(budget.acquire_writer().is_err());
        }
        // After guard drops, should be able to acquire again
        let _g2 = budget.acquire_writer().unwrap();
        assert_eq!(budget.active_writers(), 1);
    }

    #[test]
    fn clone_shares_active_writers() {
        let budget = MemoryBudget::new(MemoryBudgetConfig {
            max_concurrent_writers: 10,
            ..Default::default()
        });
        let budget2 = budget.clone();
        let _guard = budget.acquire_writer().unwrap();
        assert_eq!(budget2.active_writers(), 1);
    }

    #[test]
    fn reset_for_test_clears_writers() {
        let budget = MemoryBudget::new(MemoryBudgetConfig {
            max_concurrent_writers: 10,
            ..Default::default()
        });
        // Leak a guard intentionally
        std::mem::forget(budget.acquire_writer().unwrap());
        assert_eq!(budget.active_writers(), 1);
        budget.reset_for_test();
        assert_eq!(budget.active_writers(), 0);
    }

    #[test]
    fn max_concurrent_writers_getter() {
        let budget = MemoryBudget::new(MemoryBudgetConfig {
            max_concurrent_writers: 42,
            ..Default::default()
        });
        assert_eq!(budget.max_concurrent_writers(), 42);
    }
}
