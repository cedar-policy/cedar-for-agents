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

pub(crate) struct JsonParser {
    tokenizer: Tokenizer,
}

impl JsonParser {
    pub(crate) fn new(input: &str) -> Self {
        Self {
            tokenizer: Tokenizer::new(input),
        }
    }

    fn get_object(&mut self, loc: Loc) -> Result<LocatedValue, ParseError> {
        let mut items = LinkedHashMap::new();
        let mut maybe_empty = true;

        loop {
            let token = self.tokenizer.get_token()?;
            match token.kind() {
                TokenKind::String => {
                    maybe_empty = false;
                    let key = LocatedString::new(token.to_loc());
                    let token = self.tokenizer.get_token()?;
                    if matches!(token.kind(), TokenKind::Colon) {
                        let value = self.get_value()?;
                        // Check if duplicate value
                        match items.entry(key.clone()) {
                            Entry::Occupied(occ) => {
                                return Err(ParseError::duplicate_key(
                                    occ.key().as_loc().into(),
                                    key.to_loc(),
                                ))
                            }
                            Entry::Vacant(vac) => {
                                vac.insert(value);
                            }
                        }
                    } else {
                        return Err(ParseError::unexpected_token(token.to_loc(), "Expected `:`"));
                    }
                    // Finished parsing key-value pair. Check for end or prepare for next key-value pair
                    let token = self.tokenizer.get_token()?;
                    match token.kind() {
                        TokenKind::ObjectEnd => {
                            let start = loc.start();
                            let end = token.to_loc().end();
                            return Ok(LocatedValue::new_object(
                                items,
                                loc.span((start, end - start)),
                            ));
                        }
                        TokenKind::Comma => (),
                        _ => {
                            return Err(ParseError::unexpected_token(
                                token.to_loc(),
                                "Expected: `,` or `}`.",
                            ))
                        }
                    }
                }
                // Allow for an empty object
                TokenKind::ObjectEnd if maybe_empty => {
                    let start = loc.start();
                    let end = token.to_loc().end();
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
                    return Err(ParseError::unexpected_token(token.to_loc(), msg));
                }
            }
        }
    }

    fn get_array(&mut self, loc: Loc) -> Result<LocatedValue, ParseError> {
        let mut items = Vec::new();
        let mut maybe_empty = true;

        loop {
            let token = self.tokenizer.get_token()?;

            let item = match token.kind() {
                TokenKind::Null => LocatedValue::new_null(token.to_loc()),
                TokenKind::Bool(b) => LocatedValue::new_bool(b, token.to_loc()),
                TokenKind::Number => LocatedValue::new_number(token.to_loc()),
                TokenKind::String => LocatedValue::new_string(token.to_loc()),
                TokenKind::ArrayStart => self.get_array(token.to_loc())?,
                TokenKind::ObjectStart => self.get_object(token.to_loc())?,
                TokenKind::ArrayEnd if maybe_empty => {
                    let start = loc.start();
                    let end = token.to_loc().end();
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
                    return Err(ParseError::unexpected_token(token.to_loc(), msg));
                }
            };
            maybe_empty = false;

            items.push(item);

            let token = self.tokenizer.get_token()?;
            match token.kind() {
                TokenKind::Comma => (),
                TokenKind::ArrayEnd => {
                    let start = loc.start();
                    let end = token.to_loc().end();
                    return Ok(LocatedValue::new_array(
                        items,
                        loc.span((start, end - start)),
                    ));
                }
                _ => {
                    return Err(ParseError::unexpected_token(
                        token.to_loc(),
                        "Expected: `]` or `,`.",
                    ))
                }
            }
        }
    }

    pub(crate) fn get_value(&mut self) -> Result<LocatedValue, ParseError> {
        let token = self.tokenizer.get_token()?;
        match token.kind() {
            TokenKind::Null => Ok(LocatedValue::new_null(token.to_loc())),
            TokenKind::Bool(b) => Ok(LocatedValue::new_bool(b, token.to_loc())),
            TokenKind::Number => Ok(LocatedValue::new_number(token.to_loc())),
            TokenKind::String => Ok(LocatedValue::new_string(token.to_loc())),
            TokenKind::ArrayStart => self.get_array(token.to_loc()),
            TokenKind::ObjectStart => self.get_object(token.to_loc()),
            _ => Err(ParseError::unexpected_token(
                token.to_loc(),
                "Expected: value (i.e., null, Bool, Number, String, Array, or Object).",
            )),
        }
    }
}
