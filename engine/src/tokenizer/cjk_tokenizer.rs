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
