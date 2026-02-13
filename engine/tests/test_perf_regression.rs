/// Fast latency regression guards.
///
/// These tests assert that core search operations stay under hard P99 latency
/// ceilings on a 1K-doc corpus.  They run in ~1 s total and are safe to
/// include in `cargo smoke` or CI.
///
/// Run with:
///   cargo test --release -p flapjack --test test_perf_regression
///
/// If a test fails it means a code change introduced a latency regression.
/// Investigate before merging.
use flapjack::{Document, FacetRequest, FieldValue, Filter, IndexManager, Sort, SortOrder};
use std::collections::HashMap;
use tempfile::TempDir;

// ── Thresholds (P99, microseconds) ─────────────────────────────────────
// These are intentionally generous to avoid flaky failures on slow CI
// runners.  Tighten them over time as the engine gets faster.
const P99_TEXT_SEARCH_US: u64 = 5_000; // 5 ms
const P99_MULTI_WORD_US: u64 = 10_000; // 10 ms
const P99_LONG_QUERY_US: u64 = 25_000; // 25 ms
const P99_FILTER_US: u64 = 10_000; // 10 ms
const P99_SORT_US: u64 = 10_000; // 10 ms
const P99_FACET_US: u64 = 30_000; // 30 ms
const P99_FULL_STACK_US: u64 = 40_000; // 40 ms
const P99_SHORT_QUERY_US: u64 = 15_000; // 15 ms — 1-2 char queries trigger term expansion
const P99_TYPEAHEAD_TOTAL_US: u64 = 60_000; // 60 ms — 6-keystroke typeahead sequence total

// ── Helpers ─────────────────────────────────────────────────────────────
fn build_corpus(manager: &IndexManager) {
    manager.create_tenant("regr").unwrap();
    let brands = [
        "Samsung", "Apple", "HP", "Dell", "Sony", "LG", "Lenovo", "Asus",
    ];
    let adjectives = ["premium", "budget", "gaming", "professional", "compact"];
    let mut docs = Vec::with_capacity(1000);
    for i in 0..1000 {
        let mut fields = HashMap::new();
        fields.insert(
            "name".into(),
            FieldValue::Text(format!(
                "{} {} laptop model-{}",
                brands[i % brands.len()],
                adjectives[i % adjectives.len()],
                i
            )),
        );
        fields.insert(
            "description".into(),
            FieldValue::Text(format!(
                "High quality {} electronics device with display screen {}",
                brands[i % brands.len()],
                i
            )),
        );
        fields.insert(
            "brand".into(),
            FieldValue::Text(brands[i % brands.len()].into()),
        );
        fields.insert(
            "category".into(),
            FieldValue::Facet(format!("/electronics/cat{}", i % 20)),
        );
        fields.insert("price".into(), FieldValue::Integer(100 + (i * 7) as i64));
        docs.push(Document {
            id: format!("d{}", i),
            fields,
        });
    }
    manager.add_documents("regr", docs).unwrap();
    // Brief pause for async indexing to finish
    std::thread::sleep(std::time::Duration::from_millis(500));
}

/// Run `f` repeatedly and return (avg_us, p99_us).
fn bench(iterations: usize, f: impl Fn()) -> (u64, u64) {
    // warmup
    for _ in 0..5 {
        f();
    }
    let mut times: Vec<u64> = Vec::with_capacity(iterations);
    for _ in 0..iterations {
        let t = std::time::Instant::now();
        f();
        times.push(t.elapsed().as_micros() as u64);
    }
    times.sort_unstable();
    let avg = times.iter().sum::<u64>() / times.len() as u64;
    let p99 = times[(times.len() as f64 * 0.99) as usize];
    (avg, p99)
}

// ── Tests ───────────────────────────────────────────────────────────────

fn with_manager(f: impl FnOnce(&IndexManager)) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let _guard = rt.enter();
    let tmp = TempDir::new().unwrap();
    let mgr = IndexManager::new(tmp.path());
    build_corpus(&mgr);
    f(&mgr);
}

#[test]
fn regression_text_search() {
    with_manager(|m| {
        let (avg, p99) = bench(200, || {
            let _ = m.search("regr", "samsung", None, None, 20);
        });
        eprintln!("  text_search:  avg={avg}us  p99={p99}us  (limit {P99_TEXT_SEARCH_US}us)");
        assert!(
            p99 < P99_TEXT_SEARCH_US,
            "text_search P99 regression: {p99}us > {P99_TEXT_SEARCH_US}us"
        );
    });
}

#[test]
fn regression_multi_word() {
    with_manager(|m| {
        let (avg, p99) = bench(200, || {
            let _ = m.search("regr", "samsung laptop", None, None, 20);
        });
        eprintln!("  multi_word:   avg={avg}us  p99={p99}us  (limit {P99_MULTI_WORD_US}us)");
        assert!(
            p99 < P99_MULTI_WORD_US,
            "multi_word P99 regression: {p99}us > {P99_MULTI_WORD_US}us"
        );
    });
}

#[test]
fn regression_long_query() {
    with_manager(|m| {
        let (avg, p99) = bench(200, || {
            let _ = m.search(
                "regr",
                "samsung premium laptop display screen",
                None,
                None,
                20,
            );
        });
        eprintln!("  long_query:   avg={avg}us  p99={p99}us  (limit {P99_LONG_QUERY_US}us)");
        assert!(
            p99 < P99_LONG_QUERY_US,
            "long_query P99 regression: {p99}us > {P99_LONG_QUERY_US}us"
        );
    });
}

#[test]
fn regression_filter() {
    with_manager(|m| {
        let filter = Filter::Range {
            field: "price".into(),
            min: 200.0,
            max: 800.0,
        };
        let (avg, p99) = bench(200, || {
            let _ = m.search("regr", "laptop", Some(&filter), None, 20);
        });
        eprintln!("  filter:       avg={avg}us  p99={p99}us  (limit {P99_FILTER_US}us)");
        assert!(
            p99 < P99_FILTER_US,
            "filter P99 regression: {p99}us > {P99_FILTER_US}us"
        );
    });
}

#[test]
fn regression_sort() {
    with_manager(|m| {
        let sort = Sort::ByField {
            field: "price".into(),
            order: SortOrder::Asc,
        };
        let (avg, p99) = bench(200, || {
            let _ = m.search("regr", "laptop", None, Some(&sort), 20);
        });
        eprintln!("  sort:         avg={avg}us  p99={p99}us  (limit {P99_SORT_US}us)");
        assert!(
            p99 < P99_SORT_US,
            "sort P99 regression: {p99}us > {P99_SORT_US}us"
        );
    });
}

#[test]
fn regression_facets() {
    with_manager(|m| {
        let facet = FacetRequest {
            field: "category".into(),
            path: "/electronics".into(),
        };
        let (avg, p99) = bench(200, || {
            let _ = m.search_with_facets(
                "regr",
                "laptop",
                None,
                None,
                20,
                0,
                Some(std::slice::from_ref(&facet)),
            );
        });
        eprintln!("  facets:       avg={avg}us  p99={p99}us  (limit {P99_FACET_US}us)");
        assert!(
            p99 < P99_FACET_US,
            "facets P99 regression: {p99}us > {P99_FACET_US}us"
        );
    });
}

#[test]
fn regression_full_stack() {
    with_manager(|m| {
        let filter = Filter::Range {
            field: "price".into(),
            min: 200.0,
            max: 800.0,
        };
        let sort = Sort::ByField {
            field: "price".into(),
            order: SortOrder::Asc,
        };
        let facet = FacetRequest {
            field: "category".into(),
            path: "/electronics".into(),
        };
        let (avg, p99) = bench(200, || {
            let _ = m.search_with_facets(
                "regr",
                "samsung laptop",
                Some(&filter),
                Some(&sort),
                20,
                0,
                Some(std::slice::from_ref(&facet)),
            );
        });
        eprintln!("  full_stack:   avg={avg}us  p99={p99}us  (limit {P99_FULL_STACK_US}us)");
        assert!(
            p99 < P99_FULL_STACK_US,
            "full_stack P99 regression: {p99}us > {P99_FULL_STACK_US}us"
        );
    });
}

/// Short/prefix queries (1-2 chars) trigger term expansion in the query
/// executor. This is the most common hot path for search-as-you-type UIs
/// and was identified as a latency spike source on remote instances.
#[test]
fn regression_short_query() {
    with_manager(|m| {
        // 1-char query: expands to all terms starting with "s" (samsung, sony, screen...)
        let (avg1, p99_1) = bench(200, || {
            let _ = m.search("regr", "s", None, None, 20);
        });
        eprintln!("  short_1char:  avg={avg1}us  p99={p99_1}us  (limit {P99_SHORT_QUERY_US}us)");
        assert!(
            p99_1 < P99_SHORT_QUERY_US,
            "short_query(1char) P99 regression: {p99_1}us > {P99_SHORT_QUERY_US}us"
        );

        // 2-char query: also triggers expansion but with narrower prefix
        let (avg2, p99_2) = bench(200, || {
            let _ = m.search("regr", "sa", None, None, 20);
        });
        eprintln!("  short_2char:  avg={avg2}us  p99={p99_2}us  (limit {P99_SHORT_QUERY_US}us)");
        assert!(
            p99_2 < P99_SHORT_QUERY_US,
            "short_query(2char) P99 regression: {p99_2}us > {P99_SHORT_QUERY_US}us"
        );
    });
}

/// Simulates a search-as-you-type sequence: "s" → "sa" → "sam" → "sams" →
/// "samsu" → "samsun".  This is the exact pattern the demo website exercises
/// and is where we saw fj-usw1 spike vs Algolia.
///
/// The test measures TOTAL time for the full 6-keystroke sequence (not each
/// individually) to catch cumulative regressions like facet cache thrashing.
#[test]
fn regression_typeahead_sequence() {
    with_manager(|m| {
        let facet = FacetRequest {
            field: "category".into(),
            path: "/electronics".into(),
        };
        let prefixes = ["s", "sa", "sam", "sams", "samsu", "samsun"];

        // warmup
        for _ in 0..3 {
            for q in &prefixes {
                let _ = m.search_with_facets(
                    "regr",
                    q,
                    None,
                    None,
                    20,
                    0,
                    Some(std::slice::from_ref(&facet)),
                );
            }
        }

        let mut times: Vec<u64> = Vec::with_capacity(50);
        for _ in 0..50 {
            let t = std::time::Instant::now();
            for q in &prefixes {
                let _ = m.search_with_facets(
                    "regr",
                    q,
                    None,
                    None,
                    20,
                    0,
                    Some(std::slice::from_ref(&facet)),
                );
            }
            times.push(t.elapsed().as_micros() as u64);
        }
        times.sort_unstable();
        let avg = times.iter().sum::<u64>() / times.len() as u64;
        let p99 = times[(times.len() as f64 * 0.99) as usize];
        let per_key = avg / prefixes.len() as u64;
        eprintln!(
            "  typeahead:    avg={avg}us  p99={p99}us  per_key={per_key}us  (limit {P99_TYPEAHEAD_TOTAL_US}us)"
        );
        assert!(
            p99 < P99_TYPEAHEAD_TOTAL_US,
            "typeahead P99 regression: {p99}us > {P99_TYPEAHEAD_TOTAL_US}us (6 keystrokes)"
        );
    });
}
