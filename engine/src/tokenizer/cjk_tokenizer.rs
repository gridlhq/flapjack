use tantivy::tokenizer::{Token, TokenStream, Tokenizer};

#[derive(Clone, Default)]
pub struct CjkAwareTokenizer;

fn is_cjk(c: char) -> bool {
    matches!(c,
        '\u{4E00}'..='\u{9FFF}' |
        '\u{3400}'..='\u{4DBF}' |
        '\u{F900}'..='\u{FAFF}' |
        '\u{2E80}'..='\u{2EFF}' |
        '\u{3000}'..='\u{303F}' |
        '\u{3040}'..='\u{309F}' |
        '\u{30A0}'..='\u{30FF}' |
        '\u{31F0}'..='\u{31FF}' |
        '\u{AC00}'..='\u{D7AF}' |
        '\u{1100}'..='\u{11FF}' |
        '\u{20000}'..='\u{2A6DF}' |
        '\u{2A700}'..='\u{2B73F}' |
        '\u{2B740}'..='\u{2B81F}' |
        '\u{2B820}'..='\u{2CEAF}'
    )
}

fn is_intra_word_separator(c: char) -> bool {
    !c.is_alphanumeric() && !c.is_whitespace() && !is_cjk(c) && c != '\0'
}

pub struct CjkAwareTokenStream {
    tokens: Vec<Token>,
    index: usize,
}

impl TokenStream for CjkAwareTokenStream {
    fn advance(&mut self) -> bool {
        if self.index < self.tokens.len() {
            self.index += 1;
            true
        } else {
            false
        }
    }

    fn token(&self) -> &Token {
        &self.tokens[self.index - 1]
    }

    fn token_mut(&mut self) -> &mut Token {
        &mut self.tokens[self.index - 1]
    }
}

impl Tokenizer for CjkAwareTokenizer {
    type TokenStream<'a> = CjkAwareTokenStream;

    fn token_stream<'a>(&'a mut self, text: &'a str) -> Self::TokenStream<'a> {
        let mut tokens = Vec::new();
        let mut position = 0;
        let mut chars = text.char_indices().peekable();

        let mut pending_concat: Option<(usize, String)> = None;
        let mut pending_parts: usize = 0;
        let mut saw_separator = false;

        while let Some(&(byte_offset, c)) = chars.peek() {
            if is_cjk(c) {
                if let Some((concat_start, concat_text)) = pending_concat.take() {
                    if pending_parts >= 2 && concat_text.len() >= 3 {
                        tokens.push(Token {
                            offset_from: concat_start,
                            offset_to: byte_offset,
                            position,
                            text: concat_text,
                            ..Default::default()
                        });
                        position += 1;
                    }
                }
                pending_parts = 0;
                saw_separator = false;

                let len = c.len_utf8();
                tokens.push(Token {
                    offset_from: byte_offset,
                    offset_to: byte_offset + len,
                    position,
                    text: c.to_string(),
                    ..Default::default()
                });
                position += 1;
                chars.next();
            } else if c.is_alphanumeric() {
                let start = byte_offset;
                let mut end = byte_offset;
                let mut word = String::new();
                while let Some(&(bi, ci)) = chars.peek() {
                    if ci.is_alphanumeric() && !is_cjk(ci) {
                        end = bi + ci.len_utf8();
                        word.push(ci);
                        chars.next();
                    } else {
                        break;
                    }
                }

                tokens.push(Token {
                    offset_from: start,
                    offset_to: end,
                    position,
                    text: text[start..end].to_string(),
                    ..Default::default()
                });
                position += 1;

                if saw_separator {
                    if let Some((_, ref mut concat_text)) = pending_concat {
                        concat_text.push_str(&word);
                        pending_parts += 1;
                    }
                } else if pending_concat.is_none() {
                    pending_concat = Some((start, word.clone()));
                    pending_parts = 1;
                }
                saw_separator = false;
            } else if is_intra_word_separator(c) {
                saw_separator = true;
                chars.next();
            } else {
                if let Some((concat_start, concat_text)) = pending_concat.take() {
                    if pending_parts >= 2 && concat_text.len() >= 3 {
                        tokens.push(Token {
                            offset_from: concat_start,
                            offset_to: byte_offset,
                            position,
                            text: concat_text,
                            ..Default::default()
                        });
                        position += 1;
                    }
                }
                pending_parts = 0;
                saw_separator = false;
                chars.next();
            }
        }

        if let Some((concat_start, concat_text)) = pending_concat.take() {
            if pending_parts >= 2 && concat_text.len() >= 3 {
                tokens.push(Token {
                    offset_from: concat_start,
                    offset_to: text.len(),
                    position,
                    text: concat_text,
                    ..Default::default()
                });
                #[allow(unused_assignments)]
                {
                    position += 1;
                }
            }
        }

        CjkAwareTokenStream { tokens, index: 0 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tantivy::tokenizer::Tokenizer;

    fn collect_tokens(text: &str) -> Vec<String> {
        let mut tokenizer = CjkAwareTokenizer;
        let mut stream = tokenizer.token_stream(text);
        let mut result = Vec::new();
        while stream.advance() {
            result.push(stream.token().text.clone());
        }
        result
    }

    // ── is_cjk ──────────────────────────────────────────────────────────

    #[test]
    fn is_cjk_chinese() {
        assert!(is_cjk('中'));
        assert!(is_cjk('国'));
    }

    #[test]
    fn is_cjk_japanese_hiragana() {
        assert!(is_cjk('あ'));
        assert!(is_cjk('の'));
    }

    #[test]
    fn is_cjk_japanese_katakana() {
        assert!(is_cjk('ア'));
        assert!(is_cjk('ン'));
    }

    #[test]
    fn is_cjk_korean() {
        assert!(is_cjk('한'));
        assert!(is_cjk('글'));
    }

    #[test]
    fn is_cjk_ascii_false() {
        assert!(!is_cjk('a'));
        assert!(!is_cjk('Z'));
        assert!(!is_cjk('5'));
        assert!(!is_cjk(' '));
    }

    #[test]
    fn is_cjk_latin_extended_false() {
        assert!(!is_cjk('é'));
        assert!(!is_cjk('ñ'));
    }

    // ── is_intra_word_separator ─────────────────────────────────────────

    #[test]
    fn separator_hyphen() {
        assert!(is_intra_word_separator('-'));
    }

    #[test]
    fn separator_dot() {
        assert!(is_intra_word_separator('.'));
    }

    #[test]
    fn separator_underscore() {
        assert!(is_intra_word_separator('_'));
    }

    #[test]
    fn separator_not_alphanumeric() {
        assert!(!is_intra_word_separator('a'));
        assert!(!is_intra_word_separator('5'));
    }

    #[test]
    fn separator_not_whitespace() {
        assert!(!is_intra_word_separator(' '));
        assert!(!is_intra_word_separator('\t'));
    }

    #[test]
    fn separator_not_cjk() {
        assert!(!is_intra_word_separator('中'));
    }

    #[test]
    fn separator_not_null() {
        assert!(!is_intra_word_separator('\0'));
    }

    // ── tokenizer: basic Latin ──────────────────────────────────────────

    #[test]
    fn tokenize_simple_english() {
        let tokens = collect_tokens("hello world");
        assert_eq!(tokens, vec!["hello", "world"]);
    }

    #[test]
    fn tokenize_empty() {
        let tokens = collect_tokens("");
        assert!(tokens.is_empty());
    }

    #[test]
    fn tokenize_single_word() {
        let tokens = collect_tokens("flapjack");
        assert_eq!(tokens, vec!["flapjack"]);
    }

    // ── tokenizer: CJK ─────────────────────────────────────────────────

    #[test]
    fn tokenize_chinese_chars_individually() {
        let tokens = collect_tokens("中国人");
        assert_eq!(tokens, vec!["中", "国", "人"]);
    }

    #[test]
    fn tokenize_japanese_hiragana() {
        let tokens = collect_tokens("おはよう");
        assert_eq!(tokens, vec!["お", "は", "よ", "う"]);
    }

    #[test]
    fn tokenize_mixed_cjk_and_latin() {
        let tokens = collect_tokens("hello中国world");
        assert_eq!(tokens, vec!["hello", "中", "国", "world"]);
    }

    // ── tokenizer: intra-word separators (concat tokens) ────────────────

    #[test]
    fn tokenize_hyphenated_produces_parts_and_concat() {
        let tokens = collect_tokens("e-commerce");
        assert!(tokens.contains(&"e".to_string()));
        assert!(tokens.contains(&"commerce".to_string()));
        assert!(tokens.contains(&"ecommerce".to_string()));
    }

    #[test]
    fn tokenize_short_concat_skipped() {
        // "a-b" → parts "a" and "b", but concat "ab" is only 2 chars < 3, so no concat token
        let tokens = collect_tokens("a-b");
        assert!(tokens.contains(&"a".to_string()));
        assert!(tokens.contains(&"b".to_string()));
        assert!(!tokens.contains(&"ab".to_string()));
    }

    #[test]
    fn tokenize_dotted_word() {
        let tokens = collect_tokens("Dr.Smith");
        assert!(tokens.contains(&"Dr".to_string()));
        assert!(tokens.contains(&"Smith".to_string()));
        assert!(tokens.contains(&"DrSmith".to_string()));
    }

    // ── tokenizer: positions and offsets ─────────────────────────────────

    #[test]
    fn token_positions_increment() {
        let mut tokenizer = CjkAwareTokenizer;
        let mut stream = tokenizer.token_stream("hello world");
        let mut positions = Vec::new();
        while stream.advance() {
            positions.push(stream.token().position);
        }
        for i in 1..positions.len() {
            assert!(positions[i] > positions[i - 1]);
        }
    }

    #[test]
    fn token_offsets_within_text() {
        let text = "hello 中国";
        let mut tokenizer = CjkAwareTokenizer;
        let mut stream = tokenizer.token_stream(text);
        while stream.advance() {
            let t = stream.token();
            assert!(t.offset_from <= t.offset_to);
            assert!(t.offset_to <= text.len());
        }
    }

    // ── tokenizer: whitespace edge cases ────────────────────────────────

    #[test]
    fn tokenize_multiple_spaces() {
        let tokens = collect_tokens("hello   world");
        assert_eq!(tokens, vec!["hello", "world"]);
    }

    #[test]
    fn tokenize_only_whitespace() {
        let tokens = collect_tokens("   ");
        assert!(tokens.is_empty());
    }

    #[test]
    fn tokenize_mixed_whitespace() {
        let tokens = collect_tokens("hello\tworld\nnew");
        assert_eq!(tokens, vec!["hello", "world", "new"]);
    }
}
