use std::sync::atomic::{AtomicU32, AtomicU64, AtomicU8, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// Circuit breaker states.
const STATE_CLOSED: u8 = 0;
const STATE_OPEN: u8 = 1;
const STATE_HALF_OPEN: u8 = 2;

/// Per-peer circuit breaker that prevents sending requests to known-dead peers.
///
/// State machine:
///   Closed  --[failure_threshold consecutive failures]--> Open
///   Open    --[recovery_timeout expires]----------------> HalfOpen
///   HalfOpen --[success]-------------------------------> Closed
///   HalfOpen --[failure]-------------------------------> Open
pub struct CircuitBreaker {
    state: AtomicU8,
    consecutive_failures: AtomicU32,
    failure_threshold: u32,
    /// Seconds to wait in Open state before allowing a probe (HalfOpen)
    recovery_timeout_secs: u64,
    /// Unix timestamp (seconds) when the circuit last tripped to Open
    last_tripped: AtomicU64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    Closed,
    Open,
    HalfOpen,
}

impl CircuitBreaker {
    pub fn new(failure_threshold: u32, recovery_timeout_secs: u64) -> Self {
        Self {
            state: AtomicU8::new(STATE_CLOSED),
            consecutive_failures: AtomicU32::new(0),
            failure_threshold,
            recovery_timeout_secs,
            last_tripped: AtomicU64::new(0),
        }
    }

    fn now_secs() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }

    /// Returns the current circuit state, handling the Open → HalfOpen timeout transition.
    pub fn state(&self) -> CircuitState {
        let raw = self.state.load(Ordering::Acquire);
        if raw == STATE_OPEN {
            let tripped_at = self.last_tripped.load(Ordering::Acquire);
            let elapsed = Self::now_secs().saturating_sub(tripped_at);
            if elapsed >= self.recovery_timeout_secs {
                // Transition Open → HalfOpen (CAS to avoid races)
                if self
                    .state
                    .compare_exchange(
                        STATE_OPEN,
                        STATE_HALF_OPEN,
                        Ordering::AcqRel,
                        Ordering::Acquire,
                    )
                    .is_ok()
                {
                    return CircuitState::HalfOpen;
                }
                // Another thread already transitioned — re-read
                return match self.state.load(Ordering::Acquire) {
                    STATE_CLOSED => CircuitState::Closed,
                    STATE_OPEN => CircuitState::Open,
                    _ => CircuitState::HalfOpen,
                };
            }
            CircuitState::Open
        } else if raw == STATE_HALF_OPEN {
            CircuitState::HalfOpen
        } else {
            CircuitState::Closed
        }
    }

    /// Should we send a request to this peer right now?
    pub fn allow_request(&self) -> bool {
        match self.state() {
            CircuitState::Closed => true,
            CircuitState::HalfOpen => true, // allow the probe
            CircuitState::Open => false,
        }
    }

    /// Record a successful request — reset failures and close the circuit.
    pub fn record_success(&self) {
        self.consecutive_failures.store(0, Ordering::Release);
        self.state.store(STATE_CLOSED, Ordering::Release);
    }

    /// Record a failed request — increment failures, potentially trip the circuit.
    pub fn record_failure(&self) {
        let prev = self.consecutive_failures.fetch_add(1, Ordering::AcqRel) + 1;

        let current_state = self.state.load(Ordering::Acquire);

        if current_state == STATE_HALF_OPEN {
            // Probe failed — go back to Open
            self.trip();
        } else if current_state == STATE_CLOSED && prev >= self.failure_threshold {
            // Threshold exceeded in Closed state — trip
            self.trip();
        }
    }

    fn trip(&self) {
        self.state.store(STATE_OPEN, Ordering::Release);
        self.last_tripped.store(Self::now_secs(), Ordering::Release);
    }

    pub fn consecutive_failures(&self) -> u32 {
        self.consecutive_failures.load(Ordering::Acquire)
    }

    /// Force transition to HalfOpen (for testing and active health probes).
    #[cfg(test)]
    pub fn force_half_open(&self) {
        self.state.store(STATE_HALF_OPEN, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_closed() {
        let cb = CircuitBreaker::new(3, 30);
        assert_eq!(cb.state(), CircuitState::Closed);
        assert!(cb.allow_request());
    }

    #[test]
    fn stays_closed_on_success() {
        let cb = CircuitBreaker::new(3, 30);
        cb.record_success();
        cb.record_success();
        assert_eq!(cb.state(), CircuitState::Closed);
        assert!(cb.allow_request());
    }

    #[test]
    fn stays_closed_below_threshold() {
        let cb = CircuitBreaker::new(3, 30);
        cb.record_failure();
        cb.record_failure();
        // 2 failures, threshold is 3 — still Closed
        assert_eq!(cb.state(), CircuitState::Closed);
        assert!(cb.allow_request());
        assert_eq!(cb.consecutive_failures(), 2);
    }

    #[test]
    fn trips_to_open_at_threshold() {
        let cb = CircuitBreaker::new(3, 30);
        cb.record_failure();
        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);
        assert!(!cb.allow_request());
    }

    #[test]
    fn open_blocks_requests() {
        let cb = CircuitBreaker::new(1, 9999);
        cb.record_failure(); // trips immediately (threshold=1)
        assert_eq!(cb.state(), CircuitState::Open);
        assert!(!cb.allow_request());
    }

    #[test]
    fn open_transitions_to_half_open_after_timeout() {
        let cb = CircuitBreaker::new(1, 0); // 0-second timeout = immediate transition
        cb.record_failure();
        // With 0s timeout, state() immediately transitions Open → HalfOpen
        assert_eq!(cb.state(), CircuitState::HalfOpen);
        assert!(cb.allow_request());
    }

    #[test]
    fn half_open_success_closes_circuit() {
        let cb = CircuitBreaker::new(1, 999);
        cb.record_failure(); // Open (persists — large timeout)
        assert_eq!(cb.state(), CircuitState::Open);
        cb.force_half_open(); // simulate timeout expiry
        assert_eq!(cb.state(), CircuitState::HalfOpen);
        cb.record_success(); // should close
        assert_eq!(cb.state(), CircuitState::Closed);
        assert!(cb.allow_request());
        assert_eq!(cb.consecutive_failures(), 0);
    }

    #[test]
    fn half_open_failure_reopens_circuit() {
        let cb = CircuitBreaker::new(1, 999);
        cb.record_failure(); // Open
        assert_eq!(cb.state(), CircuitState::Open);
        cb.force_half_open(); // simulate timeout expiry
        cb.record_failure(); // probe failed → back to Open
        assert_eq!(cb.state(), CircuitState::Open);
        assert!(!cb.allow_request());
    }

    #[test]
    fn success_resets_failure_count() {
        let cb = CircuitBreaker::new(3, 30);
        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.consecutive_failures(), 2);
        cb.record_success();
        assert_eq!(cb.consecutive_failures(), 0);
        // One more failure shouldn't trip (count reset to 0, need 3 consecutive)
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Closed);
    }

    #[test]
    fn full_cycle_closed_open_halfopen_closed() {
        let cb = CircuitBreaker::new(2, 999);

        // Closed → Open
        assert_eq!(cb.state(), CircuitState::Closed);
        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);

        // Open → HalfOpen (simulated timeout)
        cb.force_half_open();
        assert_eq!(cb.state(), CircuitState::HalfOpen);

        // HalfOpen → Closed (success)
        cb.record_success();
        assert_eq!(cb.state(), CircuitState::Closed);
        assert_eq!(cb.consecutive_failures(), 0);
    }

    #[test]
    fn full_cycle_with_half_open_failure() {
        let cb = CircuitBreaker::new(2, 999);

        // Trip it
        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);

        // Transition to HalfOpen
        cb.force_half_open();

        // Probe fails → back to Open
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);

        // Transition to HalfOpen again
        cb.force_half_open();

        // Probe succeeds → Closed
        cb.record_success();
        assert_eq!(cb.state(), CircuitState::Closed);
    }

    #[test]
    fn threshold_of_one_trips_immediately() {
        let cb = CircuitBreaker::new(1, 30);
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);
        assert!(!cb.allow_request());
    }

    #[test]
    fn high_threshold_requires_many_failures() {
        let cb = CircuitBreaker::new(10, 30);
        for i in 0..9 {
            cb.record_failure();
            assert_eq!(
                cb.state(),
                CircuitState::Closed,
                "should be closed at failure {}",
                i + 1
            );
        }
        cb.record_failure(); // 10th
        assert_eq!(cb.state(), CircuitState::Open);
    }
}
