//! Tokenizer integration tests
//!
//! Combines:
//! - CjkAwareTokenizer unit behavior (from tokenizer_test.rs)
//! - Punctuation splitting + concat search (from concat_search_test.rs)
//! - Prefix-last query semantics (from prefix_last_test.rs)

mod tokenizer_unit {
    // Most basic tokenizer tests removed — covered by inline tests in tokenizer/cjk_tokenizer.rs.
    // Only the null-byte boundary test is kept (unique edge case not covered inline).

    use crate::tokenizer::CjkAwareTokenizer;
    use tantivy::tokenizer::{TokenStream, Tokenizer};

    fn tokenize(text: &str) -> Vec<String> {
        let mut tok = CjkAwareTokenizer;
        let mut stream = tok.token_stream(text);
        let mut tokens = Vec::new();
        while stream.advance() {
            tokens.push(stream.token().text.clone());
        }
        tokens
    }

    #[test]
    fn test_json_path_not_affected() {
        let tokens = tokenize("name\0sGaming Laptop");
        assert!(
            tokens.contains(&"name".to_string()),
            "should have path 'name': {:?}",
            tokens
        );
        assert!(
            tokens.contains(&"sGaming".to_string()),
            "should have 'sGaming': {:?}",
            tokens
        );
        assert!(
            tokens.contains(&"Laptop".to_string()),
            "should have 'Laptop': {:?}",
            tokens
        );
        let concat: Vec<_> = tokens
            .iter()
            .filter(|t| t.contains("name") && t.len() > 4)
            .collect();
        assert!(
            concat.is_empty(),
            "should NOT concat across \\0 boundary: {:?}",
            tokens
        );
    }
}

mod concat_search {
    use crate::error::Result;
    use crate::index::manager::IndexManager;
    use crate::types::{Document, FieldValue};
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
}

mod prefix_last {
    use crate::error::Result;
    use crate::index::manager::IndexManager;
    use crate::types::{Document, FieldValue};
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
}

// ─── Splitting/concatenation integration tests (from test_splitting.rs) ──────

mod splitting_integration {
    use crate::types::{Document, FieldValue};
    use crate::IndexManager;
    use std::collections::HashMap;
    use std::sync::Arc;
    use tempfile::TempDir;

    fn doc(id: &str, fields: Vec<(&str, FieldValue)>) -> Document {
        let f: HashMap<String, FieldValue> = fields
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect();
        Document {
            id: id.to_string(),
            fields: f,
        }
    }

    fn text(s: &str) -> FieldValue {
        FieldValue::Text(s.to_string())
    }

    struct SplittingFixture {
        _tmp: TempDir,
        mgr: Arc<IndexManager>,
    }

    static FIXTURE: tokio::sync::OnceCell<SplittingFixture> = tokio::sync::OnceCell::const_new();

    async fn get_fixture() -> &'static SplittingFixture {
        FIXTURE
            .get_or_init(|| async {
                let temp_dir = TempDir::new().unwrap();
                let manager = IndexManager::new(temp_dir.path());
                manager.create_tenant("test").unwrap();

                let docs = vec![
                    doc("1", vec![("name", text("hot dog stand"))]),
                    doc("2", vec![("name", text("hotdog vendor"))]),
                    doc("3", vec![("name", text("bluetooth speaker"))]),
                    doc("4", vec![("name", text("blue tooth fairy"))]),
                    doc("5", vec![("name", text("note book cover"))]),
                    doc("6", vec![("name", text("notebook computer"))]),
                    doc("7", vec![("name", text("backpack bag"))]),
                    doc("8", vec![("name", text("back pack strap"))]),
                    doc("9", vec![("name", text("xylophone player"))]),
                ];
                manager.add_documents_sync("test", docs).await.unwrap();

                SplittingFixture {
                    _tmp: temp_dir,
                    mgr: manager,
                }
            })
            .await
    }

    fn search_ids(mgr: &IndexManager, query: &str) -> Vec<String> {
        mgr.search("test", query, None, None, 20)
            .unwrap()
            .documents
            .iter()
            .map(|d| d.document.id.clone())
            .collect()
    }

    fn search_count(mgr: &IndexManager, query: &str) -> usize {
        mgr.search("test", query, None, None, 20).unwrap().total
    }

    mod splitting {
        use super::*;

        #[tokio::test]
        async fn hotdog_finds_hot_dog() {
            let f = get_fixture().await;
            let ids = search_ids(&f.mgr, "hotdog");
            assert!(
                ids.contains(&"1".to_string()),
                "'hotdog' should find 'hot dog stand' (doc 1)"
            );
            assert!(
                ids.contains(&"2".to_string()),
                "'hotdog' should still find 'hotdog vendor' (doc 2)"
            );
        }

        #[tokio::test]
        async fn notebook_finds_note_book() {
            let f = get_fixture().await;
            let ids = search_ids(&f.mgr, "notebook");
            assert!(
                ids.contains(&"5".to_string()),
                "'notebook' should find 'note book cover' (doc 5)"
            );
            assert!(
                ids.contains(&"6".to_string()),
                "'notebook' should find 'notebook computer' (doc 6)"
            );
        }

        #[tokio::test]
        async fn short_tokens_not_split() {
            let f = get_fixture().await;
            let ids = search_ids(&f.mgr, "hot");
            assert!(
                ids.contains(&"1".to_string()),
                "'hot' matches 'hot dog stand'"
            );
        }
    }

    mod concatenation {
        use super::*;

        #[tokio::test]
        async fn blue_tooth_finds_bluetooth() {
            let f = get_fixture().await;
            let ids = search_ids(&f.mgr, "blue tooth");
            assert!(
                ids.contains(&"3".to_string()),
                "'blue tooth' should find 'bluetooth speaker' (doc 3)"
            );
            assert!(
                ids.contains(&"4".to_string()),
                "'blue tooth' should find 'blue tooth fairy' (doc 4)"
            );
        }

        #[tokio::test]
        async fn back_pack_finds_backpack() {
            let f = get_fixture().await;
            let ids = search_ids(&f.mgr, "back pack");
            assert!(
                ids.contains(&"7".to_string()),
                "'back pack' should find 'backpack bag' (doc 7)"
            );
            assert!(
                ids.contains(&"8".to_string()),
                "'back pack' should find 'back pack strap' (doc 8)"
            );
        }
    }

    mod edge_cases {
        use super::*;

        #[tokio::test]
        async fn empty_query_unchanged() {
            let f = get_fixture().await;
            let total = search_count(&f.mgr, "");
            assert_eq!(total, 9, "empty query should return all docs");
        }

        #[tokio::test]
        async fn original_results_preserved() {
            let f = get_fixture().await;
            let ids = search_ids(&f.mgr, "hot dog");
            assert!(ids.contains(&"1".to_string()));
        }

        #[tokio::test]
        async fn no_false_positives() {
            let f = get_fixture().await;
            let ids = search_ids(&f.mgr, "xylophone");
            assert_eq!(ids.len(), 1);
            assert!(ids.contains(&"9".to_string()));
        }
    }
}
