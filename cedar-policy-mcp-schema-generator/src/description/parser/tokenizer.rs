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

use super::{err::TokenizerError, loc::Loc};
use std::sync::Arc;

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

#[derive(Debug, Clone)]
pub(crate) struct Token {
    kind: TokenKind,
    loc: Loc,
}

impl Token {
    pub(crate) fn kind(&self) -> TokenKind {
        self.kind
    }

    pub(crate) fn into_loc(self) -> Loc {
        self.loc
    }

    pub(crate) fn as_loc(&self) -> &Loc {
        &self.loc
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Tokenizer {
    input: Arc<str>,
    cur_pos: usize,
}

impl Tokenizer {
    // Create a new tokenizer which lazily tokenizes the input str.
    // All tokens track the portion of the input str that corresponds to the token.
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
    fn next_char(&mut self) -> Result<u8, TokenizerError> {
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
                    return Err(TokenizerError::unexpected_eof(loc, msg));
                }
            }
        }
    }

    // Consumes the the identifier `ident` from input str
    // Returns error if the next `ident.len()` characters
    // is not equal to `ident`
    fn consume_ident(&mut self, ident: &str) -> Result<(), TokenizerError> {
        self.cur_pos -= 1;
        if self.cur_pos + ident.len() > self.input.len() {
            let loc = Loc::new((self.input.len() - 1, 0), self.input.clone());
            let msg = format!("Encountered end of input while trying to read {ident}");
            return Err(TokenizerError::unexpected_eof(loc, msg.as_str()));
        }
        if self.input[self.cur_pos..].starts_with(ident) {
            self.cur_pos += ident.len();
            Ok(())
        } else {
            let loc = Loc::new((self.cur_pos, ident.len()), self.input.clone());
            let msg = format!("Expected {ident}");
            Err(TokenizerError::unexpected_token(loc, msg.as_str()))
        }
    }

    // Helper function for `consume_esape_sequence` to
    // consume a hex digit (to handle unicode escape sequences)
    fn eat_hex_digit(&mut self) -> Result<(), TokenizerError> {
        match self.input.as_bytes().get(self.cur_pos) {
            Some(b'0'..=b'9') | Some(b'a'..=b'f') | Some(b'A'..=b'F') => {
                self.eat_char();
                Ok(())
            }
            Some(_) => {
                let loc = Loc::new((self.cur_pos, 1), self.input.clone());
                let msg = "Expected valid unicode escape sequence";
                Err(TokenizerError::unknown_escape_sequence(loc, msg))
            }
            None => {
                let loc = Loc::new((self.cur_pos - 1, 0), self.input.clone());
                let msg = "Expected valid unicode escape sequence";
                Err(TokenizerError::unexpected_eof(loc, msg))
            }
        }
    }

    // Helper function for `consume_str_literal` which
    // consumes an escape sequence assuming first '\' has already been consumed
    fn consume_escape_sequence(&mut self) -> Result<(), TokenizerError> {
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
                Err(TokenizerError::unknown_escape_sequence(loc, msg))
            }
            None => {
                let loc = Loc::new((self.cur_pos - 1, 1), self.input.clone());
                let msg = "Expected valid escape sequence";
                Err(TokenizerError::unexpected_eof(loc, msg))
            }
        }
    }

    // Consumes a str literal assuming opening '"' has already been consumed
    fn consume_str_literal(&mut self) -> Result<(), TokenizerError> {
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
                    return Err(TokenizerError::unexpected_token(loc, msg));
                }
                Some(_) => self.eat_char(),
                None => {
                    let loc = Loc::new((self.cur_pos - 1, 0), self.input.clone());
                    let msg = "Found end of input while parsing string literal";
                    return Err(TokenizerError::unexpected_eof(loc, msg));
                }
            }
        }
    }

    // Consumes a positive number literal
    fn consume_number_literal(&mut self) -> Result<(), TokenizerError> {
        // Integral Part
        match self.input.as_bytes().get(self.cur_pos) {
            Some(b'0') => {
                self.eat_char();
                if matches!(self.input.as_bytes().get(self.cur_pos), Some(b'0'..=b'9')) {
                    let loc = Loc::new((self.cur_pos - 1, 1), self.input.clone());
                    let msg = "Number literals cannot include leading 0s";
                    return Err(TokenizerError::invalid_number(loc, msg));
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
                return Err(TokenizerError::invalid_number(loc, msg));
            }
            None => {
                let loc = Loc::new((self.cur_pos - 1, 0), self.input.clone());
                let msg = "Found end of input while parsing number literal";
                return Err(TokenizerError::unexpected_eof(loc, msg));
            }
        }

        // Fractional Part
        if matches!(self.input.as_bytes().get(self.cur_pos), Some(b'.')) {
            self.eat_char();

            // Must have at least one digit following '.'
            if !matches!(self.input.as_bytes().get(self.cur_pos), Some(b'0'..=b'9')) {
                let loc = Loc::new((self.cur_pos - 1, 1), self.input.clone());
                let msg = "Number literals must have at least one digit (0-9) following the decimal point";
                return Err(TokenizerError::invalid_number(loc, msg));
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
                return Err(TokenizerError::invalid_number(loc, msg));
            }

            while matches!(self.input.as_bytes().get(self.cur_pos), Some(b'0'..=b'9')) {
                self.eat_char();
            }
        }

        Ok(())
    }

    // Parse any token from the input str
    pub(crate) fn get_token(&mut self) -> Result<Token, TokenizerError> {
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
                Err(TokenizerError::unexpected_token(loc, msg))
            }
        }
    }
}
