//! Consolidated rules integration tests.
//!
//! Merged from (all deleted):
//!   - test_rules_http.rs      (validity, context, dedup, disabled)
//!   - test_rules_consequences.rs  (applied_rules, user_data, query rewrite)

use crate::error::Result;
use crate::types::Document;
use crate::IndexManager;
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use tempfile::TempDir;

// ============================================================
// Helper — creates a fresh index with a UUID-based name
// (avoids cross-test interference when tests run in parallel)
// ============================================================

async fn setup_test() -> (Arc<IndexManager>, TempDir, String) {
    let temp_dir = TempDir::new().unwrap();
    let manager = IndexManager::new(temp_dir.path());
    let index_name = format!("test_{}", uuid::Uuid::new_v4());
    manager.create_tenant(&index_name).unwrap();
    (manager, temp_dir, index_name)
}

// ============================================================
// From test_rules_http.rs
// ============================================================

#[tokio::test]
async fn test_rule_with_expired_validity() {
    let (manager, temp_dir, index_name) = setup_test().await;

    let past = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
        - 7200;

    let rule = crate::index::rules::Rule {
        object_id: "expired-rule".to_string(),
        conditions: vec![crate::index::rules::Condition {
            pattern: "laptop".to_string(),
            anchoring: crate::index::rules::Anchoring::Contains,
            alternatives: None,
            context: None,
            filters: None,
        }],
        consequence: crate::index::rules::Consequence {
            promote: Some(vec![crate::index::rules::Promote::Single {
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
        validity: Some(vec![crate::index::rules::TimeRange {
            from: past - 3600,
            until: past,
        }]),
    };

    let rules_path = temp_dir.path().join(&index_name).join("rules.json");
    let mut store = crate::index::rules::RuleStore::new();
    store.insert(rule);
    store.save(&rules_path).unwrap();

    let docs = vec![
        crate::types::Document::from_json(&json!({"_id": "1", "name": "Gaming Laptop"})).unwrap(),
        crate::types::Document::from_json(&json!({"_id": "2", "name": "Office Laptop"})).unwrap(),
        crate::types::Document::from_json(
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

    let rule = crate::index::rules::Rule {
        object_id: "context-rule".to_string(),
        conditions: vec![crate::index::rules::Condition {
            pattern: "laptop".to_string(),
            anchoring: crate::index::rules::Anchoring::Contains,
            alternatives: None,
            context: Some("mobile".to_string()),
            filters: None,
        }],
        consequence: crate::index::rules::Consequence {
            promote: Some(vec![crate::index::rules::Promote::Single {
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
    let mut store = crate::index::rules::RuleStore::new();
    store.insert(rule);
    store.save(&rules_path).unwrap();

    let docs = vec![
        crate::types::Document::from_json(&json!({"_id": "1", "name": "Gaming Laptop"})).unwrap(),
        crate::types::Document::from_json(&json!({"_id": "mobile-item", "name": "Budget Laptop"}))
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

    let rule = crate::index::rules::Rule {
        object_id: "dedup-rule".to_string(),
        conditions: vec![crate::index::rules::Condition {
            pattern: "laptop".to_string(),
            anchoring: crate::index::rules::Anchoring::Contains,
            alternatives: None,
            context: None,
            filters: None,
        }],
        consequence: crate::index::rules::Consequence {
            promote: Some(vec![crate::index::rules::Promote::Single {
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
    let mut store = crate::index::rules::RuleStore::new();
    store.insert(rule);
    store.save(&rules_path).unwrap();

    let docs = vec![
        crate::types::Document::from_json(
            &json!({"_id": "1", "name": "Gaming Laptop", "popularity": 500}),
        )
        .unwrap(),
        crate::types::Document::from_json(
            &json!({"_id": "2", "name": "Office Laptop", "popularity": 300}),
        )
        .unwrap(),
        crate::types::Document::from_json(
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

// ============================================================
// From test_rules_consequences.rs
// ============================================================

#[tokio::test]
async fn test_applied_rules_in_response() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let manager = IndexManager::new(temp_dir.path());
    manager.create_tenant("test")?;

    let mut fields = HashMap::new();
    fields.insert(
        "name".to_string(),
        crate::types::FieldValue::Text("laptop".to_string()),
    );
    let doc = Document {
        id: "1".to_string(),
        fields,
    };
    manager.add_documents_sync("test", vec![doc]).await?;

    let rule = json!({
        "objectID": "test-rule",
        "conditions": [{"pattern": "laptop", "anchoring": "contains"}],
        "consequence": {"promote": [{"objectID": "1", "position": 0}]}
    });
    std::fs::write(
        temp_dir.path().join("test").join("rules.json"),
        serde_json::to_string(&vec![rule])?,
    )?;

    let result = manager.search("test", "laptop", None, None, 10)?;
    assert_eq!(result.applied_rules.len(), 1);
    assert_eq!(result.applied_rules[0], "test-rule");
    Ok(())
}

#[tokio::test]
async fn test_user_data_in_response() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let manager = IndexManager::new(temp_dir.path());
    manager.create_tenant("test")?;

    let mut fields = HashMap::new();
    fields.insert(
        "name".to_string(),
        crate::types::FieldValue::Text("laptop".to_string()),
    );
    let doc = Document {
        id: "1".to_string(),
        fields,
    };
    manager.add_documents_sync("test", vec![doc]).await?;

    let rule = json!({
        "objectID": "banner-rule",
        "conditions": [{"pattern": "laptop", "anchoring": "contains"}],
        "consequence": {"userData": {"banner": "summer-sale", "discount": 20}}
    });
    std::fs::write(
        temp_dir.path().join("test").join("rules.json"),
        serde_json::to_string(&vec![rule])?,
    )?;

    let result = manager.search("test", "laptop", None, None, 10)?;
    assert_eq!(result.user_data.len(), 1);
    assert_eq!(result.user_data[0]["banner"], "summer-sale");
    assert_eq!(result.user_data[0]["discount"], 20);
    Ok(())
}

#[tokio::test]
async fn test_query_rewrite() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let manager = IndexManager::new(temp_dir.path());
    manager.create_tenant("test")?;

    let mut f1 = HashMap::new();
    f1.insert(
        "name".to_string(),
        crate::types::FieldValue::Text("gaming laptop".to_string()),
    );
    let mut f2 = HashMap::new();
    f2.insert(
        "name".to_string(),
        crate::types::FieldValue::Text("office laptop".to_string()),
    );
    manager
        .add_documents_sync(
            "test",
            vec![
                Document {
                    id: "1".to_string(),
                    fields: f1,
                },
                Document {
                    id: "2".to_string(),
                    fields: f2,
                },
            ],
        )
        .await?;

    let rule = json!({
        "objectID": "rewrite-rule",
        "conditions": [{"pattern": "lptop", "anchoring": "is"}],
        "consequence": {"params": {"query": "laptop"}}
    });
    std::fs::write(
        temp_dir.path().join("test").join("rules.json"),
        serde_json::to_string(&vec![rule])?,
    )?;

    let result = manager.search("test", "lptop", None, None, 10)?;
    assert_eq!(result.total, 2);
    assert!(result.documents.iter().any(|d| d.document.id == "1"));
    assert!(result.documents.iter().any(|d| d.document.id == "2"));
    Ok(())
}

#[tokio::test]
async fn test_multiple_rules_user_data() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let manager = IndexManager::new(temp_dir.path());
    manager.create_tenant("test")?;

    let mut fields = HashMap::new();
    fields.insert(
        "name".to_string(),
        crate::types::FieldValue::Text("laptop".to_string()),
    );
    let doc = Document {
        id: "1".to_string(),
        fields,
    };
    manager.add_documents_sync("test", vec![doc]).await?;

    let rules = vec![
        json!({"objectID": "rule-1", "conditions": [{"pattern": "laptop", "anchoring": "contains"}], "consequence": {"userData": {"type": "banner", "id": 1}}}),
        json!({"objectID": "rule-2", "conditions": [{"pattern": "laptop", "anchoring": "contains"}], "consequence": {"userData": {"type": "discount", "id": 2}}}),
    ];
    std::fs::write(
        temp_dir.path().join("test").join("rules.json"),
        serde_json::to_string(&rules)?,
    )?;

    let result = manager.search("test", "laptop", None, None, 10)?;
    assert_eq!(result.user_data.len(), 2);
    assert_eq!(result.applied_rules.len(), 2);
    Ok(())
}
