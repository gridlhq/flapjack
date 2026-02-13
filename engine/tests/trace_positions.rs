#[cfg(test)]
mod trace_test {
    use flapjack::tokenizer::CjkAwareTokenizer;
    use tantivy::tokenizer::{EdgeNgramFilter, LowerCaser, TextAnalyzer, TokenStream};

    #[test]
    fn trace_position_order() {
        let mut tokenizer = TextAnalyzer::builder(CjkAwareTokenizer)
            .filter(LowerCaser)
            .filter(EdgeNgramFilter::new(2, 20).unwrap())
            .build();

        let texts = ["CK One by Calvin Klein is a classic unisex fragrance, known for its fresh and clean scent. It's a versatile fragrance suitable for everyday wear.",
            "Coco Noir by Chanel is an elegant and mysterious fragrance, featuring notes of grapefruit, rose, and sandalwood. Perfect for evening occasions.",
            "J'adore by Dior is a luxurious and floral fragrance, known for its blend of ylang-ylang, rose, and jasmine. It embodies femininity and sophistication."];

        for (doc_idx, text) in texts.iter().enumerate() {
            let mut stream = tokenizer.token_stream(text);
            let mut per_term: std::collections::HashMap<String, Vec<u32>> =
                std::collections::HashMap::new();

            while let Some(token) = stream.next() {
                per_term
                    .entry(token.text.clone())
                    .or_default()
                    .push(token.position as u32);
            }

            for (term, positions) in &per_term {
                for i in 1..positions.len() {
                    if positions[i] < positions[i - 1] {
                        println!(
                            "DOC {} NON-MONOTONIC: term='{}' positions={:?}",
                            doc_idx, term, positions
                        );
                    }
                }
            }
        }
        println!("Trace complete.");
    }

    #[test]
    fn trace_concat_trigger() {
        let mut tokenizer = TextAnalyzer::builder(CjkAwareTokenizer)
            .filter(LowerCaser)
            .filter(EdgeNgramFilter::new(2, 20).unwrap())
            .build();

        let test_cases = vec![
            "It's the best",
            "J'adore by Dior",
            "ylang-ylang rose",
            "on-the-go makeup application",
            "The Eyeshadow Palette with Mirror offers a versatile range of eyeshadow shades for creating stunning eye looks. With a built-in mirror, it's convenient for on-the-go makeup application.",
            "The Red Lipstick is a classic and bold choice for adding a pop of color to your lips. With a creamy and pigmented formula, it provides a vibrant and long-lasting finish.",
            "CK One by Calvin Klein is a classic unisex fragrance, known for its fresh and clean scent. It's a versatile fragrance suitable for everyday wear.",
        ];

        for text in test_cases {
            let mut stream = tokenizer.token_stream(text);
            let mut per_term: std::collections::HashMap<String, Vec<u32>> =
                std::collections::HashMap::new();
            let mut all_tokens: Vec<(String, u32)> = Vec::new();

            while let Some(token) = stream.next() {
                all_tokens.push((token.text.clone(), token.position as u32));
                per_term
                    .entry(token.text.clone())
                    .or_default()
                    .push(token.position as u32);
            }

            let mut has_issue = false;
            for (term, positions) in &per_term {
                for i in 1..positions.len() {
                    if positions[i] < positions[i - 1] {
                        has_issue = true;
                        println!(
                            "NON-MONOTONIC in '{}': term='{}' positions={:?}",
                            text, term, positions
                        );
                    }
                }
            }
            if has_issue {
                println!("  All tokens for '{}':", text);
                for (t, p) in &all_tokens {
                    println!("    pos={} text='{}'", p, t);
                }
            }
        }
        println!("Concat trigger trace complete.");
    }
}
