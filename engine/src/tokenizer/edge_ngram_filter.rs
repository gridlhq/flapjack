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

#[cfg(test)]
mod tests {
    use super::*;
    use tantivy::tokenizer::{SimpleTokenizer, TokenFilter, Tokenizer};

    fn collect_ngrams(text: &str, min: usize, max: usize) -> Vec<String> {
        let filter = EdgeNgramTokenFilter::new(min, max);
        let mut tokenizer = filter.transform(SimpleTokenizer::default());
        let mut stream = tokenizer.token_stream(text);
        let mut result = Vec::new();
        while stream.advance() {
            result.push(stream.token().text.clone());
        }
        result
    }

    #[test]
    fn ngram_single_word_1_3() {
        let tokens = collect_ngrams("hello", 1, 3);
        assert_eq!(tokens, vec!["h", "he", "hel"]);
    }

    #[test]
    fn ngram_single_word_2_5() {
        let tokens = collect_ngrams("hello", 2, 5);
        assert_eq!(tokens, vec!["he", "hel", "hell", "hello"]);
    }

    #[test]
    fn ngram_short_word_below_min() {
        // "ab" with min_gram=3 → no tokens (word too short)
        let tokens = collect_ngrams("ab", 3, 5);
        assert!(tokens.is_empty());
    }

    #[test]
    fn ngram_word_at_min() {
        // "abc" with min_gram=3, max_gram=5 → ["abc"]
        let tokens = collect_ngrams("abc", 3, 5);
        assert_eq!(tokens, vec!["abc"]);
    }

    #[test]
    fn ngram_multiple_words() {
        let tokens = collect_ngrams("hello world", 1, 2);
        assert_eq!(tokens, vec!["h", "he", "w", "wo"]);
    }

    #[test]
    fn ngram_max_gram_larger_than_word() {
        // "hi" with max_gram=10 → only produce up to length 2
        let tokens = collect_ngrams("hi", 1, 10);
        assert_eq!(tokens, vec!["h", "hi"]);
    }

    #[test]
    fn ngram_empty_text() {
        let tokens = collect_ngrams("", 1, 3);
        assert!(tokens.is_empty());
    }

    #[test]
    fn ngram_min_equals_max() {
        let tokens = collect_ngrams("hello", 3, 3);
        assert_eq!(tokens, vec!["hel"]);
    }

    #[test]
    fn ngram_preserves_position() {
        let filter = EdgeNgramTokenFilter::new(1, 2);
        let mut tokenizer = filter.transform(SimpleTokenizer::default());
        let mut stream = tokenizer.token_stream("foo bar");
        let mut positions = Vec::new();
        while stream.advance() {
            positions.push(stream.token().position);
        }
        // "f", "fo" share position 0; "b", "ba" share position 1
        assert_eq!(positions[0], positions[1]);
        assert_eq!(positions[2], positions[3]);
        assert_ne!(positions[0], positions[2]);
    }

    #[test]
    fn ngram_offsets_correct() {
        let filter = EdgeNgramTokenFilter::new(1, 3);
        let mut tokenizer = filter.transform(SimpleTokenizer::default());
        let mut stream = tokenizer.token_stream("hello");
        let mut offsets = Vec::new();
        while stream.advance() {
            let t = stream.token();
            offsets.push((t.offset_from, t.offset_to));
        }
        assert_eq!(offsets, vec![(0, 1), (0, 2), (0, 3)]);
    }
}
