use flapjack::error::Result;
use flapjack::index::manager::IndexManager;
use flapjack::types::{Document, FieldValue};
use std::collections::HashMap;
use tempfile::TempDir;

fn doc(id: &str, fields: Vec<(&str, &str)>) -> Document {
    let mut f = HashMap::new();
    for (k, v) in fields {
        f.insert(k.to_string(), FieldValue::Text(v.to_string()));
    }
    Document {
        id: id.to_string(),
        fields: f,
    }
}

async fn setup_test_index() -> Result<(TempDir, std::sync::Arc<IndexManager>)> {
    let temp_dir = TempDir::new()?;
    let manager = IndexManager::new(temp_dir.path());
    manager.create_tenant("test")?;

    let docs = vec![
        doc(
            "1",
            vec![("title", "Apple Watch"), ("category", "electronics")],
        ),
        doc(
            "2",
            vec![("title", "Apple iPhone"), ("category", "electronics")],
        ),
        doc(
            "3",
            vec![
                ("title", "Fresh Apples"),
                ("description", "Delicious red apples"),
            ],
        ),
        doc(
            "4",
            vec![("title", "Washing Machine"), ("category", "appliances")],
        ),
        doc("5", vec![("title", "Water Bottle"), ("category", "sports")]),
        doc(
            "6",
            vec![("title", "Mens Watch"), ("category", "mens-watches")],
        ),
    ];

    manager.add_documents_sync("test", docs).await?;
    Ok((temp_dir, manager))
}

#[tokio::test]
async fn test_prefix_last_multi_word() -> Result<()> {
    let (_temp, manager) = setup_test_index().await?;

    let result = manager.search("test", "apple wat", None, None, 10)?;
    assert_eq!(
        result.total, 1,
        "apple wat should match Apple Watch (wat is prefix)"
    );

    let result = manager.search("test", "apple watch", None, None, 10)?;
    assert_eq!(result.total, 1, "apple watch should match Apple Watch");

    Ok(())
}

#[tokio::test]
async fn test_trailing_space_exact_match() -> Result<()> {
    let (_temp, manager) = setup_test_index().await?;

    let result = manager.search("test", "wa", None, None, 10)?;
    assert!(
        result.total >= 2,
        "wa should match Watch, Water, Washing via prefix"
    );

    let result = manager.search("test", "wa ", None, None, 10)?;
    assert_eq!(
        result.total, 0,
        "wa (with space) should require exact word match"
    );

    let result = manager.search("test", "apple ", None, None, 10)?;
    assert!(
        result.total >= 1,
        "apple (with space) should match docs with exact word apple"
    );

    Ok(())
}

#[tokio::test]
async fn test_fuzzy_with_trailing_space() -> Result<()> {
    let (_temp, manager) = setup_test_index().await?;

    let result = manager.search("test", "appel ", None, None, 10)?;
    assert!(
        result.total >= 1,
        "appel (typo with space) should fuzzy match apple"
    );

    Ok(())
}

#[tokio::test]
async fn test_hyphen_splitting() -> Result<()> {
    let (_temp, manager) = setup_test_index().await?;

    let result = manager.search("test", "watches", None, None, 10)?;
    assert!(
        result.total >= 1,
        "watches should match mens-watches via hyphen split"
    );

    let result = manager.search("test", "mens", None, None, 10)?;
    assert!(
        result.total >= 1,
        "mens should match mens-watches via hyphen split"
    );

    Ok(())
}

#[tokio::test]
async fn test_non_last_token_exact() -> Result<()> {
    let (_temp, manager) = setup_test_index().await?;

    let result = manager.search("test", "apple wat feat", None, None, 10)?;
    assert_eq!(
        result.total, 0,
        "apple wat feat - wat is not last, needs exact match, should fail"
    );

    Ok(())
}
#[tokio::test]
async fn test_query_type_setting_respected() -> Result<()> {
    let (temp, manager) = setup_test_index().await?;

    let settings_path = temp.path().join("test").join("settings.json");
    std::fs::write(&settings_path, r#"{"queryType":"prefixAll"}"#)?;
    manager.invalidate_settings_cache("test");

    let result = manager.search("test", "apple wat", None, None, 10)?;
    assert!(
        result.total >= 1,
        "prefixAll: 'apple wat' should match Apple Watch (both prefix)"
    );

    std::fs::write(&settings_path, r#"{"queryType":"prefixNone"}"#)?;
    manager.invalidate_settings_cache("test");

    let result = manager.search("test", "apple wat", None, None, 10)?;
    assert_eq!(
        result.total, 0,
        "prefixNone: 'wat' should NOT prefix-match 'watch'"
    );

    let result = manager.search("test", "apple watch", None, None, 10)?;
    assert_eq!(
        result.total, 1,
        "prefixNone: exact 'watch' should still match"
    );

    std::fs::write(&settings_path, r#"{"queryType":"prefixLast"}"#)?;
    manager.invalidate_settings_cache("test");

    let result = manager.search("test", "apple wat", None, None, 10)?;
    assert_eq!(
        result.total, 1,
        "prefixLast: last token 'wat' should prefix-match 'watch'"
    );

    let result = manager.search("test", "apple wat feat", None, None, 10)?;
    assert_eq!(
        result.total, 0,
        "prefixLast: non-last 'wat' should NOT prefix-match"
    );

    Ok(())
}
#[tokio::test]
async fn test_multi_word_no_settings() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let manager = IndexManager::new(temp_dir.path());
    manager.create_tenant("test")?;

    // NO settings - just upload docs like validation test does
    let docs = vec![
        doc(
            "1",
            vec![
                ("name", "Essence Mascara Lash Princess"),
                ("brand", "Essence"),
            ],
        ),
        doc("2", vec![("name", "Red Lipstick"), ("brand", "Glamour")]),
    ];

    manager.add_documents_sync("test", docs).await?;

    let result = manager.search("test", "essence mascara", None, None, 10)?;
    assert!(
        result.total >= 1,
        "essence mascara should match doc with both words"
    );

    Ok(())
}
