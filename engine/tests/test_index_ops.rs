use flapjack::types::Document;
use flapjack::IndexManager;
use serde_json::json;
use std::collections::HashMap;
use std::path::Path;
use tempfile::TempDir;

fn make_doc(id: &str, name: &str) -> Document {
    Document::from_json(&json!({"_id": id, "name": name})).unwrap()
}

fn make_docs(ids: &[&str]) -> Vec<Document> {
    ids.iter()
        .map(|id| Document::from_json(&json!({"_id": id, "name": format!("Item {}", id)})).unwrap())
        .collect()
}

fn dir_size(path: &Path) -> u64 {
    let mut total = 0u64;
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            let ft = match entry.file_type() {
                Ok(ft) => ft,
                Err(_) => continue,
            };
            if ft.is_file() {
                total += entry.metadata().map(|m| m.len()).unwrap_or(0);
            } else if ft.is_dir() {
                total += dir_size(&entry.path());
            }
        }
    }
    total
}

fn read_oplog_entries(oplog_dir: &Path) -> Vec<serde_json::Value> {
    let mut entries = Vec::new();
    if !oplog_dir.exists() {
        return entries;
    }
    for entry in std::fs::read_dir(oplog_dir).unwrap() {
        let entry = entry.unwrap();
        if entry.file_name().to_str().unwrap().ends_with(".jsonl") {
            let content = std::fs::read_to_string(entry.path()).unwrap();
            for line in content.lines() {
                if !line.trim().is_empty() {
                    entries.push(serde_json::from_str(line).unwrap());
                }
            }
        }
    }
    entries.sort_by_key(|e| e["seq"].as_u64().unwrap());
    entries
}

// ============================================================
// BATCH DELETE
// ============================================================

mod batch_delete {
    use super::*;

    #[tokio::test]
    async fn add_and_delete() {
        let tmp = TempDir::new().unwrap();
        let mgr = IndexManager::new(tmp.path());
        mgr.create_tenant("test").unwrap();

        let docs = vec![
            Document {
                id: "1".to_string(),
                fields: HashMap::from([(
                    "name".to_string(),
                    flapjack::types::FieldValue::Text("Product 1".to_string()),
                )]),
            },
            Document {
                id: "2".to_string(),
                fields: HashMap::from([(
                    "name".to_string(),
                    flapjack::types::FieldValue::Text("Product 2".to_string()),
                )]),
            },
            Document {
                id: "3".to_string(),
                fields: HashMap::from([(
                    "name".to_string(),
                    flapjack::types::FieldValue::Text("Product 3".to_string()),
                )]),
            },
        ];

        mgr.add_documents_sync("test", docs).await.unwrap();
        let results = mgr.search("test", "", None, None, 100).unwrap();
        assert_eq!(results.total, 3, "Should have 3 docs before delete");

        mgr.delete_documents_sync("test", vec!["1".to_string(), "3".to_string()])
            .await
            .unwrap();
        let results = mgr.search("test", "", None, None, 100).unwrap();
        assert_eq!(results.total, 1, "Should have 1 doc after deleting 2");
        assert_eq!(results.documents[0].document.id, "2");
    }

    #[tokio::test]
    async fn delete_nonexistent() {
        let tmp = TempDir::new().unwrap();
        let mgr = IndexManager::new(tmp.path());
        mgr.create_tenant("test").unwrap();

        mgr.delete_documents_sync("test", vec!["nonexistent".to_string()])
            .await
            .unwrap();
        let results = mgr.search("test", "", None, None, 100).unwrap();
        assert_eq!(results.total, 0);
    }

    #[tokio::test]
    async fn upsert_after_delete() {
        let tmp = TempDir::new().unwrap();
        let mgr = IndexManager::new(tmp.path());
        mgr.create_tenant("test").unwrap();

        mgr.add_documents_sync("test", vec![make_doc("1", "Original")])
            .await
            .unwrap();
        mgr.delete_documents_sync("test", vec!["1".to_string()])
            .await
            .unwrap();
        mgr.add_documents_sync("test", vec![make_doc("1", "Replacement")])
            .await
            .unwrap();

        let results = mgr.search("test", "", None, None, 100).unwrap();
        assert_eq!(results.total, 1);
        let name = match results.documents[0].document.fields.get("name").unwrap() {
            flapjack::types::FieldValue::Text(s) => s,
            _ => panic!("Expected text"),
        };
        assert_eq!(name, "Replacement");
    }
}

// ============================================================
// COMPACT INDEX
// ============================================================

mod compact_index {
    use super::*;

    fn make_large_doc(id: &str) -> Document {
        // Use random (incompressible) payload so document data dominates over
        // fixed per-segment metadata overhead, making size comparisons reliable.
        use rand::Rng;
        let payload: String = rand::thread_rng()
            .sample_iter(&rand::distributions::Alphanumeric)
            .take(10_000)
            .map(char::from)
            .collect();
        Document::from_json(&json!({"_id": id, "name": format!("Item {}", id), "body": payload}))
            .unwrap()
    }

    #[tokio::test]
    async fn compact_after_deletes_reclaims_space() {
        let tmp = TempDir::new().unwrap();
        let mgr = IndexManager::new(tmp.path());
        mgr.create_tenant("test").unwrap();

        // Add large docs in separate batches to create multiple segments
        let batch1: Vec<Document> = (1..=25).map(|i| make_large_doc(&i.to_string())).collect();
        let batch2: Vec<Document> = (26..=50).map(|i| make_large_doc(&i.to_string())).collect();
        mgr.add_documents_sync("test", batch1).await.unwrap();
        mgr.add_documents_sync("test", batch2).await.unwrap();

        let size_with_all_docs = dir_size(&tmp.path().join("test"));

        // Delete most docs (keep only doc "50")
        let to_delete: Vec<String> = (1..=49).map(|i| i.to_string()).collect();
        mgr.delete_documents_sync("test", to_delete).await.unwrap();

        // Compact to force merge + GC
        mgr.compact_index_sync("test").await.unwrap();

        let size_after_compact = dir_size(&tmp.path().join("test"));
        assert!(
            size_after_compact < size_with_all_docs,
            "compact should reclaim space: all_docs={} after_compact={}",
            size_with_all_docs,
            size_after_compact
        );

        // Verify remaining doc is still searchable
        let results = mgr.search("test", "", None, None, 100).unwrap();
        assert_eq!(results.total, 1);
        assert_eq!(results.documents[0].document.id, "50");
    }

    #[tokio::test]
    async fn compact_empty_index() {
        let tmp = TempDir::new().unwrap();
        let mgr = IndexManager::new(tmp.path());
        mgr.create_tenant("test").unwrap();

        // Should not panic on an empty index
        mgr.compact_index_sync("test").await.unwrap();

        let results = mgr.search("test", "", None, None, 100).unwrap();
        assert_eq!(results.total, 0);
    }
}

// ============================================================
// OPERATION INDEX (move/copy)
// ============================================================

mod operation_index {
    use super::*;

    #[tokio::test]
    async fn move_basic() {
        let tmp = TempDir::new().unwrap();
        let mgr = IndexManager::new(tmp.path());
        mgr.create_tenant("src").unwrap();
        mgr.add_documents_sync("src", make_docs(&["1", "2", "3"]))
            .await
            .unwrap();

        let results = mgr.search("src", "Item", None, None, 10).unwrap();
        assert_eq!(results.documents.len(), 3);

        mgr.move_index("src", "dst").await.unwrap();
        assert!(mgr.search("src", "Item", None, None, 10).is_err());

        let results = mgr.search("dst", "Item", None, None, 10).unwrap();
        assert_eq!(results.documents.len(), 3);
    }

    #[tokio::test]
    async fn move_overwrites_destination() {
        let tmp = TempDir::new().unwrap();
        let mgr = IndexManager::new(tmp.path());

        mgr.create_tenant("old").unwrap();
        mgr.add_documents_sync("old", make_docs(&["old1"]))
            .await
            .unwrap();

        mgr.create_tenant("new").unwrap();
        mgr.add_documents_sync("new", make_docs(&["new1", "new2"]))
            .await
            .unwrap();

        mgr.move_index("new", "old").await.unwrap();
        let results = mgr.search("old", "Item", None, None, 10).unwrap();
        assert_eq!(results.documents.len(), 2);
        assert!(mgr.search("new", "Item", None, None, 10).is_err());
    }

    #[tokio::test]
    async fn move_nonexistent_source_is_noop() {
        let tmp = TempDir::new().unwrap();
        let mgr = IndexManager::new(tmp.path());
        let task = mgr.move_index("ghost", "dst").await.unwrap();
        assert_eq!(task.status, flapjack::types::TaskStatus::Succeeded);
    }

    #[tokio::test]
    async fn copy_basic() {
        let tmp = TempDir::new().unwrap();
        let mgr = IndexManager::new(tmp.path());
        mgr.create_tenant("src").unwrap();
        mgr.add_documents_sync("src", make_docs(&["1", "2"]))
            .await
            .unwrap();

        mgr.copy_index("src", "dst", None).await.unwrap();

        let src_results = mgr.search("src", "Item", None, None, 10).unwrap();
        assert_eq!(src_results.documents.len(), 2);
        let dst_results = mgr.search("dst", "Item", None, None, 10).unwrap();
        assert_eq!(dst_results.documents.len(), 2);
    }

    #[tokio::test]
    async fn copy_nonexistent_creates_empty() {
        let tmp = TempDir::new().unwrap();
        let mgr = IndexManager::new(tmp.path());
        mgr.copy_index("ghost", "dst", None).await.unwrap();

        let results = mgr.search("dst", "", None, None, 10).unwrap();
        assert_eq!(results.documents.len(), 0);
    }

    #[tokio::test]
    async fn copy_settings_only() {
        let tmp = TempDir::new().unwrap();
        let mgr = IndexManager::new(tmp.path());
        mgr.create_tenant("src").unwrap();
        mgr.add_documents_sync("src", make_docs(&["1", "2"]))
            .await
            .unwrap();

        let settings_path = tmp.path().join("src").join("settings.json");
        assert!(settings_path.exists());

        let scope = vec!["settings".to_string()];
        mgr.copy_index("src", "dst", Some(&scope)).await.unwrap();

        let dst_settings = tmp.path().join("dst").join("settings.json");
        assert!(dst_settings.exists());

        let results = mgr.search("dst", "Item", None, None, 10).unwrap();
        assert_eq!(results.documents.len(), 0);
    }

    #[tokio::test]
    async fn move_then_search_destination() {
        let tmp = TempDir::new().unwrap();
        let mgr = IndexManager::new(tmp.path());
        mgr.create_tenant("products").unwrap();
        mgr.add_documents_sync("products", make_docs(&["a", "b", "c"]))
            .await
            .unwrap();

        mgr.move_index("products", "products_v2").await.unwrap();
        let r = mgr.search("products_v2", "Item", None, None, 10).unwrap();
        assert_eq!(r.documents.len(), 3);

        mgr.move_index("products_v2", "products").await.unwrap();
        let r = mgr.search("products", "Item", None, None, 10).unwrap();
        assert_eq!(r.documents.len(), 3);
        assert!(mgr.search("products_v2", "", None, None, 10).is_err());
    }
}

// ============================================================
// SNAPSHOT
// ============================================================

mod snapshot {
    use super::*;
    use flapjack::index::snapshot::{export_to_bytes, import_from_bytes};

    #[tokio::test]
    async fn roundtrip() {
        let src_dir = TempDir::new().unwrap();
        let dest_dir = TempDir::new().unwrap();

        let src_mgr = IndexManager::new(src_dir.path());
        src_mgr.create_tenant("products").unwrap();

        let docs: Vec<Document> = vec![
            json!({"_id": "1", "name": "Gaming Laptop", "price": 1299}),
            json!({"_id": "2", "name": "Wireless Mouse", "price": 49}),
            json!({"_id": "3", "name": "Mechanical Keyboard", "price": 159}),
        ]
        .into_iter()
        .map(|v| Document::from_json(&v).unwrap())
        .collect();
        src_mgr.add_documents_sync("products", docs).await.unwrap();

        let search_before = src_mgr
            .search("products", "laptop", None, None, 10)
            .unwrap();
        assert_eq!(search_before.total, 1, "Should find laptop before export");

        let index_path = src_dir.path().join("products");
        let tarball = export_to_bytes(&index_path).unwrap();
        assert!(tarball.len() > 100, "Tarball should have content");

        let dest_index_path = dest_dir.path().join("products");
        import_from_bytes(&tarball, &dest_index_path).unwrap();

        let dest_mgr = IndexManager::new(dest_dir.path());
        let search_after = dest_mgr
            .search("products", "laptop", None, None, 10)
            .unwrap();
        assert_eq!(search_after.total, 1, "Should find laptop after import");
        assert_eq!(
            search_after.documents[0]
                .document
                .fields
                .get("name")
                .and_then(|v| v.as_text()),
            Some("Gaming Laptop"),
        );

        let all_docs = dest_mgr.search("products", "", None, None, 100).unwrap();
        assert_eq!(all_docs.total, 3, "All 3 docs should be present");
    }
}

// ============================================================
// OPLOG INTEGRATION
// ============================================================

mod oplog {
    use super::*;

    #[tokio::test]
    async fn written_after_upsert() {
        let tmp = TempDir::new().unwrap();
        let mgr = IndexManager::new(tmp.path());
        mgr.create_tenant("oplog_test").unwrap();

        mgr.add_documents_sync(
            "oplog_test",
            vec![
                make_doc("1", "Gaming Laptop"),
                make_doc("2", "Wireless Mouse"),
                make_doc("3", "Mechanical Keyboard"),
            ],
        )
        .await
        .unwrap();

        let entries = read_oplog_entries(&tmp.path().join("oplog_test").join("oplog"));
        assert!(
            entries.len() >= 3,
            "expected >=3 oplog entries, got {}",
            entries.len()
        );
        assert_eq!(entries[0]["op_type"], "upsert");
        assert_eq!(entries[0]["tenant_id"], "oplog_test");
        assert!(entries[0]["seq"].as_u64().unwrap() >= 1);
        assert!(entries[0]["timestamp_ms"].as_u64().unwrap() > 0);
    }

    #[tokio::test]
    async fn written_after_delete() {
        let tmp = TempDir::new().unwrap();
        let mgr = IndexManager::new(tmp.path());
        mgr.create_tenant("oplog_del").unwrap();

        mgr.add_documents_sync(
            "oplog_del",
            vec![make_doc("1", "Laptop"), make_doc("2", "Mouse")],
        )
        .await
        .unwrap();
        mgr.delete_documents_sync("oplog_del", vec!["1".to_string()])
            .await
            .unwrap();

        let entries = read_oplog_entries(&tmp.path().join("oplog_del").join("oplog"));
        let delete_entries: Vec<_> = entries
            .iter()
            .filter(|e| e["op_type"] == "delete")
            .collect();
        assert!(!delete_entries.is_empty(), "expected delete oplog entries");
    }

    #[tokio::test]
    async fn seq_monotonic() {
        let tmp = TempDir::new().unwrap();
        let mgr = IndexManager::new(tmp.path());
        mgr.create_tenant("oplog_seq").unwrap();

        mgr.add_documents_sync("oplog_seq", vec![make_doc("1", "A")])
            .await
            .unwrap();
        mgr.add_documents_sync("oplog_seq", vec![make_doc("2", "B")])
            .await
            .unwrap();
        mgr.add_documents_sync("oplog_seq", vec![make_doc("3", "C")])
            .await
            .unwrap();

        let entries = read_oplog_entries(&tmp.path().join("oplog_seq").join("oplog"));
        assert!(entries.len() >= 3);
        for i in 1..entries.len() {
            let prev = entries[i - 1]["seq"].as_u64().unwrap();
            let curr = entries[i]["seq"].as_u64().unwrap();
            assert!(curr > prev, "seq not monotonic: {} then {}", prev, curr);
        }
    }

    #[tokio::test]
    async fn survives_restart() {
        let tmp = TempDir::new().unwrap();
        let base = tmp.path().to_path_buf();

        {
            let mgr = IndexManager::new(&base);
            mgr.create_tenant("oplog_restart").unwrap();
            mgr.add_documents_sync(
                "oplog_restart",
                vec![make_doc("1", "Alpha"), make_doc("2", "Beta")],
            )
            .await
            .unwrap();
        }

        let oplog_dir = base.join("oplog_restart").join("oplog");
        let entries_before = read_oplog_entries(&oplog_dir);
        assert!(entries_before.len() >= 2);
        let max_seq_before = entries_before.last().unwrap()["seq"].as_u64().unwrap();

        {
            let mgr = IndexManager::new(&base);
            mgr.create_tenant("oplog_restart").unwrap();
            mgr.add_documents_sync("oplog_restart", vec![make_doc("3", "Gamma")])
                .await
                .unwrap();
        }

        let entries_after = read_oplog_entries(&oplog_dir);
        assert!(entries_after.len() >= 3);
        let new_entries: Vec<_> = entries_after
            .iter()
            .filter(|e| e["seq"].as_u64().unwrap() > max_seq_before)
            .collect();
        assert!(!new_entries.is_empty(), "no new entries after restart");
        assert!(new_entries[0]["seq"].as_u64().unwrap() > max_seq_before);
    }

    #[tokio::test]
    async fn concurrent_tenant_oplogs() {
        let tmp = TempDir::new().unwrap();
        let mgr = IndexManager::new(tmp.path());
        mgr.create_tenant("tenant_a").unwrap();
        mgr.create_tenant("tenant_b").unwrap();

        let (ra, rb) = tokio::join!(
            mgr.add_documents_sync(
                "tenant_a",
                vec![make_doc("a1", "Apple"), make_doc("a2", "Avocado")]
            ),
            mgr.add_documents_sync(
                "tenant_b",
                vec![make_doc("b1", "Banana"), make_doc("b2", "Blueberry")]
            ),
        );
        ra.unwrap();
        rb.unwrap();

        let entries_a = read_oplog_entries(&tmp.path().join("tenant_a").join("oplog"));
        let entries_b = read_oplog_entries(&tmp.path().join("tenant_b").join("oplog"));

        assert!(entries_a.len() >= 2);
        assert!(entries_b.len() >= 2);
        assert!(entries_a.iter().all(|e| e["tenant_id"] == "tenant_a"));
        assert!(entries_b.iter().all(|e| e["tenant_id"] == "tenant_b"));

        for entries in [&entries_a, &entries_b] {
            for i in 1..entries.len() {
                assert!(
                    entries[i]["seq"].as_u64().unwrap() > entries[i - 1]["seq"].as_u64().unwrap()
                );
            }
        }
    }

    #[tokio::test]
    async fn no_seq_duplicates_with_handler_and_writes() {
        let tmp = TempDir::new().unwrap();
        let mgr = IndexManager::new(tmp.path());
        mgr.create_tenant("oplog_dup").unwrap();

        mgr.add_documents_sync(
            "oplog_dup",
            vec![make_doc("1", "Alpha"), make_doc("2", "Beta")],
        )
        .await
        .unwrap();
        mgr.append_oplog("oplog_dup", "settings", json!({"test": true}));
        mgr.append_oplog("oplog_dup", "save_synonym", json!({"word": "hi"}));
        mgr.add_documents_sync("oplog_dup", vec![make_doc("3", "Gamma")])
            .await
            .unwrap();
        mgr.append_oplog("oplog_dup", "clear_rules", json!({}));

        let entries = read_oplog_entries(&tmp.path().join("oplog_dup").join("oplog"));
        let mut seqs: Vec<u64> = entries.iter().map(|e| e["seq"].as_u64().unwrap()).collect();
        let total = seqs.len();
        seqs.sort();
        seqs.dedup();
        assert_eq!(
            seqs.len(),
            total,
            "duplicate seq numbers found! seqs: {:?}",
            seqs
        );
        for i in 1..seqs.len() {
            assert!(seqs[i] > seqs[i - 1]);
        }
    }
}
// ============================================================
// ============================================================
// OPLOG REPLAY (crash recovery)
// ============================================================

mod oplog_replay {
    use super::*;

    fn nuke_and_recreate_index(tenant_path: &Path) {
        let oplog_backup: Vec<(String, Vec<u8>)> = {
            let od = tenant_path.join("oplog");
            if od.exists() {
                std::fs::read_dir(&od)
                    .unwrap()
                    .filter_map(|e| e.ok())
                    .filter(|e| e.path().is_file())
                    .map(|e| {
                        (
                            e.file_name().to_str().unwrap().to_string(),
                            std::fs::read(e.path()).unwrap(),
                        )
                    })
                    .collect()
            } else {
                vec![]
            }
        };
        let cs = tenant_path.join("committed_seq");
        let _cs_data = if cs.exists() {
            Some(std::fs::read_to_string(&cs).unwrap())
        } else {
            None
        };
        let set_data = std::fs::read(tenant_path.join("settings.json")).ok();

        for attempt in 0..10 {
            match std::fs::remove_dir_all(tenant_path) {
                Ok(()) => break,
                Err(_) if attempt < 9 => {
                    std::thread::sleep(std::time::Duration::from_millis(100));
                }
                Err(e) => panic!("failed to remove_dir_all after retries: {e}"),
            }
        }
        std::fs::create_dir_all(tenant_path).unwrap();

        let schema = flapjack::index::schema::Schema::builder().build();
        let _ = tantivy::Index::create_in_dir(tenant_path, schema.to_tantivy()).unwrap();

        let od = tenant_path.join("oplog");
        std::fs::create_dir_all(&od).unwrap();
        for (name, data) in oplog_backup {
            std::fs::write(od.join(name), data).unwrap();
        }
        if let Some(d) = set_data {
            std::fs::write(tenant_path.join("settings.json"), d).unwrap();
        }
    }

    #[tokio::test]
    async fn replay_recovers_docs_after_simulated_crash() {
        let tmp = TempDir::new().unwrap();
        let base = tmp.path().to_path_buf();

        {
            let mgr = IndexManager::new(&base);
            mgr.create_tenant("replay_test").unwrap();
            mgr.add_documents_sync(
                "replay_test",
                vec![
                    make_doc("1", "Alpha"),
                    make_doc("2", "Beta"),
                    make_doc("3", "Gamma"),
                ],
            )
            .await
            .unwrap();
            let r = mgr.search("replay_test", "", None, None, 100).unwrap();
            assert_eq!(r.total, 3);
        }

        let cs_path = base.join("replay_test").join("committed_seq");
        assert!(cs_path.exists(), "committed_seq sidecar should exist");
        let seq: u64 = std::fs::read_to_string(&cs_path)
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        assert!(seq >= 3, "committed_seq should be >= 3, got {}", seq);

        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
        nuke_and_recreate_index(&base.join("replay_test"));
        std::fs::write(&cs_path, "0").unwrap();

        {
            let mgr = IndexManager::new(&base);
            let r = mgr.search("replay_test", "", None, None, 100).unwrap();
            assert_eq!(r.total, 3, "oplog replay should recover all 3 docs");
        }

        let new_seq: u64 = std::fs::read_to_string(&cs_path)
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        assert!(new_seq >= 3, "committed_seq should be updated after replay");
    }

    #[tokio::test]
    async fn replay_recovers_when_tantivy_files_missing() {
        let tmp = TempDir::new().unwrap();
        let base = tmp.path().to_path_buf();

        let mgr = IndexManager::new(&base);
        mgr.create_tenant("replay_nuke").unwrap();
        mgr.add_documents_sync(
            "replay_nuke",
            vec![
                make_doc("1", "Alpha"),
                make_doc("2", "Beta"),
                make_doc("3", "Gamma"),
            ],
        )
        .await
        .unwrap();
        let r = mgr.search("replay_nuke", "", None, None, 100).unwrap();
        assert_eq!(r.total, 3);

        // Shutdown write queue before directory manipulation
        mgr.unload(&"replay_nuke".to_string()).unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        let tenant_path = base.join("replay_nuke");
        let oplog_backup: Vec<(String, Vec<u8>)> = {
            let od = tenant_path.join("oplog");
            std::fs::read_dir(&od)
                .unwrap()
                .filter_map(|e| e.ok())
                .filter(|e| e.path().is_file())
                .map(|e| {
                    (
                        e.file_name().to_str().unwrap().to_string(),
                        std::fs::read(e.path()).unwrap(),
                    )
                })
                .collect()
        };
        let settings_data = std::fs::read(tenant_path.join("settings.json")).ok();
        let _cs_data = std::fs::read_to_string(tenant_path.join("committed_seq")).ok();

        std::fs::remove_dir_all(&tenant_path).unwrap();
        std::fs::create_dir_all(&tenant_path).unwrap();
        let od = tenant_path.join("oplog");
        std::fs::create_dir_all(&od).unwrap();
        for (name, data) in oplog_backup {
            std::fs::write(od.join(name), data).unwrap();
        }
        if let Some(d) = settings_data {
            std::fs::write(tenant_path.join("settings.json"), d).unwrap();
        }
        std::fs::write(tenant_path.join("committed_seq"), "0").unwrap();

        let mgr2 = IndexManager::new(&base);
        let r = mgr2.search("replay_nuke", "", None, None, 100).unwrap();
        assert_eq!(
            r.total, 3,
            "oplog replay should recover all 3 docs even with no tantivy files"
        );
        mgr2.unload(&"replay_nuke".to_string()).unwrap();
    }

    #[tokio::test]
    async fn replay_handles_deletes() {
        let tmp = TempDir::new().unwrap();
        let base = tmp.path().to_path_buf();

        {
            let mgr = IndexManager::new(&base);
            mgr.create_tenant("replay_del").unwrap();
            mgr.add_documents_sync(
                "replay_del",
                vec![make_doc("1", "Alpha"), make_doc("2", "Beta")],
            )
            .await
            .unwrap();
            mgr.delete_documents_sync("replay_del", vec!["1".to_string()])
                .await
                .unwrap();
            let r = mgr.search("replay_del", "", None, None, 100).unwrap();
            assert_eq!(r.total, 1);
        }

        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
        nuke_and_recreate_index(&base.join("replay_del"));
        std::fs::write(base.join("replay_del").join("committed_seq"), "0").unwrap();

        {
            let mgr = IndexManager::new(&base);
            let r = mgr.search("replay_del", "", None, None, 100).unwrap();
            assert_eq!(
                r.total, 1,
                "replay should result in 1 doc (2 upserted, 1 deleted)"
            );
            assert_eq!(r.documents[0].document.id, "2");
        }
    }

    #[tokio::test]
    async fn partial_replay_skips_committed_ops() {
        let tmp = TempDir::new().unwrap();
        let base = tmp.path().to_path_buf();

        {
            let mgr = IndexManager::new(&base);
            mgr.create_tenant("replay_partial").unwrap();
            mgr.add_documents_sync("replay_partial", vec![make_doc("1", "Alpha")])
                .await
                .unwrap();
            mgr.add_documents_sync("replay_partial", vec![make_doc("2", "Beta")])
                .await
                .unwrap();
            mgr.add_documents_sync("replay_partial", vec![make_doc("3", "Gamma")])
                .await
                .unwrap();
        }

        let entries = read_oplog_entries(&base.join("replay_partial").join("oplog"));
        assert!(entries.len() >= 3);
        let mid_seq = entries[0]["seq"].as_u64().unwrap();

        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
        nuke_and_recreate_index(&base.join("replay_partial"));
        std::fs::write(
            base.join("replay_partial").join("committed_seq"),
            mid_seq.to_string(),
        )
        .unwrap();

        {
            let mgr = IndexManager::new(&base);
            let r = mgr.search("replay_partial", "", None, None, 100).unwrap();
            assert!(
                r.total >= 2,
                "should replay at least 2 ops after mid_seq={}, got {}",
                mid_seq,
                r.total
            );
        }
    }

    #[tokio::test]
    async fn no_replay_when_committed_seq_current() {
        let tmp = TempDir::new().unwrap();
        let base = tmp.path().to_path_buf();

        {
            let mgr = IndexManager::new(&base);
            mgr.create_tenant("replay_noop").unwrap();
            mgr.add_documents_sync("replay_noop", vec![make_doc("1", "Alpha")])
                .await
                .unwrap();
        }

        {
            let mgr = IndexManager::new(&base);
            let r = mgr.search("replay_noop", "", None, None, 100).unwrap();
            assert_eq!(r.total, 1);
        }
    }

    #[tokio::test]
    async fn replay_with_upsert_overwrites() {
        let tmp = TempDir::new().unwrap();
        let base = tmp.path().to_path_buf();

        {
            let mgr = IndexManager::new(&base);
            mgr.create_tenant("replay_upsert").unwrap();
            mgr.add_documents_sync("replay_upsert", vec![make_doc("1", "Original")])
                .await
                .unwrap();
            mgr.add_documents_sync("replay_upsert", vec![make_doc("1", "Updated")])
                .await
                .unwrap();
            let r = mgr.search("replay_upsert", "", None, None, 100).unwrap();
            assert_eq!(r.total, 1);
        }

        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
        nuke_and_recreate_index(&base.join("replay_upsert"));
        std::fs::write(base.join("replay_upsert").join("committed_seq"), "0").unwrap();

        {
            let mgr = IndexManager::new(&base);
            let r = mgr.search("replay_upsert", "", None, None, 100).unwrap();
            assert_eq!(r.total, 1, "upsert replay should deduplicate to 1 doc");
            assert_eq!(r.documents[0].document.id, "1");
            let name = match r.documents[0].document.fields.get("name").unwrap() {
                flapjack::types::FieldValue::Text(s) => s.clone(),
                _ => panic!("expected text"),
            };
            assert_eq!(name, "Updated", "replayed doc should have latest value");
        }
    }

    #[tokio::test]
    async fn replay_many_docs_stress() {
        let tmp = TempDir::new().unwrap();
        let base = tmp.path().to_path_buf();
        let n = 200;

        {
            let mgr = IndexManager::new(&base);
            mgr.create_tenant("replay_stress").unwrap();
            let docs: Vec<Document> = (0..n)
                .map(|i| make_doc(&i.to_string(), &format!("Item {}", i)))
                .collect();
            mgr.add_documents_sync("replay_stress", docs).await.unwrap();
            let r = mgr.search("replay_stress", "", None, None, 1).unwrap();
            assert_eq!(r.total, n);
        }

        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
        nuke_and_recreate_index(&base.join("replay_stress"));
        std::fs::write(base.join("replay_stress").join("committed_seq"), "0").unwrap();

        {
            let mgr = IndexManager::new(&base);
            let r = mgr.search("replay_stress", "", None, None, 1).unwrap();
            assert_eq!(r.total, n, "all {} docs should be recovered", n);
        }
    }

    #[tokio::test]
    async fn replay_interleaved_adds_and_deletes() {
        let tmp = TempDir::new().unwrap();
        let base = tmp.path().to_path_buf();

        {
            let mgr = IndexManager::new(&base);
            mgr.create_tenant("replay_interleave").unwrap();
            mgr.add_documents_sync(
                "replay_interleave",
                vec![
                    make_doc("1", "A"),
                    make_doc("2", "B"),
                    make_doc("3", "C"),
                    make_doc("4", "D"),
                    make_doc("5", "E"),
                ],
            )
            .await
            .unwrap();
            mgr.delete_documents_sync("replay_interleave", vec!["2".to_string(), "4".to_string()])
                .await
                .unwrap();
            mgr.add_documents_sync("replay_interleave", vec![make_doc("6", "F")])
                .await
                .unwrap();
            mgr.delete_documents_sync("replay_interleave", vec!["1".to_string()])
                .await
                .unwrap();
            let r = mgr
                .search("replay_interleave", "", None, None, 100)
                .unwrap();
            assert_eq!(r.total, 3);
        }

        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
        nuke_and_recreate_index(&base.join("replay_interleave"));
        std::fs::write(base.join("replay_interleave").join("committed_seq"), "0").unwrap();

        {
            let mgr = IndexManager::new(&base);
            let r = mgr
                .search("replay_interleave", "", None, None, 100)
                .unwrap();
            assert_eq!(
                r.total, 3,
                "should have 3 docs after replay (5 added, 3 deleted, 1 re-added)"
            );
            let mut ids: Vec<String> = r.documents.iter().map(|d| d.document.id.clone()).collect();
            ids.sort();
            assert_eq!(ids, vec!["3", "5", "6"]);
        }
    }

    #[tokio::test]
    async fn replay_empty_oplog_is_noop() {
        let tmp = TempDir::new().unwrap();
        let base = tmp.path().to_path_buf();

        {
            let mgr = IndexManager::new(&base);
            mgr.create_tenant("replay_empty").unwrap();
        }

        {
            let mgr = IndexManager::new(&base);
            let r = mgr.search("replay_empty", "", None, None, 100).unwrap();
            assert_eq!(r.total, 0);
        }
    }

    #[tokio::test]
    async fn replay_tolerates_corrupt_oplog_line() {
        let tmp = TempDir::new().unwrap();
        let base = tmp.path().to_path_buf();

        {
            let mgr = IndexManager::new(&base);
            mgr.create_tenant("replay_corrupt").unwrap();
            mgr.add_documents_sync("replay_corrupt", vec![make_doc("1", "Good")])
                .await
                .unwrap();
        }

        let oplog_dir = base.join("replay_corrupt").join("oplog");
        let seg_files: Vec<_> = std::fs::read_dir(&oplog_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_str().unwrap().ends_with(".jsonl"))
            .collect();
        assert!(!seg_files.is_empty());
        let seg_path = seg_files[0].path();
        let mut content = std::fs::read_to_string(&seg_path).unwrap();
        content.push_str("{this is not valid json\n");
        content.push_str("{\"seq\":999,\"timestamp_ms\":1,\"node_id\":\"n\",\"tenant_id\":\"replay_corrupt\",\"op_type\":\"upsert\",\"payload\":{\"objectID\":\"2\",\"body\":{\"_id\":\"2\",\"name\":\"AfterCorrupt\"}}}\n");
        std::fs::write(&seg_path, content).unwrap();

        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
        nuke_and_recreate_index(&base.join("replay_corrupt"));
        std::fs::write(base.join("replay_corrupt").join("committed_seq"), "0").unwrap();

        {
            let mgr = IndexManager::new(&base);
            let r = mgr.search("replay_corrupt", "", None, None, 100).unwrap();
            assert!(
                r.total >= 1,
                "should recover at least the valid entries, got {}",
                r.total
            );
        }
    }

    #[tokio::test]
    async fn replay_with_settings_op() {
        let tmp = TempDir::new().unwrap();
        let base = tmp.path().to_path_buf();

        {
            let mgr = IndexManager::new(&base);
            mgr.create_tenant("replay_settings").unwrap();
            mgr.add_documents_sync("replay_settings", vec![make_doc("1", "Test")])
                .await
                .unwrap();
            mgr.append_oplog(
                "replay_settings",
                "settings",
                serde_json::json!({
                    "body": {"searchableAttributes": ["name"], "queryType": "prefixAll"}
                }),
            );
        }

        let settings_path = base.join("replay_settings").join("settings.json");
        let _original_settings = std::fs::read_to_string(&settings_path).unwrap();

        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
        nuke_and_recreate_index(&base.join("replay_settings"));
        std::fs::write(base.join("replay_settings").join("committed_seq"), "0").unwrap();
        std::fs::write(&settings_path, "{}").unwrap();

        {
            let mgr = IndexManager::new(&base);
            let r = mgr.search("replay_settings", "", None, None, 100).unwrap();
            assert_eq!(r.total, 1, "doc should be recovered");
            let replayed_settings = std::fs::read_to_string(&settings_path).unwrap();
            assert!(
                replayed_settings.contains("searchableAttributes"),
                "settings op should be replayed"
            );
        }
    }

    #[tokio::test]
    async fn replay_after_compaction_still_works() {
        let tmp = TempDir::new().unwrap();
        let base = tmp.path().to_path_buf();
        std::env::set_var("FLAPJACK_OPLOG_RETENTION", "5");

        {
            let mgr = IndexManager::new(&base);
            mgr.create_tenant("replay_compact").unwrap();
            for i in 0..20 {
                mgr.add_documents_sync(
                    "replay_compact",
                    vec![make_doc(&format!("d{}", i), &format!("Item {}", i))],
                )
                .await
                .unwrap();
            }
            let r = mgr.search("replay_compact", "", None, None, 1).unwrap();
            assert_eq!(r.total, 20);
        }

        let oplog_dir = base.join("replay_compact").join("oplog");
        let _seg_count = std::fs::read_dir(&oplog_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_str().unwrap().ends_with(".jsonl"))
            .count();

        let entries = read_oplog_entries(&oplog_dir);
        let _min_seq = entries
            .first()
            .map(|e| e["seq"].as_u64().unwrap())
            .unwrap_or(0);

        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
        nuke_and_recreate_index(&base.join("replay_compact"));
        std::fs::write(base.join("replay_compact").join("committed_seq"), "0").unwrap();

        {
            let mgr = IndexManager::new(&base);
            let r = mgr.search("replay_compact", "", None, None, 1).unwrap();
            let remaining_entries = read_oplog_entries(&oplog_dir);
            assert!(r.total >= remaining_entries.len(),
                "should recover at least as many docs as oplog entries remain (got {} docs, {} entries)",
                r.total, remaining_entries.len());
        }

        std::env::remove_var("FLAPJACK_OPLOG_RETENTION");
    }

    #[tokio::test]
    async fn committed_seq_written_on_each_commit() {
        let tmp = TempDir::new().unwrap();
        let mgr = IndexManager::new(tmp.path());
        mgr.create_tenant("cs_test").unwrap();

        mgr.add_documents_sync("cs_test", vec![make_doc("1", "A")])
            .await
            .unwrap();
        let cs_path = tmp.path().join("cs_test").join("committed_seq");
        assert!(cs_path.exists());
        let s1: u64 = std::fs::read_to_string(&cs_path)
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        assert!(s1 >= 1);

        mgr.add_documents_sync("cs_test", vec![make_doc("2", "B")])
            .await
            .unwrap();
        let s2: u64 = std::fs::read_to_string(&cs_path)
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        assert!(s2 > s1, "committed_seq should increase: {} -> {}", s1, s2);
    }
}
