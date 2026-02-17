use flapjack::tokenizer::CjkAwareTokenizer;
use tantivy::tokenizer::{LowerCaser, TextAnalyzer, TokenStream, Tokenizer};

fn raw_tokens(text: &str) -> Vec<(String, usize)> {
    let mut tok = CjkAwareTokenizer;
    let mut stream = tok.token_stream(text);
    let mut out = Vec::new();
    while stream.advance() {
        out.push((stream.token().text.clone(), stream.token().position));
    }
    out
}

fn ngram_tokens(text: &str) -> Vec<String> {
    let analyzer = TextAnalyzer::builder(CjkAwareTokenizer)
        .filter(LowerCaser)
        .filter(tantivy::tokenizer::EdgeNgramFilter::new(2, 10).unwrap())
        .build();
    let mut tok = analyzer;
    let mut stream = tok.token_stream(text);
    let mut out = Vec::new();
    while stream.advance() {
        out.push(stream.token().text.clone());
    }
    out
}

fn simple_tokens(text: &str) -> Vec<String> {
    let analyzer = TextAnalyzer::builder(CjkAwareTokenizer)
        .filter(LowerCaser)
        .build();
    let mut tok = analyzer;
    let mut stream = tok.token_stream(text);
    let mut out = Vec::new();
    while stream.advance() {
        out.push(stream.token().text.clone());
    }
    out
}

#[test]
fn debug_concat_emission() {
    let test_cases = vec![
        "O'Kelly",
        "D'Agostino",
        "Abdel-Rahman",
        "mens-watches",
        "Jean-Pierre",
    ];

    println!("\n=== RAW TOKENIZER OUTPUT ===");
    for input in &test_cases {
        let tokens = raw_tokens(input);
        println!("{:20} → {:?}", input, tokens);
    }

    println!("\n=== EDGE-NGRAM (index pipeline) ===");
    for input in &test_cases {
        let tokens = ngram_tokens(input);
        let unique: std::collections::BTreeSet<_> = tokens.into_iter().collect();
        println!("{:20} → {:?}", input, unique);
    }

    println!("\n=== SIMPLE (query pipeline) ===");
    for input in &test_cases {
        let tokens = simple_tokens(input);
        println!("{:20} → {:?}", input, tokens);
    }

    println!("\n=== CONCAT VERIFICATION ===");
    let critical = vec![
        ("Abdel-Rahman", "AbdelRahman"),
        ("mens-watches", "menswatches"),
        ("Jean-Pierre", "JeanPierre"),
        ("O'Kelly", "OKelly"),
        ("D'Agostino", "DAgostino"),
    ];
    for (input, expected_concat) in &critical {
        let raw = raw_tokens(input);
        let has_concat = raw.iter().any(|(t, _)| t == expected_concat);
        println!(
            "{:20} concat='{}' emitted={}",
            input, expected_concat, has_concat
        );
    }

    println!("\n=== QUERY-SIDE SPLIT ===");
    let queries = vec![
        "okelly",
        "abdelrahman",
        "menswatches",
        "jeanpierre",
        "dagostino",
        "o'kelly",
        "abdel-rahman",
        "mens-watches",
    ];
    for q in &queries {
        let tokens = simple_tokens(q);
        println!("{:20} → {:?}", q, tokens);
    }
}
