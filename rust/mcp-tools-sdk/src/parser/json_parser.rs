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

use super::err::{ParseError, TokenizeError};
use super::json_value::{LocatedString, LocatedValue};
use super::loc::Loc;
use linked_hash_map::LinkedHashMap;
use mcp_tools_sdk_verus_proofs::parser as verified_parser;
use mcp_tools_sdk_verus_proofs::tokenizer as verified_tok;
use smol_str::SmolStr;
use std::sync::Arc;

/// A Parser for JSON that parses input into `LocatedValue`s.
///
/// Internally uses the formally verified tokenizer and parser from
/// the `mcp-tools-sdk-verus-proofs` crate. The verified parser produces
/// a `JsonValue` tree with byte-span indices, which is then converted
/// to `LocatedValue` with escape decoding and duplicate key detection.
#[derive(Debug)]
pub(crate) struct JsonParser {
    input: Arc<str>,
    consumed: bool,
}

impl JsonParser {
    /// Create a new JSON Parser to parse the input string
    pub(crate) fn new(input: &str) -> Self {
        Self {
            input: Arc::from(input),
            consumed: false,
        }
    }

    /// Get a single JSON Value (as a `LocatedValue`) from the Parser's input.
    pub(crate) fn get_value(&mut self) -> Result<LocatedValue, ParseError> {
        if self.consumed {
            let p = if self.input.is_empty() {
                0
            } else {
                self.input.len() - 1
            };
            let loc = Loc::new((p, 0), self.input.clone());
            return Err(ParseError::from(TokenizeError::unexpected_eof(
                loc,
                "Expected more input.",
            )));
        }

        let bytes = self.input.as_bytes();

        // Tokenize and parse using verified code (includes trailing token rejection)
        let value = match verified_parser::parse_json(bytes) {
            Ok(value) => value,
            Err(verified_parser::ParseJsonError::Tokenize { err }) => {
                return Err(convert_tokenize_error(&err, bytes, &self.input));
            }
            Err(verified_parser::ParseJsonError::Parse { err }) => {
                return Err(convert_parse_error(&err, bytes, &self.input));
            }
        };

        // Convert verified JsonValue tree to LocatedValue
        self.consumed = true;
        convert_value(&value, &self.input)
    }
}

/// Map a verified tokenizer error to the main crate's error type.
fn convert_tokenize_error(
    err: &verified_tok::TokenizeError,
    bytes: &[u8],
    src: &Arc<str>,
) -> ParseError {
    match err {
        verified_tok::TokenizeError::UnexpectedEof { pos } => {
            let loc = Loc::new((*pos, 0), src.clone());
            ParseError::from(TokenizeError::unexpected_eof(loc, "Expected more input."))
        }
        verified_tok::TokenizeError::InvalidNumber { pos } => {
            let loc = Loc::new((*pos, 1), src.clone());
            ParseError::from(TokenizeError::invalid_number(loc, "Invalid number literal"))
        }
        verified_tok::TokenizeError::InvalidEscape { pos } => {
            let is_eof = *pos >= bytes.len()
                || (*pos + 4 > bytes.len() && *pos >= 1 && bytes.get(*pos - 1) == Some(&0x75));
            if is_eof {
                let loc = Loc::new((*pos, 0), src.clone());
                ParseError::from(TokenizeError::unexpected_eof(loc, "Expected more input."))
            } else {
                let loc = Loc::new((*pos, 1), src.clone());
                ParseError::from(TokenizeError::unknown_escape_sequence(
                    loc,
                    "Expected valid escape sequence",
                ))
            }
        }
        verified_tok::TokenizeError::UnexpectedToken { pos } => {
            let loc = Loc::new((*pos, 1), src.clone());
            ParseError::from(TokenizeError::unexpected_token(loc, "Unexpected token"))
        }
    }
}

/// Convert a verified [`verified_parser::JsonValue`] tree into a [`LocatedValue`].
/// Escape decoding and duplicate key detection have already been done
/// by the verified parser — this is purely structural conversion.
fn convert_value(
    value: &verified_parser::JsonValue,
    src: &Arc<str>,
) -> Result<LocatedValue, ParseError> {
    match value {
        verified_parser::JsonValue::Null { start, end } => {
            let loc = Loc::new((*start, end - start), src.clone());
            Ok(LocatedValue::new_null(loc))
        }
        verified_parser::JsonValue::Bool { val, start, end } => {
            let loc = Loc::new((*start, end - start), src.clone());
            Ok(LocatedValue::new_bool(*val, loc))
        }
        verified_parser::JsonValue::Number { start, end } => {
            let loc = Loc::new((*start, end - start), src.clone());
            Ok(LocatedValue::new_number(loc))
        }
        verified_parser::JsonValue::String {
            start,
            end,
            decoded,
        } => {
            let loc = Loc::new((*start, end - start), src.clone());
            let decoded_str = std::str::from_utf8(decoded).map_err(|_| {
                ParseError::invalid_unicode_escape(loc.clone(), "Invalid UTF-8 in decoded string")
            })?;
            Ok(LocatedValue::new_string(loc, SmolStr::from(decoded_str)))
        }
        verified_parser::JsonValue::Array {
            elements,
            start,
            end,
        } => {
            let loc = Loc::new((*start, end - start), src.clone());
            let mut items = Vec::with_capacity(elements.len());
            for elem in elements.iter() {
                items.push(convert_value(elem, src)?);
            }
            Ok(LocatedValue::new_array(items, loc))
        }
        verified_parser::JsonValue::Object {
            entries,
            start,
            end,
        } => {
            let loc = Loc::new((*start, end - start), src.clone());
            let mut items: LinkedHashMap<LocatedString, LocatedValue> = LinkedHashMap::new();
            for entry in entries.iter() {
                let key_loc = Loc::new(
                    (entry.key_start, entry.key_end - entry.key_start),
                    src.clone(),
                );
                // Use the already-decoded key bytes from the verified parser
                let decoded_str = std::str::from_utf8(&entry.decoded_key).map_err(|_| {
                    ParseError::invalid_unicode_escape(
                        key_loc.clone(),
                        "Invalid UTF-8 in decoded key",
                    )
                })?;
                let key = LocatedString::new(key_loc, SmolStr::from(decoded_str));
                let value = convert_value(&entry.value, src)?;
                items.insert(key, value);
            }
            Ok(LocatedValue::new_object(items, loc))
        }
    }
}

/// Convert a verified parser error into the main crate's [`ParseError`].
fn convert_parse_error(
    err: &verified_parser::ParseError,
    bytes: &[u8],
    src: &Arc<str>,
) -> ParseError {
    match err {
        verified_parser::ParseError::UnexpectedToken { pos } => {
            if *pos > 0 && *pos < bytes.len() {
                let loc = Loc::new((*pos, 1), src.clone());
                ParseError::unexpected_token(
                    loc,
                    "Expected: value (i.e., null, Bool, Number, String, Array, or Object).",
                )
            } else {
                let p = if bytes.is_empty() { 0 } else { bytes.len() - 1 };
                let loc = Loc::new((p, 0), src.clone());
                ParseError::from(TokenizeError::unexpected_eof(loc, "Expected more input."))
            }
        }
        verified_parser::ParseError::InvalidEscape { pos } => {
            let loc = Loc::new((*pos, 1), src.clone());
            ParseError::invalid_unicode_escape(loc, "Invalid Unicode escape sequence")
        }
        verified_parser::ParseError::DuplicateKey {
            first_pos,
            second_pos,
        } => {
            let first_loc = Loc::new((*first_pos, 1), src.clone());
            let second_loc = Loc::new((*second_pos, 1), src.clone());
            ParseError::duplicate_key(first_loc.into(), second_loc)
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::parser::err::TokenizeError;

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
        assert_matches!(value.get_str(), Some("   \t\n\r\t  "));
        assert_matches!(
            parser.get_value(),
            Err(ParseError::TokenizeError(TokenizeError::UnexpectedEof(..)))
        );
    }

    #[test]
    fn parse_unicode_escape_seq_string_literal() {
        let mut parser = JsonParser::new("\"\\u0912\"");
        let value = parser.get_value().expect("Failed to parse `\"\\u0912\"`");
        assert_matches!(value.get_str(), Some("\u{0912}"));
        assert_matches!(
            parser.get_value(),
            Err(ParseError::TokenizeError(TokenizeError::UnexpectedEof(..)))
        );
    }

    #[test]
    fn parse_all_escape_sequences() {
        let mut parser =
            JsonParser::new(r#""quote:\" backslash:\\ slash:\/ bs:\b ff:\f nl:\n cr:\r tab:\t""#);
        let value = parser
            .get_value()
            .expect("Failed to parse escape sequences");
        assert_eq!(
            value.get_str().unwrap(),
            "quote:\" backslash:\\ slash:/ bs:\u{0008} ff:\u{000C} nl:\n cr:\r tab:\t"
        );
    }

    #[test]
    fn parse_unicode_surrogate_pair() {
        // U+1F600 (😀) encoded as surrogate pair \uD83D\uDE00
        let mut parser = JsonParser::new("\"\\uD83D\\uDE00\"");
        let value = parser.get_value().expect("Failed to parse surrogate pair");
        assert_eq!(value.get_str().unwrap(), "😀");
    }

    #[test]
    fn parse_lone_high_surrogate_rejected() {
        // High surrogate not followed by a low surrogate
        let mut parser = JsonParser::new("\"\\uD83D\\u0041\"");
        assert_matches!(
            parser.get_value(),
            Err(ParseError::InvalidUnicodeEscape(..))
        );
    }

    #[test]
    fn parse_lone_low_surrogate_rejected() {
        // Low surrogate without preceding high surrogate
        let mut parser = JsonParser::new("\"\\uDE00\"");
        assert_matches!(
            parser.get_value(),
            Err(ParseError::InvalidUnicodeEscape(..))
        );
    }

    #[test]
    fn parse_object_key_with_escapes() {
        let mut parser = JsonParser::new(r#"{"key\nname": "value"}"#);
        let value = parser.get_value().expect("Failed to parse object");
        let obj = value.get_object().expect("Expected object");
        // The key should be decoded: "key\nname" (with actual newline)
        assert!(obj.get("key\nname").is_some());
    }

    #[test]
    fn parse_string_no_escapes_unchanged() {
        let mut parser = JsonParser::new("\"hello world\"");
        let value = parser.get_value().expect("Failed to parse");
        assert_eq!(value.get_str().unwrap(), "hello world");
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

    #[test]
    fn parse_fail_duplicate_keys_unicode_escape() {
        // "a" and "\u0061" are the same decoded string
        let mut parser = JsonParser::new(r#"{"\u0061": true, "a": false}"#);
        assert_matches!(parser.get_value(), Err(ParseError::DuplicateKey(..)));
    }

    #[test]
    fn parse_fail_duplicate_keys_backslash_escape() {
        // JSON: {"\\": true, "\u005c": false}
        // Both keys decode to a single backslash character
        let input = "{\"\\\\\":true,\"\\u005c\":false}";
        let mut parser = JsonParser::new(input);
        assert_matches!(parser.get_value(), Err(ParseError::DuplicateKey(..)));
    }
}
