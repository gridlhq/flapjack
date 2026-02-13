use flapjack::tokenizer::CjkAwareTokenizer;
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
fn test_apostrophe_produces_concat() {
    let tokens = tokenize("O'Kelly");
    assert!(
        tokens.contains(&"O".to_string()),
        "should have 'O': {:?}",
        tokens
    );
    assert!(
        tokens.contains(&"Kelly".to_string()),
        "should have 'Kelly': {:?}",
        tokens
    );
    assert!(
        tokens.contains(&"OKelly".to_string()),
        "should have concat 'OKelly': {:?}",
        tokens
    );
}

#[test]
fn test_hyphen_produces_concat() {
    let tokens = tokenize("Abdel-Rahman");
    assert!(
        tokens.contains(&"Abdel".to_string()),
        "should have 'Abdel': {:?}",
        tokens
    );
    assert!(
        tokens.contains(&"Rahman".to_string()),
        "should have 'Rahman': {:?}",
        tokens
    );
    assert!(
        tokens.contains(&"AbdelRahman".to_string()),
        "should have concat 'AbdelRahman': {:?}",
        tokens
    );
}

#[test]
fn test_hyphen_multi_part() {
    let tokens = tokenize("mens-watches");
    assert!(
        tokens.contains(&"mens".to_string()),
        "should have 'mens': {:?}",
        tokens
    );
    assert!(
        tokens.contains(&"watches".to_string()),
        "should have 'watches': {:?}",
        tokens
    );
    assert!(
        tokens.contains(&"menswatches".to_string()),
        "should have concat 'menswatches': {:?}",
        tokens
    );
}

#[test]
fn test_no_concat_for_plain_words() {
    let tokens = tokenize("hello world");
    assert_eq!(tokens, vec!["hello", "world"]);
}

#[test]
fn test_no_concat_short_prefix() {
    let tokens = tokenize("a.b");
    let concat: Vec<_> = tokens
        .iter()
        .filter(|t| t.len() > 1 && *t != "a" && *t != "b")
        .collect();
    assert!(
        concat.is_empty(),
        "should NOT concat when result < 3 chars: {:?}",
        tokens
    );
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
