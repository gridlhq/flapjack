use dashmap::DashSet;
use std::sync::Arc;

/// Tracks which indexes are currently paused (writes rejected with 503).
/// Thread-safe and lock-free via DashSet.
#[derive(Clone)]
pub struct PausedIndexes {
    inner: Arc<DashSet<String>>,
}

impl PausedIndexes {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(DashSet::new()),
        }
    }

    /// Mark an index as paused. Idempotent — pausing an already-paused index is a no-op.
    pub fn pause(&self, index_name: &str) {
        self.inner.insert(index_name.to_string());
    }

    /// Clear the paused flag for an index. Idempotent — resuming a non-paused index is a no-op.
    pub fn resume(&self, index_name: &str) {
        self.inner.remove(index_name);
    }

    /// Returns true if the given index is currently paused.
    pub fn is_paused(&self, index_name: &str) -> bool {
        self.inner.contains(index_name)
    }
}

impl Default for PausedIndexes {
    fn default() -> Self {
        Self::new()
    }
}

/// Guard function: returns `Err(FlapjackError::IndexPaused)` if the index is paused.
/// Call at the top of each write handler to reject writes during migration.
pub fn check_not_paused(
    paused: &PausedIndexes,
    index_name: &str,
) -> Result<(), flapjack::error::FlapjackError> {
    if paused.is_paused(index_name) {
        Err(flapjack::error::FlapjackError::IndexPaused(
            index_name.to_string(),
        ))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pause_registry_starts_empty() {
        let registry = PausedIndexes::new();
        assert!(
            !registry.is_paused("foo"),
            "new registry should have no paused indexes"
        );
        assert!(
            !registry.is_paused("bar"),
            "new registry should have no paused indexes"
        );
    }

    #[test]
    fn test_pause_marks_index_as_paused() {
        let registry = PausedIndexes::new();
        registry.pause("foo");
        assert!(
            registry.is_paused("foo"),
            "after pause('foo'), is_paused('foo') should be true"
        );
    }

    #[test]
    fn test_resume_clears_paused_flag() {
        let registry = PausedIndexes::new();
        registry.pause("foo");
        registry.resume("foo");
        assert!(
            !registry.is_paused("foo"),
            "after pause then resume, is_paused should be false"
        );
    }

    #[test]
    fn test_pause_is_per_index() {
        let registry = PausedIndexes::new();
        registry.pause("foo");
        assert!(registry.is_paused("foo"), "foo should be paused");
        assert!(!registry.is_paused("bar"), "bar should NOT be paused");
    }

    #[test]
    fn test_double_pause_is_idempotent() {
        let registry = PausedIndexes::new();
        registry.pause("foo");
        registry.pause("foo"); // second call should not panic or error
        assert!(
            registry.is_paused("foo"),
            "foo should still be paused after double pause"
        );
    }

    #[test]
    fn test_double_resume_is_idempotent() {
        let registry = PausedIndexes::new();
        // resume without ever pausing — should be a no-op
        registry.resume("foo");
        assert!(!registry.is_paused("foo"), "foo should not be paused");

        // pause, resume, resume — second resume should be a no-op
        registry.pause("bar");
        registry.resume("bar");
        registry.resume("bar");
        assert!(
            !registry.is_paused("bar"),
            "bar should not be paused after double resume"
        );
    }

    #[test]
    fn test_check_not_paused_ok_when_not_paused() {
        let registry = PausedIndexes::new();
        assert!(check_not_paused(&registry, "foo").is_ok());
    }

    #[test]
    fn test_check_not_paused_err_when_paused() {
        let registry = PausedIndexes::new();
        registry.pause("foo");
        let result = check_not_paused(&registry, "foo");
        assert!(result.is_err());
        match result.unwrap_err() {
            flapjack::error::FlapjackError::IndexPaused(name) => {
                assert_eq!(name, "foo");
            }
            other => panic!("expected IndexPaused, got {:?}", other),
        }
    }

    #[test]
    fn test_check_not_paused_per_index() {
        let registry = PausedIndexes::new();
        registry.pause("foo");
        assert!(
            check_not_paused(&registry, "foo").is_err(),
            "foo is paused, should be Err"
        );
        assert!(
            check_not_paused(&registry, "bar").is_ok(),
            "bar is not paused, should be Ok"
        );
    }

    #[test]
    fn test_pause_resume_concurrent_safe() {
        let registry = PausedIndexes::new();
        let mut handles = Vec::new();

        for i in 0..100 {
            let r = registry.clone();
            let handle = std::thread::spawn(move || {
                if i % 2 == 0 {
                    r.pause("shared");
                } else {
                    r.resume("shared");
                }
            });
            handles.push(handle);
        }

        for h in handles {
            h.join().expect("thread should not panic");
        }

        // No assertion on final state (it's racy), just verifying no panic/deadlock
        let _ = registry.is_paused("shared");
    }
}
