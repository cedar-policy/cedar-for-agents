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

use super::err::ParseError;
use super::json_value::{LocatedString, LocatedValue};
use super::loc::Loc;
use super::tokenizer::{TokenKind, Tokenizer};
use linked_hash_map::{Entry, LinkedHashMap};

/// A Parser for JSON that lazily parses a stream of `LocatedValue`s
#[derive(Debug)]
pub(crate) struct JsonParser {
    tokenizer: Tokenizer,
}

impl JsonParser {
    /// Create a new JSON Parser to parse the input string
    pub(crate) fn new(input: &str) -> Self {
        Self {
            tokenizer: Tokenizer::new(input),
        }
    }

    fn get_object(&mut self, loc: &Loc) -> Result<LocatedValue, ParseError> {
        let mut items = LinkedHashMap::new();
        let mut maybe_empty = true;

        loop {
            let token = self.tokenizer.get_token()?;
            match token.kind() {
                TokenKind::String => {
                    maybe_empty = false;
                    let key = LocatedString::new(token.into_loc());
                    let token = self.tokenizer.get_token()?;
                    if matches!(token.kind(), TokenKind::Colon) {
                        let value = self.get_value()?;
                        // Check if duplicate value
                        match items.entry(key.clone()) {
                            Entry::Occupied(occ) => {
                                return Err(ParseError::duplicate_key(
                                    occ.key().as_loc().into(),
                                    key.into_loc(),
                                ))
                            }
                            Entry::Vacant(vac) => {
                                vac.insert(value);
                            }
                        }
                    } else {
                        return Err(ParseError::unexpected_token(
                            token.into_loc(),
                            "Expected `:`",
                        ));
                    }
                    // Finished parsing key-value pair. Check for end or prepare for next key-value pair
                    let token = self.tokenizer.get_token()?;
                    match token.kind() {
                        TokenKind::ObjectEnd => {
                            let start = loc.start();
                            let end = token.as_loc().end();
                            return Ok(LocatedValue::new_object(
                                items,
                                loc.span((start, end - start)),
                            ));
                        }
                        TokenKind::Comma => (),
                        _ => {
                            return Err(ParseError::unexpected_token(
                                token.into_loc(),
                                "Expected: `,` or `}`.",
                            ))
                        }
                    }
                }
                // Allow for an empty object
                TokenKind::ObjectEnd if maybe_empty => {
                    let start = loc.start();
                    let end = token.as_loc().end();
                    return Ok(LocatedValue::new_object(
                        items,
                        loc.span((start, end - start)),
                    ));
                }
                _ => {
                    let msg = if maybe_empty {
                        "Expected: String or `}`."
                    } else {
                        "Expected: String."
                    };
                    return Err(ParseError::unexpected_token(token.into_loc(), msg));
                }
            }
        }
    }

    fn get_array(&mut self, loc: &Loc) -> Result<LocatedValue, ParseError> {
        let mut items = Vec::new();
        let mut maybe_empty = true;

        loop {
            let token = self.tokenizer.get_token()?;

            let item = match token.kind() {
                TokenKind::Null => LocatedValue::new_null(token.into_loc()),
                TokenKind::Bool(b) => LocatedValue::new_bool(b, token.into_loc()),
                TokenKind::Number => LocatedValue::new_number(token.into_loc()),
                TokenKind::String => LocatedValue::new_string(token.into_loc()),
                TokenKind::ArrayStart => self.get_array(token.as_loc())?,
                TokenKind::ObjectStart => self.get_object(token.as_loc())?,
                TokenKind::ArrayEnd if maybe_empty => {
                    let start = loc.start();
                    let end = token.as_loc().end();
                    return Ok(LocatedValue::new_array(
                        items,
                        loc.span((start, end - start)),
                    ));
                }
                _ => {
                    let msg = if maybe_empty {
                        "Expected: `]` or value (i.e., null, Bool, Number, String, Array, or Object)."
                    } else {
                        "Expected: value (i.e., null, Bool, Number, String, Array, or Object)."
                    };
                    return Err(ParseError::unexpected_token(token.into_loc(), msg));
                }
            };
            maybe_empty = false;

            items.push(item);

            let token = self.tokenizer.get_token()?;
            match token.kind() {
                TokenKind::Comma => (),
                TokenKind::ArrayEnd => {
                    let start = loc.start();
                    let end = token.as_loc().end();
                    return Ok(LocatedValue::new_array(
                        items,
                        loc.span((start, end - start)),
                    ));
                }
                _ => {
                    return Err(ParseError::unexpected_token(
                        token.into_loc(),
                        "Expected: `]` or `,`.",
                    ))
                }
            }
        }
    }

    /// Get a single JSON Value (as a `LocatedValue`) from the Parser's input.
    pub(crate) fn get_value(&mut self) -> Result<LocatedValue, ParseError> {
        let token = self.tokenizer.get_token()?;
        match token.kind() {
            TokenKind::Null => Ok(LocatedValue::new_null(token.into_loc())),
            TokenKind::Bool(b) => Ok(LocatedValue::new_bool(b, token.into_loc())),
            TokenKind::Number => Ok(LocatedValue::new_number(token.into_loc())),
            TokenKind::String => Ok(LocatedValue::new_string(token.into_loc())),
            TokenKind::ArrayStart => self.get_array(token.as_loc()),
            TokenKind::ObjectStart => self.get_object(token.as_loc()),
            _ => Err(ParseError::unexpected_token(
                token.into_loc(),
                "Expected: value (i.e., null, Bool, Number, String, Array, or Object).",
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::mcp::parser::err::TokenizeError;

    use super::*;
    use cool_asserts::assert_matches;

    #[test]
    fn parse_true() {
        let mut parser = JsonParser::new("true");
        let value = parser.get_value().expect("Failed to parse `true`");
        assert_matches!(value.get_bool(), Some(true));
        assert_matches!(
            parser.get_value(),
            Err(ParseError::TokenizeError(TokenizeError::UnexpectedEof(..)))
        );
    }

    #[test]
    fn parse_false() {
        let mut parser = JsonParser::new("false");
        let value = parser.get_value().expect("Failed to parse `false`");
        assert_matches!(value.get_bool(), Some(false));
        assert_matches!(
            parser.get_value(),
            Err(ParseError::TokenizeError(TokenizeError::UnexpectedEof(..)))
        );
    }

    #[test]
    fn parse_null() {
        let mut parser = JsonParser::new("null");
        let value = parser.get_value().expect("Failed to parse `null`");
        assert!(value.is_null());
        assert_matches!(
            parser.get_value(),
            Err(ParseError::TokenizeError(TokenizeError::UnexpectedEof(..)))
        );
    }

    #[test]
    fn parse_number_int() {
        let mut parser = JsonParser::new("102");
        let value = parser.get_value().expect("Failed to parse `102`");
        assert_matches!(value.get_numeric_str(), Some("102"));
        assert_matches!(
            parser.get_value(),
            Err(ParseError::TokenizeError(TokenizeError::UnexpectedEof(..)))
        );
    }

    #[test]
    fn parse_number_neg_int() {
        let mut parser = JsonParser::new("-420");
        let value = parser.get_value().expect("Failed to parse `-420`");
        assert_matches!(value.get_numeric_str(), Some("-420"));
        assert_matches!(
            parser.get_value(),
            Err(ParseError::TokenizeError(TokenizeError::UnexpectedEof(..)))
        );
    }

    #[test]
    fn parse_number_int_with_exponent() {
        let mut parser = JsonParser::new("90e+10");
        let value = parser.get_value().expect("Failed to parse `90e+10`");
        assert_matches!(value.get_numeric_str(), Some("90e+10"));
        assert_matches!(
            parser.get_value(),
            Err(ParseError::TokenizeError(TokenizeError::UnexpectedEof(..)))
        );
    }

    #[test]
    fn parse_number_float() {
        let mut parser = JsonParser::new("0.000912");
        let value = parser.get_value().expect("Failed to parse `0.000912`");
        assert_matches!(value.get_numeric_str(), Some("0.000912"));
        assert_matches!(
            parser.get_value(),
            Err(ParseError::TokenizeError(TokenizeError::UnexpectedEof(..)))
        );
    }

    #[test]
    fn parse_number_neg_float() {
        let mut parser = JsonParser::new("-1092.0912");
        let value = parser.get_value().expect("Failed to parse `-1092.0912`");
        assert_matches!(value.get_numeric_str(), Some("-1092.0912"));
        assert_matches!(
            parser.get_value(),
            Err(ParseError::TokenizeError(TokenizeError::UnexpectedEof(..)))
        );
    }

    #[test]
    fn parse_number_float_with_exponent() {
        let mut parser = JsonParser::new("648.917529e-982");
        let value = parser
            .get_value()
            .expect("Failed to parse `648.917529e-982`");
        assert_matches!(value.get_numeric_str(), Some("648.917529e-982"));
        assert_matches!(
            parser.get_value(),
            Err(ParseError::TokenizeError(TokenizeError::UnexpectedEof(..)))
        );
    }

    #[test]
    fn parse_empty_string_literal() {
        let mut parser = JsonParser::new("\"\"");
        let value = parser.get_value().expect("Failed to parse `\"\"`");
        assert_matches!(value.get_str(), Some(""));
        assert_matches!(
            parser.get_value(),
            Err(ParseError::TokenizeError(TokenizeError::UnexpectedEof(..)))
        );
    }

    #[test]
    fn parse_whitespace_string_literal() {
        let mut parser = JsonParser::new("    \"   \\t\\n\\r\\t  \"        ");
        let value = parser
            .get_value()
            .expect("Failed to parse `    \"   \\t\\n\\r\\t  \"        `");
        assert_matches!(value.get_str(), Some("   \\t\\n\\r\\t  "));
        assert_matches!(
            parser.get_value(),
            Err(ParseError::TokenizeError(TokenizeError::UnexpectedEof(..)))
        );
    }

    #[test]
    fn parse_unicode_escape_seq_string_literal() {
        let mut parser = JsonParser::new("\"\\u0912\"");
        let value = parser.get_value().expect("Failed to parse `\"\\u0912\"`");
        assert_matches!(value.get_str(), Some("\\u0912"));
        assert_matches!(
            parser.get_value(),
            Err(ParseError::TokenizeError(TokenizeError::UnexpectedEof(..)))
        );
    }

    #[test]
    fn parse_empty_array() {
        let mut parser = JsonParser::new("[]");
        let value = parser.get_value().expect("Failed to parse `[]`");
        let arr = value.get_array().expect("Expected array");
        assert!(arr.is_empty());
        assert_matches!(
            parser.get_value(),
            Err(ParseError::TokenizeError(TokenizeError::UnexpectedEof(..)))
        );
    }

    #[test]
    fn parse_array_of_bools() {
        let mut parser = JsonParser::new("[true, false, true]");
        let value = parser
            .get_value()
            .expect("Failed to parse `[true, false, true]`");
        let arr = value.get_array().expect("Expected array");
        assert!(arr.len() == 3);
        assert_matches!(arr.get(0).and_then(LocatedValue::get_bool), Some(true));
        assert_matches!(arr.get(1).and_then(LocatedValue::get_bool), Some(false));
        assert_matches!(arr.get(2).and_then(LocatedValue::get_bool), Some(true));
        assert_matches!(
            parser.get_value(),
            Err(ParseError::TokenizeError(TokenizeError::UnexpectedEof(..)))
        );
    }

    #[test]
    fn parse_array_of_strs() {
        let mut parser = JsonParser::new("[\"\", \"bleh\"]");
        let value = parser
            .get_value()
            .expect("Failed to parse `[\"\", \"bleh\"]`");
        let arr = value.get_array().expect("Expected array");
        assert!(arr.len() == 2);
        assert_matches!(arr.get(0).and_then(LocatedValue::get_str), Some(""));
        assert_matches!(arr.get(1).and_then(LocatedValue::get_str), Some("bleh"));
        assert_matches!(
            parser.get_value(),
            Err(ParseError::TokenizeError(TokenizeError::UnexpectedEof(..)))
        );
    }

    #[test]
    fn parse_nested_mixed_array() {
        let mut parser = JsonParser::new("[[], null, [[]], {}, 0.1]");
        let value = parser
            .get_value()
            .expect("Failed to parse `[[], null, [[]], {}, 0.1]`");
        let arr = value.get_array().expect("Expected array");
        assert!(arr.len() == 5);
        assert_matches!(arr.get(0).and_then(LocatedValue::get_array), Some([]));
        assert_matches!(arr.get(1).map(LocatedValue::is_null), Some(true));
        assert_matches!(arr.get(2).and_then(LocatedValue::get_array), Some([..]));
        assert_matches!(
            arr.get(3)
                .and_then(LocatedValue::get_object)
                .map(LinkedHashMap::is_empty),
            Some(true)
        );
        assert_matches!(
            arr.get(4).and_then(LocatedValue::get_numeric_str),
            Some("0.1")
        );
        assert_matches!(
            parser.get_value(),
            Err(ParseError::TokenizeError(TokenizeError::UnexpectedEof(..)))
        );
    }

    #[test]
    fn parse_empty_object() {
        let mut parser = JsonParser::new("{}");
        let value = parser.get_value().expect("Failed to parse `{}`");
        let obj = value.get_object().expect("Expected object");
        assert!(obj.is_empty());
        assert_matches!(
            parser.get_value(),
            Err(ParseError::TokenizeError(TokenizeError::UnexpectedEof(..)))
        );
    }

    #[test]
    fn parse_nested_object() {
        let mut parser = JsonParser::new("{\"hi\": {}, \"bye\": false, \"\": {}}");
        let value = parser
            .get_value()
            .expect("Failed to parse `{\"hi\": {}, \"bye\": false, \"\": {}}`");
        let obj = value.get_object().expect("Expected object");
        assert!(obj.iter().count() == 3);
        assert_matches!(
            obj.get("hi")
                .and_then(LocatedValue::get_object)
                .map(LinkedHashMap::is_empty),
            Some(true)
        );
        assert_matches!(
            obj.get("")
                .and_then(LocatedValue::get_object)
                .map(LinkedHashMap::is_empty),
            Some(true)
        );
        assert_matches!(obj.get("bye").and_then(LocatedValue::get_bool), Some(false));
        assert_matches!(
            parser.get_value(),
            Err(ParseError::TokenizeError(TokenizeError::UnexpectedEof(..)))
        );
    }

    #[test]
    fn parse_fail_trailing_comma_array() {
        let mut parser = JsonParser::new("{\"hey\", true,}");
        assert_matches!(parser.get_value(), Err(ParseError::UnexpectedToken(..)))
    }

    #[test]
    fn parse_fail_trailing_comma_object() {
        let mut parser = JsonParser::new("[false, true,]");
        assert_matches!(parser.get_value(), Err(ParseError::UnexpectedToken(..)))
    }

    #[test]
    fn parse_fail_colon_in_array() {
        let mut parser = JsonParser::new("[false : true]");
        assert_matches!(parser.get_value(), Err(ParseError::UnexpectedToken(..)))
    }

    #[test]
    fn parse_fail_object_forgot_colon() {
        let mut parser = JsonParser::new("{\"true\" true}");
        assert_matches!(parser.get_value(), Err(ParseError::UnexpectedToken(..)))
    }

    #[test]
    fn parse_fail_object_non_string_key() {
        let mut parser = JsonParser::new("{true: true}");
        assert_matches!(parser.get_value(), Err(ParseError::UnexpectedToken(..)))
    }

    #[test]
    fn parse_fail_object_duplicate_keys() {
        let mut parser = JsonParser::new("{\"hi\": true, \"hi\": false}");
        assert_matches!(parser.get_value(), Err(ParseError::DuplicateKey(..)))
    }

    #[test]
    fn parse_fail_eof_in_object() {
        let mut parser = JsonParser::new("{\"hi\": true,");
        assert_matches!(parser.get_value(), Err(ParseError::TokenizeError(..)))
    }
}
