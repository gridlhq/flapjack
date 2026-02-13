use flapjack::error::Result;
use flapjack::index::manager::IndexManager;
use flapjack::types::{Document, FieldValue};
use std::collections::HashMap;
use tempfile::TempDir;

fn doc(id: &str, name: &str) -> Document {
    let mut fields = HashMap::new();
    fields.insert("name".to_string(), FieldValue::Text(name.to_string()));
    Document {
        id: id.to_string(),
        fields,
    }
}

async fn setup() -> Result<(TempDir, std::sync::Arc<IndexManager>)> {
    let temp_dir = TempDir::new()?;
    let manager = IndexManager::new(temp_dir.path());
    manager.create_tenant("test")?;
    manager
        .add_documents_sync(
            "test",
            vec![
                doc("1", "O'Kelly"),
                doc("2", "D'Agostino"),
                doc("3", "Abdel-Rahman"),
                doc("4", "mens-watches"),
                doc("5", "Jean-Pierre"),
                doc("6", "plain kelly"),
                doc("7", "Al Hassan"),
            ],
        )
        .await?;
    Ok((temp_dir, manager))
}

fn search(manager: &IndexManager, query: &str) -> Result<Vec<String>> {
    let result = manager.search("test", query, None, None, 10)?;
    Ok(result
        .documents
        .iter()
        .map(|h| {
            h.document
                .fields
                .get("name")
                .and_then(|v| {
                    if let FieldValue::Text(t) = v {
                        Some(t.clone())
                    } else {
                        None
                    }
                })
                .unwrap_or_default()
        })
        .collect())
}

#[tokio::test]
async fn test_punctuation_split_search() -> Result<()> {
    let (_tmp, mgr) = setup().await?;

    let hits = search(&mgr, "o'kelly")?;
    assert!(
        hits.contains(&"O'Kelly".to_string()),
        "o'kelly → {:?}",
        hits
    );

    let hits = search(&mgr, "o kelly")?;
    assert!(
        hits.contains(&"O'Kelly".to_string()),
        "o kelly → {:?}",
        hits
    );

    let hits = search(&mgr, "abdel-rahman")?;
    assert!(
        hits.contains(&"Abdel-Rahman".to_string()),
        "abdel-rahman → {:?}",
        hits
    );

    let hits = search(&mgr, "abdel rahman")?;
    assert!(
        hits.contains(&"Abdel-Rahman".to_string()),
        "abdel rahman → {:?}",
        hits
    );

    Ok(())
}

#[tokio::test]
async fn test_concat_search() -> Result<()> {
    let (_tmp, mgr) = setup().await?;

    let cases = vec![
        ("okelly", "O'Kelly"),
        ("dagostino", "D'Agostino"),
        ("abdelrahman", "Abdel-Rahman"),
        ("menswatches", "mens-watches"),
        ("jeanpierre", "Jean-Pierre"),
    ];

    for (query, expected) in &cases {
        let hits = search(&mgr, query)?;
        assert!(
            hits.contains(&expected.to_string()),
            "query='{}' expected='{}' got={:?}",
            query,
            expected,
            hits
        );
    }

    Ok(())
}

#[tokio::test]
async fn test_concat_prefix_search() -> Result<()> {
    let (_tmp, mgr) = setup().await?;

    let hits = search(&mgr, "abdelr")?;
    assert!(
        hits.contains(&"Abdel-Rahman".to_string()),
        "abdelr prefix → {:?}",
        hits
    );

    let hits = search(&mgr, "mensw")?;
    assert!(
        hits.contains(&"mens-watches".to_string()),
        "mensw prefix → {:?}",
        hits
    );

    let hits = search(&mgr, "jeanp")?;
    assert!(
        hits.contains(&"Jean-Pierre".to_string()),
        "jeanp prefix → {:?}",
        hits
    );

    Ok(())
}
