use flapjack::query::highlighter::Highlighter;

#[test]
fn test_multi_word_highlighting() {
    let highlighter = Highlighter::default();
    let query_words = vec!["essence".to_string(), "mascara".to_string()];

    // Test brand field: "Essence" - should only match "essence"
    let result = highlighter.highlight_text("Essence", &query_words);
    eprintln!("\n=== brand field: 'Essence' ===");
    eprintln!("matched_words: {:?}", result.matched_words);
    eprintln!("match_level: {:?}", result.match_level);
    eprintln!("value: {}", result.value);

    assert_eq!(result.matched_words, vec!["essence"]);
    assert!(matches!(result.match_level, flapjack::query::highlighter::MatchLevel::Partial));

    // Test tags[1] field: "mascara" - should only match "mascara"
    let result2 = highlighter.highlight_text("mascara", &query_words);
    eprintln!("\n=== tags[1] field: 'mascara' ===");
    eprintln!("matched_words: {:?}", result2.matched_words);
    eprintln!("match_level: {:?}", result2.match_level);
    eprintln!("value: {}", result2.value);

    assert_eq!(result2.matched_words, vec!["mascara"]);
    assert!(matches!(result2.match_level, flapjack::query::highlighter::MatchLevel::Partial));

    // Test name field: "Essence Mascara..." - should match both
    let result3 = highlighter.highlight_text("Essence Mascara Lash Princess", &query_words);
    eprintln!("\n=== name field: 'Essence Mascara Lash Princess' ===");
    eprintln!("matched_words: {:?}", result3.matched_words);
    eprintln!("match_level: {:?}", result3.match_level);
    eprintln!("value: {}", result3.value);

    assert_eq!(result3.matched_words, vec!["essence", "mascara"]);
    assert!(matches!(result3.match_level, flapjack::query::highlighter::MatchLevel::Full));
}
