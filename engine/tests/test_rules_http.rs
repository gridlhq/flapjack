use flapjack::IndexManager;
use serde_json::json;
use std::sync::Arc;
use tempfile::TempDir;

async fn setup_test() -> (Arc<IndexManager>, TempDir, String) {
    let temp_dir = TempDir::new().unwrap();
    let manager = IndexManager::new(temp_dir.path());
    let index_name = format!("test_{}", uuid::Uuid::new_v4());
    manager.create_tenant(&index_name).unwrap();
    (manager, temp_dir, index_name)
}

#[tokio::test]
async fn test_rule_with_expired_validity() {
    let (manager, temp_dir, index_name) = setup_test().await;

    let past = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
        - 7200;

    let rule = flapjack::index::rules::Rule {
        object_id: "expired-rule".to_string(),
        conditions: vec![flapjack::index::rules::Condition {
            pattern: "laptop".to_string(),
            anchoring: flapjack::index::rules::Anchoring::Contains,
            alternatives: None,
            context: None,
            filters: None,
        }],
        consequence: flapjack::index::rules::Consequence {
            promote: Some(vec![flapjack::index::rules::Promote::Single {
                object_id: "promoted-item".to_string(),
                position: 0,
            }]),
            hide: None,
            filter_promotes: None,
            user_data: None,
            params: None,
        },
        description: None,
        enabled: None,
        validity: Some(vec![flapjack::index::rules::TimeRange {
            from: past - 3600,
            until: past,
        }]),
    };

    let rules_path = temp_dir.path().join(&index_name).join("rules.json");
    let mut store = flapjack::index::rules::RuleStore::new();
    store.insert(rule);
    store.save(&rules_path).unwrap();

    let docs = vec![
        flapjack::types::Document::from_json(&json!({"_id": "1", "name": "Gaming Laptop"}))
            .unwrap(),
        flapjack::types::Document::from_json(&json!({"_id": "2", "name": "Office Laptop"}))
            .unwrap(),
        flapjack::types::Document::from_json(
            &json!({"_id": "promoted-item", "name": "Budget Laptop"}),
        )
        .unwrap(),
    ];
    manager.add_documents_sync(&index_name, docs).await.unwrap();

    let result = manager
        .search(&index_name, "laptop", None, None, 10)
        .unwrap();

    assert!(
        result.documents[0].document.id != "promoted-item",
        "Expired rule should not apply"
    );
}

#[tokio::test]
async fn test_context_based_rule() {
    let (manager, temp_dir, index_name) = setup_test().await;

    let rule = flapjack::index::rules::Rule {
        object_id: "context-rule".to_string(),
        conditions: vec![flapjack::index::rules::Condition {
            pattern: "laptop".to_string(),
            anchoring: flapjack::index::rules::Anchoring::Contains,
            alternatives: None,
            context: Some("mobile".to_string()),
            filters: None,
        }],
        consequence: flapjack::index::rules::Consequence {
            promote: Some(vec![flapjack::index::rules::Promote::Single {
                object_id: "mobile-item".to_string(),
                position: 0,
            }]),
            hide: None,
            filter_promotes: None,
            user_data: None,
            params: None,
        },
        description: None,
        enabled: None,
        validity: None,
    };

    let rules_path = temp_dir.path().join(&index_name).join("rules.json");
    let mut store = flapjack::index::rules::RuleStore::new();
    store.insert(rule);
    store.save(&rules_path).unwrap();

    let docs = vec![
        flapjack::types::Document::from_json(&json!({"_id": "1", "name": "Gaming Laptop"}))
            .unwrap(),
        flapjack::types::Document::from_json(
            &json!({"_id": "mobile-item", "name": "Budget Laptop"}),
        )
        .unwrap(),
    ];
    manager.add_documents_sync(&index_name, docs).await.unwrap();

    let result = manager
        .search(&index_name, "laptop", None, None, 10)
        .unwrap();

    assert!(
        result.documents[0].document.id != "mobile-item",
        "Rule should not apply without context"
    );
}

#[tokio::test]
async fn test_pin_deduplication() {
    let (manager, temp_dir, index_name) = setup_test().await;

    let rule = flapjack::index::rules::Rule {
        object_id: "dedup-rule".to_string(),
        conditions: vec![flapjack::index::rules::Condition {
            pattern: "laptop".to_string(),
            anchoring: flapjack::index::rules::Anchoring::Contains,
            alternatives: None,
            context: None,
            filters: None,
        }],
        consequence: flapjack::index::rules::Consequence {
            promote: Some(vec![flapjack::index::rules::Promote::Single {
                object_id: "1".to_string(),
                position: 0,
            }]),
            hide: None,
            filter_promotes: None,
            user_data: None,
            params: None,
        },
        description: None,
        enabled: None,
        validity: None,
    };

    let rules_path = temp_dir.path().join(&index_name).join("rules.json");
    let mut store = flapjack::index::rules::RuleStore::new();
    store.insert(rule);
    store.save(&rules_path).unwrap();

    let docs = vec![
        flapjack::types::Document::from_json(
            &json!({"_id": "1", "name": "Gaming Laptop", "popularity": 500}),
        )
        .unwrap(),
        flapjack::types::Document::from_json(
            &json!({"_id": "2", "name": "Office Laptop", "popularity": 300}),
        )
        .unwrap(),
        flapjack::types::Document::from_json(
            &json!({"_id": "3", "name": "Budget Laptop", "popularity": 100}),
        )
        .unwrap(),
    ];
    manager.add_documents_sync(&index_name, docs).await.unwrap();

    let result = manager
        .search(&index_name, "laptop", None, None, 10)
        .unwrap();

    assert_eq!(result.documents[0].document.id, "1");

    let id_positions: Vec<_> = result
        .documents
        .iter()
        .enumerate()
        .filter(|(_, d)| d.document.id == "1")
        .map(|(i, _)| i)
        .collect();

    assert_eq!(id_positions.len(), 1, "Pinned item should appear only once");
    assert_eq!(id_positions[0], 0, "Pinned item should be at position 0");
}

#[tokio::test]
async fn test_disabled_rule_ignored() {
    let (manager, temp_dir, index_name) = setup_test().await;

    let rule = flapjack::index::rules::Rule {
        object_id: "disabled-rule".to_string(),
        conditions: vec![flapjack::index::rules::Condition {
            pattern: "laptop".to_string(),
            anchoring: flapjack::index::rules::Anchoring::Contains,
            alternatives: None,
            context: None,
            filters: None,
        }],
        consequence: flapjack::index::rules::Consequence {
            promote: Some(vec![flapjack::index::rules::Promote::Single {
                object_id: "promoted".to_string(),
                position: 0,
            }]),
            hide: None,
            filter_promotes: None,
            user_data: None,
            params: None,
        },
        description: None,
        enabled: Some(false),
        validity: None,
    };

    let rules_path = temp_dir.path().join(&index_name).join("rules.json");
    let mut store = flapjack::index::rules::RuleStore::new();
    store.insert(rule);
    store.save(&rules_path).unwrap();

    let docs = vec![
        flapjack::types::Document::from_json(&json!({"_id": "1", "name": "Gaming Laptop"}))
            .unwrap(),
        flapjack::types::Document::from_json(
            &json!({"_id": "promoted", "name": "Promoted Laptop"}),
        )
        .unwrap(),
    ];
    manager.add_documents_sync(&index_name, docs).await.unwrap();

    let result = manager
        .search(&index_name, "laptop", None, None, 10)
        .unwrap();

    assert!(
        result.documents[0].document.id != "promoted",
        "Disabled rule should not apply"
    );
}
