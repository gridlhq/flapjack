use tantivy::tokenizer::{Token, TokenFilter, TokenStream, Tokenizer};

#[derive(Clone)]
pub struct EdgeNgramTokenFilter {
    min_gram: usize,
    max_gram: usize,
}

impl EdgeNgramTokenFilter {
    pub fn new(min_gram: usize, max_gram: usize) -> Self {
        EdgeNgramTokenFilter { min_gram, max_gram }
    }
}

impl TokenFilter for EdgeNgramTokenFilter {
    type Tokenizer<T: Tokenizer> = EdgeNgramFilterWrapper<T>;

    fn transform<T: Tokenizer>(self, tokenizer: T) -> Self::Tokenizer<T> {
        EdgeNgramFilterWrapper {
            inner: tokenizer,
            min_gram: self.min_gram,
            max_gram: self.max_gram,
        }
    }
}

#[derive(Clone)]
pub struct EdgeNgramFilterWrapper<T> {
    inner: T,
    min_gram: usize,
    max_gram: usize,
}

impl<T: Tokenizer> Tokenizer for EdgeNgramFilterWrapper<T> {
    type TokenStream<'a> = EdgeNgramTokenStream<T::TokenStream<'a>>;

    fn token_stream<'a>(&'a mut self, text: &'a str) -> Self::TokenStream<'a> {
        EdgeNgramTokenStream {
            inner: self.inner.token_stream(text),
            min_gram: self.min_gram,
            max_gram: self.max_gram,
            current_token: None,
            current_ngram_index: 0,
            ngram_token: Token::default(),
        }
    }
}

pub struct EdgeNgramTokenStream<T> {
    inner: T,
    min_gram: usize,
    max_gram: usize,
    current_token: Option<Token>,
    current_ngram_index: usize,
    ngram_token: Token,
}

impl<T: TokenStream> TokenStream for EdgeNgramTokenStream<T> {
    fn advance(&mut self) -> bool {
        loop {
            if let Some(ref token) = self.current_token {
                let char_count = token.text.chars().count();
                let ngram_len = self.min_gram + self.current_ngram_index;

                if ngram_len <= self.max_gram && ngram_len <= char_count {
                    let ngram_text: String = token.text.chars().take(ngram_len).collect();
                    self.ngram_token = token.clone();
                    self.ngram_token.text.clear();
                    self.ngram_token.text.push_str(&ngram_text);
                    self.ngram_token.offset_to = self.ngram_token.offset_from + ngram_text.len();

                    self.current_ngram_index += 1;
                    return true;
                }

                self.current_token = None;
                self.current_ngram_index = 0;
            }

            if !self.inner.advance() {
                return false;
            }

            self.current_token = Some(self.inner.token().clone());

            let char_count = self.current_token.as_ref().unwrap().text.chars().count();
            if char_count >= self.min_gram {
                self.current_ngram_index = 0;
            }
        }
    }

    fn token(&self) -> &Token {
        &self.ngram_token
    }

    fn token_mut(&mut self) -> &mut Token {
        &mut self.ngram_token
    }
}
