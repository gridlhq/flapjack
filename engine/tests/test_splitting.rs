use flapjack::types::{Document, FieldValue};
use flapjack::IndexManager;
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
                // Splitting: "hotdog" should find "hot dog"
                doc("1", vec![("name", text("hot dog stand"))]),
                doc("2", vec![("name", text("hotdog vendor"))]),
                // Concatenation: "blue tooth" should find "bluetooth"
                doc("3", vec![("name", text("bluetooth speaker"))]),
                doc("4", vec![("name", text("blue tooth fairy"))]),
                // Splitting: "notebook" -> "note book"
                doc("5", vec![("name", text("note book cover"))]),
                doc("6", vec![("name", text("notebook computer"))]),
                // Concatenation: "back pack" -> "backpack"
                doc("7", vec![("name", text("backpack bag"))]),
                doc("8", vec![("name", text("back pack strap"))]),
                // Unrelated
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

// ============================================================
// SPLITTING TESTS
// ============================================================

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
        // "hot" is 3 chars, should not be split
        let ids = search_ids(&f.mgr, "hot");
        assert!(
            ids.contains(&"1".to_string()),
            "'hot' matches 'hot dog stand'"
        );
        // Should not crash or return garbage
    }
}

// ============================================================
// CONCATENATION TESTS
// ============================================================

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

// ============================================================
// EDGE CASES
// ============================================================

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
        // "hot dog" should still work as before
        let ids = search_ids(&f.mgr, "hot dog");
        assert!(ids.contains(&"1".to_string()));
    }

    #[tokio::test]
    async fn no_false_positives() {
        let f = get_fixture().await;
        // "xylophone" should only match doc 9, splitting shouldn't produce garbage
        let ids = search_ids(&f.mgr, "xylophone");
        assert_eq!(ids.len(), 1);
        assert!(ids.contains(&"9".to_string()));
    }
}
