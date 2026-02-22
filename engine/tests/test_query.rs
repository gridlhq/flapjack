//! Tests that depend on flapjack_http::dto::SearchRequest.
//! All other query tests moved to engine/src/integ_tests/test_query.rs.

use flapjack::query::stopwords::RemoveStopWordsValue;

mod stopwords_dto {
    use super::*;

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
