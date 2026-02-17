use criterion::{black_box, criterion_group, criterion_main, Criterion};
use flapjack::{Document, FacetRequest, FieldValue, IndexManager};
use std::collections::HashMap;
use tempfile::TempDir;

fn setup_tenant(manager: &IndexManager, tenant_id: &str, num_docs: usize) {
    manager.create_tenant(tenant_id).unwrap();

    let mut docs = Vec::new();
    for i in 0..num_docs {
        let mut doc = Document {
            id: format!("doc_{}", i),
            fields: HashMap::new(),
        };
        doc.fields.insert(
            "title".to_string(),
            FieldValue::Text(format!("Laptop Product {}", i)),
        );
        doc.fields.insert(
            "description".to_string(),
            FieldValue::Text(format!("Gaming laptop description {}", i)),
        );
        doc.fields.insert(
            "category".to_string(),
            FieldValue::Facet(format!("/cat{}", i % 100)),
        );
        doc.fields.insert(
            "price".to_string(),
            FieldValue::Integer((100 + i * 5) as i64),
        );
        docs.push(doc);
    }

    manager.add_documents(tenant_id, docs).unwrap();
}

fn query_p99_budget(c: &mut Criterion) {
    let temp = TempDir::new().unwrap();
    let manager = IndexManager::new(temp.path());
    setup_tenant(&manager, "bench", 10_000);

    let mut group = c.benchmark_group("p99_budgets");
    group.sample_size(500);
    group.measurement_time(std::time::Duration::from_secs(30));

    group.bench_function("facet_p99_under_50ms", |b| {
        b.iter(|| {
            let facet = FacetRequest {
                field: "category".to_string(),
                path: "/cat".to_string(),
            };
            black_box(manager.search_with_facets(
                "bench",
                "laptop",
                None,
                None,
                10,
                0,
                Some(&[facet]),
            ))
        })
    });

    group.bench_function("text_search_p99_under_50ms", |b| {
        b.iter(|| black_box(manager.search("bench", "laptop gaming", None, None, 10)))
    });

    group.bench_function("full_stack_p99_under_50ms", |b| {
        b.iter(|| {
            let filter = flapjack::Filter::Range {
                field: "price".to_string(),
                min: 200.0,
                max: 800.0,
            };
            let sort = flapjack::Sort::ByField {
                field: "price".to_string(),
                order: flapjack::SortOrder::Asc,
            };
            let facet = FacetRequest {
                field: "category".to_string(),
                path: "/cat".to_string(),
            };
            black_box(manager.search_with_facets(
                "bench",
                "laptop",
                Some(&filter),
                Some(&sort),
                10,
                0,
                Some(&[facet]),
            ))
        })
    });

    group.finish();
}

criterion_group!(regression_guards, query_p99_budget);
criterion_main!(regression_guards);
