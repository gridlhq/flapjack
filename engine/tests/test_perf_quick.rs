/// Quick performance measurement test.
/// Run with: cargo test --release test_search_latency -- --nocapture
use flapjack::{Document, FacetRequest, FieldValue, Filter, IndexManager, Sort, SortOrder};
use std::collections::HashMap;
use tempfile::TempDir;

fn setup(manager: &IndexManager, num_docs: usize) {
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
    manager.add_documents("bench", docs).unwrap();
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
fn test_search_latency() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let _guard = rt.enter();

    let temp = TempDir::new().unwrap();
    let manager = IndexManager::new(temp.path());

    println!("\n=== Setting up 10K docs ===");
    let start = std::time::Instant::now();
    setup(&manager, 10_000);
    // Wait for async indexing
    std::thread::sleep(std::time::Duration::from_secs(2));
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
