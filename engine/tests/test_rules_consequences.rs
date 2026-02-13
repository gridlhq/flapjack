use flapjack::error::Result;
use flapjack::types::Document;
use flapjack::IndexManager;
use std::collections::HashMap;
use tempfile::TempDir;

#[tokio::test]
async fn test_applied_rules_in_response() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let manager = IndexManager::new(temp_dir.path());

    manager.create_tenant("test")?;

    let mut fields = HashMap::new();
    fields.insert(
        "name".to_string(),
        flapjack::types::FieldValue::Text("laptop".to_string()),
    );
    let doc = Document {
        id: "1".to_string(),
        fields,
    };
    manager.add_documents_sync("test", vec![doc]).await?;

    let rule = serde_json::json!({
        "objectID": "test-rule",
        "conditions": [{"pattern": "laptop", "anchoring": "contains"}],
        "consequence": {
            "promote": [{"objectID": "1", "position": 0}]
        }
    });

    let rules_path = temp_dir.path().join("test").join("rules.json");
    std::fs::write(&rules_path, serde_json::to_string(&vec![rule])?)?;

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
        flapjack::types::FieldValue::Text("laptop".to_string()),
    );
    let doc = Document {
        id: "1".to_string(),
        fields,
    };
    manager.add_documents_sync("test", vec![doc]).await?;

    let rule = serde_json::json!({
        "objectID": "banner-rule",
        "conditions": [{"pattern": "laptop", "anchoring": "contains"}],
        "consequence": {
            "userData": {"banner": "summer-sale", "discount": 20}
        }
    });

    let rules_path = temp_dir.path().join("test").join("rules.json");
    std::fs::write(&rules_path, serde_json::to_string(&vec![rule])?)?;

    let result = manager.search("test", "laptop", None, None, 10)?;

    assert_eq!(result.user_data.len(), 1);
    let user_obj = &result.user_data[0];
    assert_eq!(user_obj["banner"], "summer-sale");
    assert_eq!(user_obj["discount"], 20);

    Ok(())
}

#[tokio::test]
async fn test_query_rewrite() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let manager = IndexManager::new(temp_dir.path());

    manager.create_tenant("test")?;

    let mut fields1 = HashMap::new();
    fields1.insert(
        "name".to_string(),
        flapjack::types::FieldValue::Text("gaming laptop".to_string()),
    );
    let doc1 = Document {
        id: "1".to_string(),
        fields: fields1,
    };

    let mut fields2 = HashMap::new();
    fields2.insert(
        "name".to_string(),
        flapjack::types::FieldValue::Text("office laptop".to_string()),
    );
    let doc2 = Document {
        id: "2".to_string(),
        fields: fields2,
    };

    manager.add_documents_sync("test", vec![doc1, doc2]).await?;

    let rule = serde_json::json!({
        "objectID": "rewrite-rule",
        "conditions": [{"pattern": "lptop", "anchoring": "is"}],
        "consequence": {
            "params": {"query": "laptop"}
        }
    });

    let rules_path = temp_dir.path().join("test").join("rules.json");
    std::fs::write(&rules_path, serde_json::to_string(&vec![rule])?)?;

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
        flapjack::types::FieldValue::Text("laptop".to_string()),
    );
    let doc = Document {
        id: "1".to_string(),
        fields,
    };
    manager.add_documents_sync("test", vec![doc]).await?;

    let rules = vec![
        serde_json::json!({
            "objectID": "rule-1",
            "conditions": [{"pattern": "laptop", "anchoring": "contains"}],
            "consequence": {"userData": {"type": "banner", "id": 1}}
        }),
        serde_json::json!({
            "objectID": "rule-2",
            "conditions": [{"pattern": "laptop", "anchoring": "contains"}],
            "consequence": {"userData": {"type": "discount", "id": 2}}
        }),
    ];

    let rules_path = temp_dir.path().join("test").join("rules.json");
    std::fs::write(&rules_path, serde_json::to_string(&rules)?)?;

    let result = manager.search("test", "laptop", None, None, 10)?;

    assert_eq!(result.user_data.len(), 2);
    assert_eq!(result.applied_rules.len(), 2);

    Ok(())
}
