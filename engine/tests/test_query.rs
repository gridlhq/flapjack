use flapjack::index::settings::IndexSettings;
use flapjack::index::synonyms::{Synonym, SynonymStore};
use flapjack::query::plurals::IgnorePluralsValue;
use flapjack::query::stopwords::RemoveStopWordsValue;
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

// ============================================================
// Shared fixtures
// ============================================================

struct PluralFixture {
    _tmp: TempDir,
    mgr: Arc<IndexManager>,
}

static PLURAL_FIXTURE: tokio::sync::OnceCell<PluralFixture> = tokio::sync::OnceCell::const_new();

async fn get_plural_fixture() -> &'static PluralFixture {
    PLURAL_FIXTURE
        .get_or_init(|| async {
            let temp_dir = TempDir::new().unwrap();
            let manager = IndexManager::new(temp_dir.path());
            manager.create_tenant("test").unwrap();

            let settings = IndexSettings {
                ignore_plurals: IgnorePluralsValue::All,
                query_languages: vec!["en".to_string()],
                ..Default::default()
            };
            settings
                .save(temp_dir.path().join("test/settings.json"))
                .unwrap();
            manager.invalidate_settings_cache("test");

            let docs = vec![
                doc("1", vec![("name", text("car"))]),
                doc("2", vec![("name", text("cars"))]),
                doc("3", vec![("name", text("child"))]),
                doc("4", vec![("name", text("children"))]),
                doc("5", vec![("name", text("battery"))]),
                doc("6", vec![("name", text("batteries"))]),
                doc("7", vec![("name", text("church"))]),
                doc("8", vec![("name", text("churches"))]),
                doc("9", vec![("name", text("knife"))]),
                doc("10", vec![("name", text("knives"))]),
                doc("11", vec![("name", text("person"))]),
                doc("12", vec![("name", text("people"))]),
            ];
            manager.add_documents_sync("test", docs).await.unwrap();

            PluralFixture {
                _tmp: temp_dir,
                mgr: manager,
            }
        })
        .await
}

struct StopwordFixture {
    _tmp: TempDir,
    mgr: Arc<IndexManager>,
}

static STOPWORD_ENABLED_FIXTURE: tokio::sync::OnceCell<StopwordFixture> =
    tokio::sync::OnceCell::const_new();
static STOPWORD_DISABLED_FIXTURE: tokio::sync::OnceCell<StopwordFixture> =
    tokio::sync::OnceCell::const_new();
static STOPWORD_LANG_EN_FIXTURE: tokio::sync::OnceCell<StopwordFixture> =
    tokio::sync::OnceCell::const_new();
static STOPWORD_LANG_XX_FIXTURE: tokio::sync::OnceCell<StopwordFixture> =
    tokio::sync::OnceCell::const_new();

async fn make_stopword_fixture(rsw: RemoveStopWordsValue) -> StopwordFixture {
    let temp_dir = TempDir::new().unwrap();
    let manager = IndexManager::new(temp_dir.path());
    manager.create_tenant("test").unwrap();

    let settings = IndexSettings {
        remove_stop_words: rsw,
        ..IndexSettings::default()
    };
    settings
        .save(temp_dir.path().join("test/settings.json"))
        .unwrap();
    manager.invalidate_settings_cache("test");

    let docs = vec![
        doc("1", vec![("title", text("best search engine"))]),
        doc("2", vec![("title", text("the best search tool"))]),
        doc("3", vec![("title", text("how to build a search engine"))]),
        doc("4", vec![("title", text("search and discover"))]),
        doc("5", vec![("title", text("is this a test"))]),
    ];
    manager.add_documents_sync("test", docs).await.unwrap();

    StopwordFixture {
        _tmp: temp_dir,
        mgr: manager,
    }
}

async fn get_stopword_enabled() -> &'static StopwordFixture {
    STOPWORD_ENABLED_FIXTURE
        .get_or_init(|| make_stopword_fixture(RemoveStopWordsValue::All))
        .await
}

async fn get_stopword_disabled() -> &'static StopwordFixture {
    STOPWORD_DISABLED_FIXTURE
        .get_or_init(|| make_stopword_fixture(RemoveStopWordsValue::Disabled))
        .await
}

async fn get_stopword_lang_en() -> &'static StopwordFixture {
    STOPWORD_LANG_EN_FIXTURE
        .get_or_init(|| {
            make_stopword_fixture(RemoveStopWordsValue::Languages(vec!["en".to_string()]))
        })
        .await
}

async fn get_stopword_lang_xx() -> &'static StopwordFixture {
    STOPWORD_LANG_XX_FIXTURE
        .get_or_init(|| {
            make_stopword_fixture(RemoveStopWordsValue::Languages(vec!["xx".to_string()]))
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

// ============================================================
// PLURAL TESTS (shared fixture)
// ============================================================

mod plurals {
    use super::*;

    #[tokio::test]
    async fn car_finds_cars() {
        let f = get_plural_fixture().await;
        let ids = search_ids(&f.mgr, "car");
        assert!(ids.contains(&"1".to_string()));
        assert!(ids.contains(&"2".to_string()));
    }

    #[tokio::test]
    async fn cars_finds_car() {
        let f = get_plural_fixture().await;
        let ids = search_ids(&f.mgr, "cars");
        assert!(ids.contains(&"1".to_string()));
        assert!(ids.contains(&"2".to_string()));
    }

    #[tokio::test]
    async fn child_finds_children() {
        let f = get_plural_fixture().await;
        let ids = search_ids(&f.mgr, "child");
        assert!(ids.contains(&"3".to_string()));
        assert!(ids.contains(&"4".to_string()));
    }

    #[tokio::test]
    async fn children_finds_child() {
        let f = get_plural_fixture().await;
        let ids = search_ids(&f.mgr, "children");
        assert!(ids.contains(&"3".to_string()));
        assert!(ids.contains(&"4".to_string()));
    }

    #[tokio::test]
    async fn battery_finds_batteries() {
        let f = get_plural_fixture().await;
        let ids = search_ids(&f.mgr, "battery");
        assert!(ids.contains(&"5".to_string()));
        assert!(ids.contains(&"6".to_string()));
    }

    #[tokio::test]
    async fn person_finds_people() {
        let f = get_plural_fixture().await;
        let ids = search_ids(&f.mgr, "person");
        assert!(ids.contains(&"11".to_string()));
        assert!(ids.contains(&"12".to_string()));
    }

    #[tokio::test]
    async fn knife_finds_knives() {
        let f = get_plural_fixture().await;
        let ids = search_ids(&f.mgr, "knife");
        assert!(ids.contains(&"9".to_string()));
        assert!(ids.contains(&"10".to_string()));
    }

    #[tokio::test]
    async fn disabled_no_expansion() {
        let temp_dir = TempDir::new().unwrap();
        let manager = IndexManager::new(temp_dir.path());
        manager.create_tenant("test").unwrap();

        let settings = IndexSettings {
            ignore_plurals: IgnorePluralsValue::Disabled,
            ..Default::default()
        };
        settings
            .save(temp_dir.path().join("test/settings.json"))
            .unwrap();

        let docs = vec![
            doc("1", vec![("name", text("child"))]),
            doc("2", vec![("name", text("children"))]),
        ];
        manager.add_documents_sync("test", docs).await.unwrap();

        let result = manager.search("test", "child ", None, None, 10).unwrap();
        let ids: Vec<&str> = result
            .documents
            .iter()
            .map(|d| d.document.id.as_str())
            .collect();
        assert!(ids.contains(&"1"));
        assert!(!ids.contains(&"2"));
    }

    #[tokio::test]
    async fn per_query_override() {
        let temp_dir = TempDir::new().unwrap();
        let manager = IndexManager::new(temp_dir.path());
        manager.create_tenant("test").unwrap();

        let settings = IndexSettings::default();
        settings
            .save(temp_dir.path().join("test/settings.json"))
            .unwrap();

        let docs = vec![
            doc("1", vec![("name", text("child"))]),
            doc("2", vec![("name", text("children"))]),
        ];
        manager.add_documents_sync("test", docs).await.unwrap();

        let ip = IgnorePluralsValue::All;
        let ql = vec!["en".to_string()];
        let result = manager
            .search_full_with_stop_words(
                "test",
                "child",
                None,
                None,
                10,
                0,
                None,
                None,
                None,
                None,
                Some(&ip),
                Some(&ql),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            )
            .unwrap();
        let ids: Vec<&str> = result
            .documents
            .iter()
            .map(|d| d.document.id.as_str())
            .collect();
        assert!(ids.contains(&"1"));
        assert!(ids.contains(&"2"));
    }

    #[tokio::test]
    async fn query_languages_wiring() {
        let temp_dir = TempDir::new().unwrap();
        let manager = IndexManager::new(temp_dir.path());
        manager.create_tenant("test").unwrap();

        let settings = IndexSettings {
            ignore_plurals: IgnorePluralsValue::All,
            query_languages: vec!["xx".to_string()],
            ..Default::default()
        };
        settings
            .save(temp_dir.path().join("test/settings.json"))
            .unwrap();
        manager.invalidate_settings_cache("test");

        let docs = vec![
            doc("1", vec![("name", text("child"))]),
            doc("2", vec![("name", text("children"))]),
        ];
        manager.add_documents_sync("test", docs).await.unwrap();

        let result = manager.search("test", "child ", None, None, 10).unwrap();
        let ids: Vec<&str> = result
            .documents
            .iter()
            .map(|d| d.document.id.as_str())
            .collect();
        assert!(ids.contains(&"1"));
        assert!(!ids.contains(&"2"));
    }

    #[tokio::test]
    async fn serde_roundtrip_settings() {
        let settings = IndexSettings {
            ignore_plurals: IgnorePluralsValue::Languages(vec!["en".to_string(), "fr".to_string()]),
            query_languages: vec!["en".to_string()],
            ..Default::default()
        };
        let json = serde_json::to_string_pretty(&settings).unwrap();
        let loaded: IndexSettings = serde_json::from_str(&json).unwrap();
        assert_eq!(
            loaded.ignore_plurals,
            IgnorePluralsValue::Languages(vec!["en".to_string(), "fr".to_string()])
        );
        assert_eq!(loaded.query_languages, vec!["en".to_string()]);
    }

    #[tokio::test]
    async fn settings_default_false() {
        let json = r#"{"queryType":"prefixLast"}"#;
        let settings: IndexSettings = serde_json::from_str(json).unwrap();
        assert_eq!(settings.ignore_plurals, IgnorePluralsValue::Disabled);
        assert!(settings.query_languages.is_empty());
    }
}

// ============================================================
// STOPWORD TESTS
// ============================================================

mod stopwords {
    use super::*;

    #[tokio::test]
    async fn disabled_matches_all_words() {
        let f = get_stopword_disabled().await;
        let result = f.mgr.search("test", "the best", None, None, 10).unwrap();
        let ids: Vec<&str> = result
            .documents
            .iter()
            .map(|d| d.document.id.as_str())
            .collect();
        assert!(
            ids.contains(&"2"),
            "should match 'the best search tool' when stop words disabled"
        );

        let result2 = f.mgr.search("test", "the", None, None, 10).unwrap();
        assert!(
            result2.total > 0,
            "'the' should match docs when stop words disabled"
        );
    }

    #[tokio::test]
    async fn enabled_strips_common_words() {
        let f = get_stopword_enabled().await;
        let with_stop = f
            .mgr
            .search("test", "the best search", None, None, 10)
            .unwrap();
        let without_stop = f.mgr.search("test", "best search", None, None, 10).unwrap();
        assert_eq!(
            with_stop
                .documents
                .iter()
                .map(|d| d.document.id.as_str())
                .collect::<Vec<_>>(),
            without_stop
                .documents
                .iter()
                .map(|d| d.document.id.as_str())
                .collect::<Vec<_>>(),
            "removing 'the' should produce same results"
        );
    }

    #[tokio::test]
    async fn all_stop_words_query_not_emptied() {
        let f = get_stopword_enabled().await;
        let result = f.mgr.search("test", "the a an", None, None, 10).unwrap();
        assert!(
            result.total > 0,
            "all-stop-word query should not be emptied"
        );
    }

    #[tokio::test]
    async fn prefix_last_preserves_last_word() {
        let f = get_stopword_enabled().await;
        let result = f.mgr.search("test", "how to the", None, None, 10).unwrap();
        assert!(
            result.total > 0,
            "last word 'the' should be preserved as prefix in prefixLast mode"
        );
    }

    #[tokio::test]
    async fn language_specific_en() {
        let f = get_stopword_lang_en().await;
        let with_stop = f
            .mgr
            .search("test", "the search engine", None, None, 10)
            .unwrap();
        let without_stop = f
            .mgr
            .search("test", "search engine", None, None, 10)
            .unwrap();
        assert_eq!(
            with_stop
                .documents
                .iter()
                .map(|d| d.document.id.as_str())
                .collect::<Vec<_>>(),
            without_stop
                .documents
                .iter()
                .map(|d| d.document.id.as_str())
                .collect::<Vec<_>>(),
            "en stop words should strip 'the'"
        );
    }

    #[tokio::test]
    async fn unsupported_language_noop() {
        let f = get_stopword_lang_xx().await;
        let result = f.mgr.search("test", "the best", None, None, 10).unwrap();
        let ids: Vec<&str> = result
            .documents
            .iter()
            .map(|d| d.document.id.as_str())
            .collect();
        assert!(
            ids.contains(&"2"),
            "unsupported lang should not strip any words"
        );
    }

    #[tokio::test]
    async fn does_not_affect_content_words() {
        let f = get_stopword_enabled().await;
        let result = f
            .mgr
            .search("test", "search engine", None, None, 10)
            .unwrap();
        let ids: Vec<&str> = result
            .documents
            .iter()
            .map(|d| d.document.id.as_str())
            .collect();
        assert!(ids.contains(&"1"));
        assert!(ids.contains(&"3"));
    }

    #[tokio::test]
    async fn empty_query_unchanged() {
        let f = get_stopword_enabled().await;
        let result = f.mgr.search("test", "", None, None, 10).unwrap();
        assert_eq!(result.total, 5);
    }

    #[tokio::test]
    async fn prefix_none_strips_all_stop_words() {
        let temp_dir = TempDir::new().unwrap();
        let manager = IndexManager::new(temp_dir.path());
        manager.create_tenant("test").unwrap();

        let settings = IndexSettings {
            remove_stop_words: RemoveStopWordsValue::All,
            query_type: "prefixNone".to_string(),
            ..IndexSettings::default()
        };
        settings
            .save(temp_dir.path().join("test/settings.json"))
            .unwrap();
        manager.invalidate_settings_cache("test");

        let docs = vec![
            doc("1", vec![("title", text("best search engine"))]),
            doc("2", vec![("title", text("the best search tool"))]),
        ];
        manager.add_documents_sync("test", docs).await.unwrap();

        let with_the = manager
            .search("test", "the best search", None, None, 10)
            .unwrap();
        let without_the = manager
            .search("test", "best search", None, None, 10)
            .unwrap();
        assert_eq!(
            with_the
                .documents
                .iter()
                .map(|d| d.document.id.as_str())
                .collect::<Vec<_>>(),
            without_the
                .documents
                .iter()
                .map(|d| d.document.id.as_str())
                .collect::<Vec<_>>(),
        );
    }

    #[tokio::test]
    async fn per_query_override() {
        let temp_dir = TempDir::new().unwrap();
        let manager = IndexManager::new(temp_dir.path());
        manager.create_tenant("test").unwrap();

        let docs = vec![
            doc("1", vec![("title", text("best search engine"))]),
            doc("2", vec![("title", text("the best search tool"))]),
        ];
        manager.add_documents_sync("test", docs).await.unwrap();

        let enabled = RemoveStopWordsValue::All;
        let with_override = manager
            .search_full_with_stop_words(
                "test",
                "the best search",
                None,
                None,
                10,
                0,
                None,
                None,
                None,
                Some(&enabled),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            )
            .unwrap();
        let without_override = manager
            .search_full_with_stop_words(
                "test",
                "best search",
                None,
                None,
                10,
                0,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            )
            .unwrap();
        assert_eq!(
            with_override
                .documents
                .iter()
                .map(|d| d.document.id.as_str())
                .collect::<Vec<_>>(),
            without_override
                .documents
                .iter()
                .map(|d| d.document.id.as_str())
                .collect::<Vec<_>>(),
        );
    }

    #[tokio::test]
    async fn per_query_override_trumps_setting() {
        let temp_dir = TempDir::new().unwrap();
        let manager = IndexManager::new(temp_dir.path());
        manager.create_tenant("test").unwrap();

        let settings = IndexSettings {
            remove_stop_words: RemoveStopWordsValue::All,
            ..IndexSettings::default()
        };
        settings
            .save(temp_dir.path().join("test/settings.json"))
            .unwrap();
        manager.invalidate_settings_cache("test");

        let docs = vec![doc("1", vec![("title", text("the best search engine"))])];
        manager.add_documents_sync("test", docs).await.unwrap();

        let disabled = RemoveStopWordsValue::Disabled;
        let result = manager
            .search_full_with_stop_words(
                "test",
                "the",
                None,
                None,
                10,
                0,
                None,
                None,
                None,
                Some(&disabled),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            )
            .unwrap();
        assert!(
            result.total > 0,
            "per-query disabled should override setting enabled"
        );
    }

    #[tokio::test]
    async fn setting_serialization_roundtrip() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("settings.json");

        let mut settings = IndexSettings {
            remove_stop_words: RemoveStopWordsValue::All,
            ..IndexSettings::default()
        };
        settings.save(&path).unwrap();
        let loaded = IndexSettings::load(&path).unwrap();
        assert_eq!(loaded.remove_stop_words, RemoveStopWordsValue::All);

        settings.remove_stop_words =
            RemoveStopWordsValue::Languages(vec!["en".to_string(), "fr".to_string()]);
        settings.save(&path).unwrap();
        let loaded2 = IndexSettings::load(&path).unwrap();
        assert_eq!(
            loaded2.remove_stop_words,
            RemoveStopWordsValue::Languages(vec!["en".to_string(), "fr".to_string()])
        );
    }

    #[tokio::test]
    async fn existing_settings_without_field_defaults_to_false() {
        let json = r#"{"queryType":"prefixAll"}"#;
        let settings: IndexSettings = serde_json::from_str(json).unwrap();
        assert_eq!(settings.remove_stop_words, RemoveStopWordsValue::Disabled);
    }

    #[test]
    fn search_request_deserialization() {
        let json = r#"{"query":"the best","removeStopWords":true}"#;
        let req: flapjack_http::dto::SearchRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.remove_stop_words, Some(RemoveStopWordsValue::All));

        let json2 = r#"{"query":"the best","removeStopWords":["en","fr"]}"#;
        let req2: flapjack_http::dto::SearchRequest = serde_json::from_str(json2).unwrap();
        assert_eq!(
            req2.remove_stop_words,
            Some(RemoveStopWordsValue::Languages(vec![
                "en".to_string(),
                "fr".to_string()
            ]))
        );

        let json3 = r#"{"query":"the best"}"#;
        let req3: flapjack_http::dto::SearchRequest = serde_json::from_str(json3).unwrap();
        assert_eq!(req3.remove_stop_words, None);
    }

    #[test]
    fn params_string_parsing() {
        let mut req: flapjack_http::dto::SearchRequest =
            serde_json::from_str(r#"{"query":"test","params":"removeStopWords=true"}"#).unwrap();
        req.apply_params_string();
        assert_eq!(req.remove_stop_words, Some(RemoveStopWordsValue::All));

        let mut req2: flapjack_http::dto::SearchRequest =
            serde_json::from_str(r#"{"query":"test","params":"removeStopWords=%5B%22en%22%5D"}"#)
                .unwrap();
        req2.apply_params_string();
        assert_eq!(
            req2.remove_stop_words,
            Some(RemoveStopWordsValue::Languages(vec!["en".to_string()]))
        );
    }
}

// ============================================================
// SYNONYM TESTS (pure unit, no IndexManager)
// ============================================================

mod synonyms {
    use super::*;

    #[test]
    fn regular_synonym() {
        let syn = Synonym::Regular {
            object_id: "pants-trousers".to_string(),
            synonyms: vec!["pants".to_string(), "trousers".to_string()],
        };
        assert_eq!(syn.object_id(), "pants-trousers");
        assert_eq!(syn.synonym_type(), "synonym");
        assert!(syn.matches_text("pants"));
        assert!(syn.matches_text("trousers"));
        assert!(!syn.matches_text("shoes"));
    }

    #[test]
    fn oneway_synonym() {
        let syn = Synonym::OneWay {
            object_id: "tablet-ipad".to_string(),
            input: "tablet".to_string(),
            synonyms: vec!["ipad".to_string(), "galaxy tab".to_string()],
        };
        assert_eq!(syn.synonym_type(), "onewaysynonym");
        assert!(syn.matches_text("tablet"));
        assert!(syn.matches_text("ipad"));
    }

    #[test]
    fn altcorrection1() {
        let syn = Synonym::AltCorrection1 {
            object_id: "trousers-pants".to_string(),
            word: "trousers".to_string(),
            corrections: vec!["pants".to_string()],
        };
        assert_eq!(syn.synonym_type(), "altcorrection1");
        assert!(syn.matches_text("trousers"));
        assert!(syn.matches_text("pants"));
    }

    #[test]
    fn placeholder() {
        let syn = Synonym::Placeholder {
            object_id: "street".to_string(),
            placeholder: "<street>".to_string(),
            replacements: vec!["street".to_string(), "st".to_string()],
        };
        assert_eq!(syn.synonym_type(), "placeholder");
        assert!(syn.matches_text("street"));
        assert!(syn.matches_text("st"));
    }

    #[test]
    fn store_save_load() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("synonyms.json");

        let mut store = SynonymStore::new();
        store.insert(Synonym::Regular {
            object_id: "pants-trousers".to_string(),
            synonyms: vec!["pants".to_string(), "trousers".to_string()],
        });
        store.save(&path).unwrap();

        let loaded = SynonymStore::load(&path).unwrap();
        assert!(loaded.get("pants-trousers").is_some());
    }

    #[test]
    fn store_search() {
        let mut store = SynonymStore::new();
        store.insert(Synonym::Regular {
            object_id: "pants-trousers".to_string(),
            synonyms: vec!["pants".to_string(), "trousers".to_string()],
        });
        store.insert(Synonym::OneWay {
            object_id: "tablet-ipad".to_string(),
            input: "tablet".to_string(),
            synonyms: vec!["ipad".to_string()],
        });

        let (hits, total) = store.search("pants", None, 0, 10);
        assert_eq!(total, 1);
        assert_eq!(hits[0].object_id(), "pants-trousers");

        let (hits, total) = store.search("", Some("synonym"), 0, 10);
        assert_eq!(total, 1);
        assert_eq!(hits[0].synonym_type(), "synonym");
    }

    #[test]
    fn query_expansion() {
        let mut store = SynonymStore::new();
        store.insert(Synonym::Regular {
            object_id: "pants-trousers".to_string(),
            synonyms: vec!["pants".to_string(), "trousers".to_string()],
        });

        let expanded = store.expand_query("black pants");
        assert!(expanded.contains(&"black pants".to_string()));
        assert!(expanded.contains(&"black trousers".to_string()));
    }
}
