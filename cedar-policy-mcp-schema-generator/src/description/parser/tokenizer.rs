/*
 * Copyright Cedar Contributors
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *      https://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

use super::{err::TokenizeError, loc::Loc};
use std::sync::Arc;

/// The kind of accepted JSON Tokens / Lexemes
#[derive(Debug, Clone, Copy)]
pub(crate) enum TokenKind {
    Null,
    Bool(bool),
    Number, // The text of token can be retrieved from loc
    String, // The text of token can be retrieved from loc
    ArrayStart,
    ArrayEnd,
    ObjectStart,
    ObjectEnd,
    Comma,
    Colon,
}

/// A JSON `Token` with both its kind and location within the input string
#[derive(Debug, Clone)]
pub(crate) struct Token {
    kind: TokenKind,
    loc: Loc,
}

impl Token {
    /// Retrieve what `kind` of lexeme the `Token` represents
    pub(crate) fn kind(&self) -> TokenKind {
        self.kind
    }

    /// Unwrap the `Token` and retrieve its location within the input string
    pub(crate) fn into_loc(self) -> Loc {
        self.loc
    }

    /// Return a reference to the location of the `Token` within the input string
    pub(crate) fn as_loc(&self) -> &Loc {
        &self.loc
    }

    #[cfg(test)]
    pub(crate) fn to_number_str(&self) -> Option<&str> {
        self.loc.snippet()
    }

    #[cfg(test)]
    pub(crate) fn to_str(&self) -> Option<&str> {
        self.loc.snippet().and_then(|s| {
            if s.len() >= 2 {
                Some(&s[1..s.len() - 1])
            } else {
                None
            }
        })
    }
}

/// A Tokenizer that lazily tokenizes the input String
#[derive(Debug, Clone)]
pub(crate) struct Tokenizer {
    input: Arc<str>,
    cur_pos: usize,
}

impl Tokenizer {
    /// Create a new tokenizer which lazily tokenizes the input str.
    /// All tokens track the portion of the input str that corresponds to the token.
    pub(crate) fn new(input: &str) -> Self {
        Self {
            input: Arc::from(input),
            cur_pos: 0,
        }
    }

    // Create a new token of Kind `kind` with source information
    fn new_token(&self, start: usize, len: usize, kind: TokenKind) -> Token {
        let loc = Loc::new((start, len), self.input.clone());
        Token { kind, loc }
    }

    // Consume 1 byte from input string
    // Should only be called if `self.cur_pos < self.input.len()`
    fn eat_char(&mut self) {
        self.cur_pos += 1
    }

    // Consume the next byte (ignoring any whitespace)
    // Returns EOF error if no such character is available
    fn next_char(&mut self) -> Result<u8, TokenizeError> {
        loop {
            match self.input.as_bytes().get(self.cur_pos) {
                Some(b' ') | Some(b'\t') | Some(b'\n') | Some(b'\r') => self.eat_char(),
                Some(c) => {
                    let ret = *c;
                    self.eat_char();
                    return Ok(ret);
                }
                None => {
                    let pos = if self.cur_pos > 0 {
                        self.cur_pos - 1
                    } else {
                        0
                    };
                    let loc = Loc::new((pos, 0), self.input.clone());
                    let msg = "Expected more input.";
                    return Err(TokenizeError::unexpected_eof(loc, msg));
                }
            }
        }
    }

    // Consumes the the identifier `ident` from input str
    // Returns error if the next `ident.len()` characters
    // is not equal to `ident`
    fn consume_ident(&mut self, ident: &str) -> Result<(), TokenizeError> {
        self.cur_pos -= 1;
        if self.cur_pos + ident.len() > self.input.len() {
            let loc = Loc::new((self.input.len() - 1, 0), self.input.clone());
            let msg = format!("Encountered end of input while trying to read {ident}");
            return Err(TokenizeError::unexpected_eof(loc, msg.as_str()));
        }
        if self.input[self.cur_pos..].starts_with(ident) {
            self.cur_pos += ident.len();
            Ok(())
        } else {
            let loc = Loc::new((self.cur_pos, ident.len()), self.input.clone());
            let msg = format!("Expected {ident}");
            Err(TokenizeError::unexpected_token(loc, msg.as_str()))
        }
    }

    // Helper function for `consume_esape_sequence` to
    // consume a hex digit (to handle unicode escape sequences)
    fn eat_hex_digit(&mut self) -> Result<(), TokenizeError> {
        match self.input.as_bytes().get(self.cur_pos) {
            Some(b'0'..=b'9') | Some(b'a'..=b'f') | Some(b'A'..=b'F') => {
                self.eat_char();
                Ok(())
            }
            Some(_) => {
                let loc = Loc::new((self.cur_pos, 1), self.input.clone());
                let msg = "Expected valid unicode escape sequence";
                Err(TokenizeError::unknown_escape_sequence(loc, msg))
            }
            None => {
                let loc = Loc::new((self.cur_pos - 1, 0), self.input.clone());
                let msg = "Expected valid unicode escape sequence";
                Err(TokenizeError::unexpected_eof(loc, msg))
            }
        }
    }

    // Helper function for `consume_str_literal` which
    // consumes an escape sequence assuming first '\' has already been consumed
    fn consume_escape_sequence(&mut self) -> Result<(), TokenizeError> {
        match self.input.as_bytes().get(self.cur_pos) {
            Some(b'"') | Some(b'\\') | Some(b'/') | Some(b'b') | Some(b'f') | Some(b'n')
            | Some(b'r') | Some(b't') => {
                self.eat_char();
                Ok(())
            }
            Some(b'u') => {
                // unicode escape sequence
                self.eat_char();
                for _ in 0..4 {
                    self.eat_hex_digit()?
                }
                Ok(())
            }
            Some(_) => {
                let loc = Loc::new((self.cur_pos - 1, 2), self.input.clone());
                let msg = "Expected valid escape sequence";
                Err(TokenizeError::unknown_escape_sequence(loc, msg))
            }
            None => {
                let loc = Loc::new((self.cur_pos - 1, 1), self.input.clone());
                let msg = "Expected valid escape sequence";
                Err(TokenizeError::unexpected_eof(loc, msg))
            }
        }
    }

    // Consumes a str literal assuming opening '"' has already been consumed
    fn consume_str_literal(&mut self) -> Result<(), TokenizeError> {
        loop {
            match self.input.as_bytes().get(self.cur_pos) {
                Some(b'"') => {
                    // End of String reached
                    self.eat_char();
                    return Ok(());
                }
                Some(b'\\') => {
                    // Escape sequence started
                    self.eat_char();
                    self.consume_escape_sequence()?
                }
                Some(b) if *b < 0x20 => {
                    let loc = Loc::new((self.cur_pos, 1), self.input.clone());
                    let msg = "String literals cannot include control characters";
                    return Err(TokenizeError::unexpected_token(loc, msg));
                }
                Some(_) => self.eat_char(),
                None => {
                    let loc = Loc::new((self.cur_pos - 1, 0), self.input.clone());
                    let msg = "Found end of input while parsing string literal";
                    return Err(TokenizeError::unexpected_eof(loc, msg));
                }
            }
        }
    }

    // Consumes a positive number literal
    fn consume_number_literal(&mut self) -> Result<(), TokenizeError> {
        // Integral Part
        match self.input.as_bytes().get(self.cur_pos) {
            Some(b'0') => {
                self.eat_char();
                if matches!(self.input.as_bytes().get(self.cur_pos), Some(b'0'..=b'9')) {
                    let loc = Loc::new((self.cur_pos - 1, 1), self.input.clone());
                    let msg = "Number literals cannot include leading 0s";
                    return Err(TokenizeError::invalid_number(loc, msg));
                }
            }
            Some(b'1'..=b'9') => {
                while matches!(self.input.as_bytes().get(self.cur_pos), Some(b'0'..=b'9')) {
                    self.eat_char();
                }
            }
            Some(_) => {
                let loc = Loc::new((self.cur_pos, 1), self.input.clone());
                let msg = "Unexpected character in number literal";
                return Err(TokenizeError::invalid_number(loc, msg));
            }
            None => {
                let loc = Loc::new((self.cur_pos - 1, 0), self.input.clone());
                let msg = "Found end of input while parsing number literal";
                return Err(TokenizeError::unexpected_eof(loc, msg));
            }
        }

        // Fractional Part
        if matches!(self.input.as_bytes().get(self.cur_pos), Some(b'.')) {
            self.eat_char();

            // Must have at least one digit following '.'
            if !matches!(self.input.as_bytes().get(self.cur_pos), Some(b'0'..=b'9')) {
                let loc = Loc::new((self.cur_pos - 1, 1), self.input.clone());
                let msg = "Number literals must have at least one digit (0-9) following the decimal point";
                return Err(TokenizeError::invalid_number(loc, msg));
            }

            while matches!(self.input.as_bytes().get(self.cur_pos), Some(b'0'..=b'9')) {
                self.eat_char();
            }
        }

        // Exponent Part
        if matches!(
            self.input.as_bytes().get(self.cur_pos),
            Some(b'e') | Some(b'E')
        ) {
            self.eat_char();

            // optional sign
            if matches!(
                self.input.as_bytes().get(self.cur_pos),
                Some(b'+') | Some(b'-')
            ) {
                self.eat_char();
            }

            // Must have at least one digit following exponent
            if !matches!(self.input.as_bytes().get(self.cur_pos), Some(b'0'..=b'9')) {
                let loc = Loc::new((self.cur_pos - 1, 1), self.input.clone());
                let msg = "Number literals must have at least one digit (0-9) following exponent";
                return Err(TokenizeError::invalid_number(loc, msg));
            }

            while matches!(self.input.as_bytes().get(self.cur_pos), Some(b'0'..=b'9')) {
                self.eat_char();
            }
        }

        Ok(())
    }

    /// Retrieve one `Token` from the `Tokenizer`'s input string
    pub(crate) fn get_token(&mut self) -> Result<Token, TokenizeError> {
        let next = self.next_char()?;
        let start = self.cur_pos - 1;

        match next {
            b't' => {
                // true
                self.consume_ident("true")?;
                Ok(self.new_token(start, 4, TokenKind::Bool(true)))
            }
            b'f' => {
                // false
                self.consume_ident("false")?;
                Ok(self.new_token(start, 5, TokenKind::Bool(false)))
            }
            b'n' => {
                // null
                self.consume_ident("null")?;
                Ok(self.new_token(start, 4, TokenKind::Null))
            }
            b'-' => {
                self.consume_number_literal()?;
                Ok(self.new_token(start, self.cur_pos - start, TokenKind::Number))
            }
            b'0'..=b'9' => {
                // number
                self.cur_pos -= 1; // unconsume first digit
                self.consume_number_literal()?;
                Ok(self.new_token(start, self.cur_pos - start, TokenKind::Number))
            }
            b'"' => {
                // string
                self.consume_str_literal()?;
                Ok(self.new_token(start, self.cur_pos - start, TokenKind::String))
            }
            b'[' => Ok(self.new_token(start, 1, TokenKind::ArrayStart)),
            b']' => Ok(self.new_token(start, 1, TokenKind::ArrayEnd)),
            b'{' => Ok(self.new_token(start, 1, TokenKind::ObjectStart)),
            b'}' => Ok(self.new_token(start, 1, TokenKind::ObjectEnd)),
            b',' => Ok(self.new_token(start, 1, TokenKind::Comma)),
            b':' => Ok(self.new_token(start, 1, TokenKind::Colon)),
            _ => {
                let loc = Loc::new((self.cur_pos - 1, 1), self.input.clone());
                let msg = "Expected one of `null`, `true`, `false`, `:`, `,`, `[`, `]`, `{`, `}`, or string or number literal";
                Err(TokenizeError::unexpected_token(loc, msg))
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use cool_asserts::assert_matches;

    #[test]
    fn tokenizes_comma() {
        let mut tokenizer = Tokenizer::new(",");
        assert_matches!(
            tokenizer.get_token(),
            Ok(Token {
                kind: TokenKind::Comma,
                ..
            })
        );
        assert_matches!(
            tokenizer.get_token(),
            Err(TokenizeError::UnexpectedEof(..))
        );
    }

    #[test]
    fn tokenizes_colon() {
        let mut tokenizer = Tokenizer::new(":");
        assert_matches!(
            tokenizer.get_token(),
            Ok(Token {
                kind: TokenKind::Colon,
                ..
            })
        );
        assert_matches!(
            tokenizer.get_token(),
            Err(TokenizeError::UnexpectedEof(..))
        );
    }

    #[test]
    fn tokenizes_array_begin() {
        let mut tokenizer = Tokenizer::new("[");
        assert_matches!(
            tokenizer.get_token(),
            Ok(Token {
                kind: TokenKind::ArrayStart,
                ..
            })
        );
        assert_matches!(
            tokenizer.get_token(),
            Err(TokenizeError::UnexpectedEof(..))
        );
    }

    #[test]
    fn tokenizes_array_end() {
        let mut tokenizer = Tokenizer::new("]");
        assert_matches!(
            tokenizer.get_token(),
            Ok(Token {
                kind: TokenKind::ArrayEnd,
                ..
            })
        );
        assert_matches!(
            tokenizer.get_token(),
            Err(TokenizeError::UnexpectedEof(..))
        );
    }

    #[test]
    fn tokenizes_object_begin() {
        let mut tokenizer = Tokenizer::new("{");
        assert_matches!(
            tokenizer.get_token(),
            Ok(Token {
                kind: TokenKind::ObjectStart,
                ..
            })
        );
        assert_matches!(
            tokenizer.get_token(),
            Err(TokenizeError::UnexpectedEof(..))
        );
    }

    #[test]
    fn tokenizes_object_end() {
        let mut tokenizer = Tokenizer::new("}");
        assert_matches!(
            tokenizer.get_token(),
            Ok(Token {
                kind: TokenKind::ObjectEnd,
                ..
            })
        );
        assert_matches!(
            tokenizer.get_token(),
            Err(TokenizeError::UnexpectedEof(..))
        );
    }

    #[test]
    fn tokenize_true() {
        let mut tokenizer = Tokenizer::new("true");
        assert_matches!(
            tokenizer.get_token(),
            Ok(Token {
                kind: TokenKind::Bool(true),
                ..
            })
        );
        assert_matches!(
            tokenizer.get_token(),
            Err(TokenizeError::UnexpectedEof(..))
        );
    }

    #[test]
    fn tokenize_false() {
        let mut tokenizer = Tokenizer::new("false");
        assert_matches!(
            tokenizer.get_token(),
            Ok(Token {
                kind: TokenKind::Bool(false),
                ..
            })
        );
        assert_matches!(
            tokenizer.get_token(),
            Err(TokenizeError::UnexpectedEof(..))
        );
    }

    #[test]
    fn tokenize_null() {
        let mut tokenizer = Tokenizer::new("null");
        assert_matches!(
            tokenizer.get_token(),
            Ok(Token {
                kind: TokenKind::Null,
                ..
            })
        );
        assert_matches!(
            tokenizer.get_token(),
            Err(TokenizeError::UnexpectedEof(..))
        );
    }

    macro_rules! test_tokenize_number {
        ($test_name:ident, $input:literal) => {
            #[test]
            fn $test_name() {
                let mut tokenizer = Tokenizer::new($input);
                let token = tokenizer
                    .get_token()
                    .expect(&format!("Failed to tokenize `{}`", $input));
                assert_matches!(
                    token,
                    Token {
                        kind: TokenKind::Number,
                        ..
                    }
                );
                assert_eq!(token.to_number_str(), Some($input));
                assert_matches!(
                    tokenizer.get_token(),
                    Err(TokenizeError::UnexpectedEof(..))
                );
            }
        };
    }

    test_tokenize_number!(tokenize_int_zero, "0");
    test_tokenize_number!(tokenize_neg_int, "-120");
    test_tokenize_number!(tokenize_pos_int, "920");
    test_tokenize_number!(tokenize_int_zero_exp, "0e1");
    test_tokenize_number!(tokenize_int_zero_exp_pos, "0E+1");
    test_tokenize_number!(tokenize_int_zero_exp_neg, "0e-1");
    test_tokenize_number!(tokenize_int_pos_exp, "43e0");
    test_tokenize_number!(tokenize_int_pos_exp_pos, "21E+9");
    test_tokenize_number!(tokenize_int_pos_exp_neg, "21E-1");
    test_tokenize_number!(tokenize_float_zero, "0.0");
    test_tokenize_number!(tokenize_neg_float, "-1.000");
    test_tokenize_number!(tokenize_pos_float, "93.120");
    test_tokenize_number!(tokenize_float_zero_exp, "0.0E9");
    test_tokenize_number!(tokenize_float_zero_exp_pos, "0.0e+2");
    test_tokenize_number!(tokenize_float_zero_exp_neg, "0.0e-1");
    test_tokenize_number!(tokenize_float_pos_exp, "10.0E0");
    test_tokenize_number!(tokenize_float_pos_exp_pos, "21.0e+0");
    test_tokenize_number!(tokenize_float_pos_exp_neg, "99.012e-91");

    macro_rules! test_tokenize_invalid_number {
        ($test_name:ident, $input:literal) => {
            #[test]
            fn $test_name() {
                let mut tokenizer = Tokenizer::new($input);
                assert_matches!(
                    tokenizer.get_token(),
                    Err(TokenizeError::InvalidNumberLiteral(..))
                );
            }
        };
    }

    test_tokenize_invalid_number!(tokenize_fail_leading_zero, "01");
    test_tokenize_invalid_number!(tokenize_fail_leading_zero_neg, "-01");
    test_tokenize_invalid_number!(tokenize_fail_leading_zero_float, "01.0");
    test_tokenize_invalid_number!(tokenize_fail_float_no_trailing_digits, "0.");
    test_tokenize_invalid_number!(tokenize_fail_neg_but_not_number, "-a");
    test_tokenize_invalid_number!(tokenize_fail_exp_no_number1, "-1e");
    test_tokenize_invalid_number!(tokenize_fail_exp_no_number2, "1E");

    macro_rules! test_tokenize_string {
        ($test_name:ident, $input:literal) => {
            #[test]
            fn $test_name() {
                let mut tokenizer = Tokenizer::new(&format!("\"{}\"", $input));
                let token = tokenizer
                    .get_token()
                    .expect(&format!("Failed to tokenize `{}`", $input));
                assert_matches!(
                    token,
                    Token {
                        kind: TokenKind::String,
                        ..
                    }
                );
                assert_eq!(token.to_str(), Some($input));
                assert_matches!(
                    tokenizer.get_token(),
                    Err(TokenizeError::UnexpectedEof(..))
                );
            }
        };
    }

    test_tokenize_string!(tokenize_empty_str, "");
    test_tokenize_string!(tokenize_str1, "a;lkc");
    test_tokenize_string!(tokenize_str2, "hellow world!");
    test_tokenize_string!(tokenize_str3, "I'm a test!");
    test_tokenize_string!(tokenize_str4, "Woohoo <3");
    test_tokenize_string!(tokenize_quote_escape, "\\\"");
    test_tokenize_string!(tokenize_whitespace_escape, " \\n\\r\\t\\f\\b");
    test_tokenize_string!(tokenize_slash_escape, "\\\\\\/");
    test_tokenize_string!(tokenize_unicode_escape1, "\\u0000");
    test_tokenize_string!(tokenize_unicode_escape2, "\\uFFFF");
    test_tokenize_string!(tokenize_unicode_escape3, "\\uaaaa");
    test_tokenize_string!(tokenize_unicode_escape4, "\\u01Ac");

    macro_rules! test_tokenize_invalid_escape {
        ($test_name:ident, $input:literal) => {
            #[test]
            fn $test_name() {
                let mut tokenizer = Tokenizer::new(&format!("\"{}\"", $input));
                assert_matches!(
                    tokenizer.get_token(),
                    Err(TokenizeError::UnexpectedEscapeSequence(..))
                );
            }
        };
    }

    test_tokenize_invalid_escape!(tokenize_invalid_escape1, "\\0");
    test_tokenize_invalid_escape!(tokenize_invalid_escape2, "\\a");
    test_tokenize_invalid_escape!(tokenize_invalid_escape3, "\\E");
    test_tokenize_invalid_escape!(tokenize_invalid_escape4, "\\-");

    test_tokenize_invalid_escape!(tokenize_invalid_unicode_escape1, "\\u0x09");
    test_tokenize_invalid_escape!(tokenize_invalid_unicode_escape2, "\\uabaq");
    test_tokenize_invalid_escape!(tokenize_invalid_unicode_escape3, "\\uABEZ");
    test_tokenize_invalid_escape!(tokenize_invalid_unicode_escape4, "\\uNICODE");

    macro_rules! test_tokenize_unexpected_eof {
        ($test_name:ident, $input:literal) => {
            #[test]
            fn $test_name() {
                let mut tokenizer = Tokenizer::new($input);
                assert_matches!(
                    tokenizer.get_token(),
                    Err(TokenizeError::UnexpectedEof(..))
                );
            }
        };
    }

    test_tokenize_unexpected_eof!(tokenize_eof_empty_str, "");
    test_tokenize_unexpected_eof!(tokenize_eof_neg_number, "-");
    test_tokenize_unexpected_eof!(tokenize_eof_str_literal1, "\"");
    test_tokenize_unexpected_eof!(tokenize_eof_str_literal2, "\"abce");
    test_tokenize_unexpected_eof!(tokenize_eof_str_literal3, "\"q0987l");
    test_tokenize_unexpected_eof!(tokenize_eof_escape_sequence, "\"\\\"");
    test_tokenize_unexpected_eof!(tokenize_eof_unicode_escape_sequence1, "\"\\u");
    test_tokenize_unexpected_eof!(tokenize_eof_unicode_escape_sequence2, "\"\\u0");
    test_tokenize_unexpected_eof!(tokenize_eof_unicode_escape_sequence3, "\"\\u01");
    test_tokenize_unexpected_eof!(tokenize_eof_unicode_escape_sequence4, "\"\\u012");
    test_tokenize_unexpected_eof!(tokenize_eof_true, "tru");
    test_tokenize_unexpected_eof!(tokenize_eof_false, "fal");
    test_tokenize_unexpected_eof!(tokenize_eof_null, "n");
}
