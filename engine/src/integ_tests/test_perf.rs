/// Performance tests moved inline from engine/tests/test_perf.rs.
///
/// Contains both:
///   - `test_search_latency`: manual perf measurement (run with --nocapture)
///   - `regression_*_slow`: P99 latency regression guards (gated behind --release)
///
/// Run quick measurement:
///   cargo test --release --lib -p flapjack test_search_latency -- --nocapture
///
/// Run regression guards:
///   cargo test --release --lib -p flapjack regression_ -- --nocapture
// ─── Quick latency measurement ──────────────────────────────────────────────
use crate::{Document, FacetRequest, FieldValue, Filter, IndexManager, Sort, SortOrder};
use std::collections::HashMap;
use tempfile::TempDir;

fn setup_quick(manager: &IndexManager, rt: &tokio::runtime::Runtime, num_docs: usize) {
    manager.create_tenant("bench").unwrap();
    let mut docs = Vec::new();
    for i in 0..num_docs {
        let mut doc = Document {
            id: format!("doc_{}", i),
            fields: HashMap::new(),
        };
        doc.fields.insert(
            "title".to_string(),
            FieldValue::Text(format!(
                "Laptop Gaming Product {} electronics samsung apple",
                i
            )),
        );
        doc.fields.insert(
            "description".to_string(),
            FieldValue::Text(format!(
                "High performance gaming laptop with premium display description {}",
                i
            )),
        );
        doc.fields.insert(
            "brand".to_string(),
            FieldValue::Text(["Samsung", "Apple", "HP", "Dell", "Sony"][i % 5].to_string()),
        );
        doc.fields.insert(
            "category".to_string(),
            FieldValue::Facet(format!("/cat{}", i % 50)),
        );
        doc.fields.insert(
            "price".to_string(),
            FieldValue::Integer((100 + i * 5) as i64),
        );
        docs.push(doc);
    }
    rt.block_on(manager.add_documents_sync("bench", docs))
        .unwrap();
}

fn measure(label: &str, iterations: usize, f: impl Fn()) {
    // Warmup
    for _ in 0..3 {
        f();
    }
    let mut times: Vec<f64> = Vec::with_capacity(iterations);
    for _ in 0..iterations {
        let start = std::time::Instant::now();
        f();
        times.push(start.elapsed().as_micros() as f64);
    }
    times.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let p50 = times[times.len() / 2];
    let p99 = times[(times.len() as f64 * 0.99) as usize];
    let avg = times.iter().sum::<f64>() / times.len() as f64;
    println!(
        "  {:<35} avg={:>8.0}us  p50={:>8.0}us  p99={:>8.0}us",
        label, avg, p50, p99
    );
}

#[test]
fn test_search_latency_slow() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let _guard = rt.enter();

    let temp = TempDir::new().unwrap();
    let manager = IndexManager::new(temp.path());

    println!("\n=== Setting up 10K docs ===");
    let start = std::time::Instant::now();
    setup_quick(&manager, &rt, 10_000);
    println!("  Setup took {:?}", start.elapsed());

    let iters = 100;
    println!("\n=== Search Latency ({} iterations each) ===", iters);

    measure("text_only (laptop)", iters, || {
        let _ = manager.search("bench", "laptop", None, None, 20);
    });
    measure("text_only (samsung)", iters, || {
        let _ = manager.search("bench", "samsung", None, None, 20);
    });
    measure("short_query (l)", iters, || {
        let _ = manager.search("bench", "l", None, None, 20);
    });
    measure("multi_word (laptop gaming)", iters, || {
        let _ = manager.search("bench", "laptop gaming", None, None, 20);
    });
    measure("long_query (samsung galaxy premium)", iters, || {
        let _ = manager.search("bench", "samsung galaxy premium display", None, None, 20);
    });
    measure("text + filter", iters, || {
        let filter = Filter::Range {
            field: "price".to_string(),
            min: 200.0,
            max: 800.0,
        };
        let _ = manager.search("bench", "laptop", Some(&filter), None, 20);
    });
    measure("text + sort", iters, || {
        let sort = Sort::ByField {
            field: "price".to_string(),
            order: SortOrder::Asc,
        };
        let _ = manager.search("bench", "laptop", None, Some(&sort), 20);
    });
    measure("text + facets", iters, || {
        let facet = FacetRequest {
            field: "category".to_string(),
            path: "/cat".to_string(),
        };
        let _ = manager.search_with_facets("bench", "laptop", None, None, 20, 0, Some(&[facet]));
    });
    measure("full_stack (text+filter+sort+facets)", iters, || {
        let filter = Filter::Range {
            field: "price".to_string(),
            min: 200.0,
            max: 800.0,
        };
        let sort = Sort::ByField {
            field: "price".to_string(),
            order: SortOrder::Asc,
        };
        let facet = FacetRequest {
            field: "category".to_string(),
            path: "/cat".to_string(),
        };
        let _ = manager.search_with_facets(
            "bench",
            "laptop",
            Some(&filter),
            Some(&sort),
            20,
            0,
            Some(&[facet]),
        );
    });
    measure("empty_query + facets", iters, || {
        let facet = FacetRequest {
            field: "category".to_string(),
            path: "/cat".to_string(),
        };
        let _ = manager.search_with_facets("bench", "", None, None, 20, 0, Some(&[facet]));
    });

    println!();
}

// ─── Regression guards (release-only) ───────────────────────────────────────

#[cfg(not(debug_assertions))]
const P99_TEXT_SEARCH_US: u64 = 5_000;
#[cfg(not(debug_assertions))]
const P99_MULTI_WORD_US: u64 = 10_000;
#[cfg(not(debug_assertions))]
const P99_LONG_QUERY_US: u64 = 25_000;
#[cfg(not(debug_assertions))]
const P99_FILTER_US: u64 = 10_000;
#[cfg(not(debug_assertions))]
const P99_SORT_US: u64 = 10_000;
#[cfg(not(debug_assertions))]
const P99_FACET_US: u64 = 30_000;
#[cfg(not(debug_assertions))]
const P99_FULL_STACK_US: u64 = 40_000;
#[cfg(not(debug_assertions))]
const P99_SHORT_QUERY_US: u64 = 15_000;
#[cfg(not(debug_assertions))]
const P99_TYPEAHEAD_TOTAL_US: u64 = 60_000;

#[cfg(not(debug_assertions))]
fn build_corpus(manager: &IndexManager, rt: &tokio::runtime::Runtime) {
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
    rt.block_on(manager.add_documents_sync("regr", docs))
        .unwrap();
}

#[cfg(not(debug_assertions))]
fn bench(iterations: usize, f: impl Fn()) -> (u64, u64) {
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

#[cfg(not(debug_assertions))]
fn with_manager(f: impl FnOnce(&IndexManager)) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let _guard = rt.enter();
    let tmp = TempDir::new().unwrap();
    let mgr = IndexManager::new(tmp.path());
    build_corpus(&mgr, &rt);
    f(&mgr);
}

#[cfg(not(debug_assertions))]
#[test]
fn regression_text_search_slow() {
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

#[cfg(not(debug_assertions))]
#[test]
fn regression_multi_word_slow() {
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

#[cfg(not(debug_assertions))]
#[test]
fn regression_long_query_slow() {
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

#[cfg(not(debug_assertions))]
#[test]
fn regression_filter_slow() {
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

#[cfg(not(debug_assertions))]
#[test]
fn regression_sort_slow() {
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

#[cfg(not(debug_assertions))]
#[test]
fn regression_facets_slow() {
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

#[cfg(not(debug_assertions))]
#[test]
fn regression_full_stack_slow() {
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

#[cfg(not(debug_assertions))]
#[test]
fn regression_short_query_slow() {
    with_manager(|m| {
        let (avg1, p99_1) = bench(200, || {
            let _ = m.search("regr", "s", None, None, 20);
        });
        eprintln!("  short_1char:  avg={avg1}us  p99={p99_1}us  (limit {P99_SHORT_QUERY_US}us)");
        assert!(
            p99_1 < P99_SHORT_QUERY_US,
            "short_query(1char) P99 regression: {p99_1}us > {P99_SHORT_QUERY_US}us"
        );

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

#[cfg(not(debug_assertions))]
#[test]
fn regression_typeahead_sequence_slow() {
    with_manager(|m| {
        let facet = FacetRequest {
            field: "category".into(),
            path: "/electronics".into(),
        };
        let prefixes = ["s", "sa", "sam", "sams", "samsu", "samsun"];

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
        eprintln!("  typeahead:    avg={avg}us  p99={p99}us  per_key={per_key}us  (limit {P99_TYPEAHEAD_TOTAL_US}us)");
        assert!(
            p99 < P99_TYPEAHEAD_TOTAL_US,
            "typeahead P99 regression: {p99}us > {P99_TYPEAHEAD_TOTAL_US}us (6 keystrokes)"
        );
    });
}
