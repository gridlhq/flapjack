//! System memory awareness: heap stats, memory limits, and pressure levels.
//!
//! Requires the `memory-stats` feature flag. Without it, all methods return
//! safe defaults (zero bytes allocated, unlimited memory, Normal pressure).

use std::env;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::OnceLock;

/// Pressure levels for load shedding decisions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PressureLevel {
    /// Below high-watermark — all operations allowed.
    Normal,
    /// Above high-watermark — reject writes, allow reads.
    Elevated,
    /// Above critical threshold — reject everything except health checks.
    Critical,
}

impl std::fmt::Display for PressureLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PressureLevel::Normal => write!(f, "normal"),
            PressureLevel::Elevated => write!(f, "elevated"),
            PressureLevel::Critical => write!(f, "critical"),
        }
    }
}

/// Snapshot of memory metrics for the health endpoint.
#[derive(Debug, Clone)]
pub struct MemoryStats {
    pub heap_allocated_bytes: usize,
    pub system_limit_bytes: usize,
    pub limit_source: String,
    pub pressure_level: PressureLevel,
    pub allocator: &'static str,
    pub high_watermark_pct: usize,
    pub critical_pct: usize,
}

/// Observes heap usage and system memory limits.
pub struct MemoryObserver {
    high_watermark_pct: usize,
    critical_pct: usize,
    limit_bytes: usize,
    limit_source: String,
    /// Override pressure level for testing: 0=no override, 1=Normal, 2=Elevated, 3=Critical
    pressure_override: AtomicU8,
}

static GLOBAL_OBSERVER: OnceLock<MemoryObserver> = OnceLock::new();

impl Default for MemoryObserver {
    fn default() -> Self {
        Self::new()
    }
}

impl MemoryObserver {
    /// Create a new observer, reading configuration from environment variables.
    pub fn new() -> Self {
        let high_watermark_pct: usize = env::var("FLAPJACK_MEMORY_HIGH_WATERMARK")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(80);

        let critical_pct: usize = env::var("FLAPJACK_MEMORY_CRITICAL")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(90);

        let (limit_bytes, limit_source) = Self::detect_memory_limit();

        MemoryObserver {
            high_watermark_pct,
            critical_pct,
            limit_bytes,
            limit_source,
            pressure_override: AtomicU8::new(0),
        }
    }

    /// Get or initialize the global observer singleton.
    pub fn global() -> &'static MemoryObserver {
        GLOBAL_OBSERVER.get_or_init(MemoryObserver::new)
    }

    /// Current heap allocation in bytes.
    ///
    /// Uses jemalloc-ctl on non-MSVC platforms (when `memory-stats` feature is
    /// enabled), falls back to process RSS via sysinfo on MSVC or when the
    /// feature is disabled.
    pub fn heap_allocated_bytes(&self) -> usize {
        Self::read_heap_allocated()
    }

    /// Detected system memory limit in bytes and its source.
    pub fn system_memory_limit(&self) -> (usize, &str) {
        (self.limit_bytes, &self.limit_source)
    }

    /// Effective memory limit, checking env var override on each call.
    ///
    /// This allows `FLAPJACK_MEMORY_LIMIT_MB` to be changed at runtime
    /// (e.g. in tests or via live reconfiguration).
    fn effective_limit(&self) -> (usize, &str) {
        if let Ok(mb) = env::var("FLAPJACK_MEMORY_LIMIT_MB") {
            if let Ok(val) = mb.parse::<usize>() {
                return (val * 1024 * 1024, "env");
            }
        }
        (self.limit_bytes, &self.limit_source)
    }

    /// Current memory pressure level based on heap usage vs limit.
    pub fn pressure_level(&self) -> PressureLevel {
        // Check test override first
        match self.pressure_override.load(Ordering::Relaxed) {
            1 => return PressureLevel::Normal,
            2 => return PressureLevel::Elevated,
            3 => return PressureLevel::Critical,
            _ => {}
        }

        let (limit, _) = self.effective_limit();
        if limit == 0 {
            return PressureLevel::Normal;
        }
        let allocated = self.heap_allocated_bytes();
        let usage_pct = (allocated * 100) / limit;

        if usage_pct >= self.critical_pct {
            PressureLevel::Critical
        } else if usage_pct >= self.high_watermark_pct {
            PressureLevel::Elevated
        } else {
            PressureLevel::Normal
        }
    }

    /// Override the pressure level (for testing). Pass `None` to clear.
    pub fn set_pressure_override(&self, level: Option<PressureLevel>) {
        let val = match level {
            None => 0,
            Some(PressureLevel::Normal) => 1,
            Some(PressureLevel::Elevated) => 2,
            Some(PressureLevel::Critical) => 3,
        };
        self.pressure_override.store(val, Ordering::Relaxed);
    }

    /// All memory metrics in one struct (for health endpoint).
    pub fn stats(&self) -> MemoryStats {
        let (limit, source) = self.effective_limit();
        MemoryStats {
            heap_allocated_bytes: self.heap_allocated_bytes(),
            system_limit_bytes: limit,
            limit_source: source.to_string(),
            pressure_level: self.pressure_level(),
            allocator: Self::allocator_name(),
            high_watermark_pct: self.high_watermark_pct,
            critical_pct: self.critical_pct,
        }
    }

    /// Name of the active allocator.
    pub fn allocator_name() -> &'static str {
        #[cfg(all(feature = "memory-stats", not(target_env = "msvc")))]
        {
            "jemalloc"
        }
        #[cfg(any(not(feature = "memory-stats"), target_env = "msvc"))]
        {
            "system"
        }
    }

    // -- private helpers --

    fn read_heap_allocated() -> usize {
        #[cfg(all(feature = "memory-stats", not(target_env = "msvc")))]
        {
            // jemalloc epoch must be advanced before reading stats
            tikv_jemalloc_ctl::epoch::advance().ok();
            tikv_jemalloc_ctl::stats::allocated::read().unwrap_or(0)
        }
        #[cfg(any(not(feature = "memory-stats"), target_env = "msvc"))]
        {
            // Fallback: process RSS via sysinfo
            #[cfg(feature = "memory-stats")]
            {
                use sysinfo::{Pid, System};
                let pid = Pid::from(std::process::id() as usize);
                let mut sys = System::new();
                sys.refresh_process(pid);
                sys.process(pid).map(|p| p.memory() as usize).unwrap_or(0)
            }
            #[cfg(not(feature = "memory-stats"))]
            {
                0
            }
        }
    }

    fn detect_memory_limit() -> (usize, String) {
        // Priority 1: explicit env var
        if let Ok(mb) = env::var("FLAPJACK_MEMORY_LIMIT_MB") {
            if let Ok(val) = mb.parse::<usize>() {
                return (val * 1024 * 1024, "env".to_string());
            }
        }

        // Priority 2: cgroup v2 memory limit (Linux only)
        #[cfg(target_os = "linux")]
        {
            if let Ok(contents) = std::fs::read_to_string("/sys/fs/cgroup/memory.max") {
                let trimmed = contents.trim();
                if trimmed != "max" {
                    if let Ok(bytes) = trimmed.parse::<usize>() {
                        return (bytes, "cgroup".to_string());
                    }
                }
            }
        }

        // Priority 3: total system memory via sysinfo
        #[cfg(feature = "memory-stats")]
        {
            use sysinfo::System;
            let sys = System::new_all();
            let total = sys.total_memory() as usize;
            if total > 0 {
                return (total, "sysinfo".to_string());
            }
        }

        (0, "unknown".to_string())
    }
}
