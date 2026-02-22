//! Integration tests moved inline to avoid nextest process-per-test overhead.
//!
//! These tests exercise library APIs only (no HTTP servers, no cross-crate type
//! sharing). Running them in-process via `cargo test --lib` takes <1s instead
//! of minutes under nextest.

#[cfg(feature = "analytics")]
mod test_analytics;
#[cfg(feature = "analytics")]
mod test_analytics_io;
mod test_facets;
mod test_library;
mod test_perf;
mod test_query;
mod test_ranking;
mod test_rules;
mod test_tokenizer;
