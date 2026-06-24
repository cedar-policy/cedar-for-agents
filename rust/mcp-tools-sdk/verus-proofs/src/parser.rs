use crate::dedup::slices_equal;
use crate::escape::{decode_json_escapes_bytes, DecodeResult};
use crate::tokenizer::{Token, TokenKind};
use vstd::prelude::*;

verus! {

// =============================================================================
// JSON Value tree
// =============================================================================

pub enum JsonValue {
    Null { start: usize, end: usize },
    Bool { val: bool, start: usize, end: usize },
    Number { start: usize, end: usize },
    String { start: usize, end: usize, decoded: Vec<u8> },
    Array { elements: Vec<JsonValue>, start: usize, end: usize },
    Object { entries: Vec<ObjectEntry>, start: usize, end: usize },
}

pub struct ObjectEntry {
    pub key_start: usize,
    pub key_end: usize,
    /// The decoded key bytes (escape sequences resolved to UTF-8)
    pub decoded_key: Vec<u8>,
    pub value: JsonValue,
}

/// Parse error kinds
pub enum ParseError {
    UnexpectedToken { pos: usize },
    InvalidEscape { pos: usize },
    DuplicateKey { first_pos: usize, second_pos: usize },
}

pub enum ParseResult {
    Ok { value: JsonValue, next: usize },
    Err { err: ParseError },
}

// =============================================================================
// Parser
// =============================================================================

enum DecodeStringResult {
    Ok { bytes: Vec<u8> },
    Err { pos: usize },
}

/// Decode a string token (strip quotes, resolve escapes).
fn decode_string_token(input: &[u8], start: usize, end: usize) -> (result: DecodeStringResult)
    requires
        start < end,
        end <= input@.len(),
{
    // String tokens include the quotes: input[start] == '"', input[end-1] == '"'
    if end - start < 2 {
        return DecodeStringResult::Ok { bytes: Vec::new() };
    }
    let content_start = start + 1;
    let content_end = end - 1;
    if content_start > content_end || content_end > input.len() {
        return DecodeStringResult::Ok { bytes: Vec::new() };
    }
    match decode_json_escapes_bytes(input, content_start, content_end) {
        DecodeResult::Ok { bytes } => DecodeStringResult::Ok { bytes },
        DecodeResult::NoEscapes => {
            let mut raw: Vec<u8> = Vec::new();
            let mut k = content_start;
            while k < content_end
                invariant
                    content_start <= k <= content_end,
                    content_end <= input@.len(),
                decreases content_end - k,
            {
                raw.push(input[k]);
                k += 1;
            }
            DecodeStringResult::Ok { bytes: raw }
        }
        DecodeResult::Err { pos } => DecodeStringResult::Err { pos },
    }
}

/// Parse a JSON value. `input` is the raw source bytes (needed for string decoding).
pub fn parse_value(input: &[u8], tokens: &[Token], idx: usize, gas: usize) -> (result: ParseResult)
    requires
        idx <= tokens@.len(),
        forall|i: int| #![auto] 0 <= i && i < tokens@.len() ==>
            tokens@[i].start < tokens@[i].end && tokens@[i].end <= input@.len(),
    ensures
        match result {
            ParseResult::Ok { value: _, next } => {
                next > idx && next <= tokens@.len()
            },
            ParseResult::Err { .. } => true,
        },
    decreases gas, 2nat,
{
    if gas == 0 || idx >= tokens.len() {
        return ParseResult::Err { err: ParseError::UnexpectedToken { pos: 0 } };
    }

    let token = &tokens[idx];
    match token.kind {
        TokenKind::Null => {
            ParseResult::Ok { value: JsonValue::Null { start: token.start, end: token.end }, next: idx + 1 }
        }
        TokenKind::True => {
            ParseResult::Ok { value: JsonValue::Bool { val: true, start: token.start, end: token.end }, next: idx + 1 }
        }
        TokenKind::False => {
            ParseResult::Ok { value: JsonValue::Bool { val: false, start: token.start, end: token.end }, next: idx + 1 }
        }
        TokenKind::Number => {
            ParseResult::Ok { value: JsonValue::Number { start: token.start, end: token.end }, next: idx + 1 }
        }
        TokenKind::String => {
            if token.start >= token.end {
                return ParseResult::Err { err: ParseError::UnexpectedToken { pos: token.start } };
            }
            let decoded = decode_string_token(input, token.start, token.end);
            match decoded {
                DecodeStringResult::Ok { bytes } => {
                    ParseResult::Ok { value: JsonValue::String { start: token.start, end: token.end, decoded: bytes }, next: idx + 1 }
                }
                DecodeStringResult::Err { pos } => {
                    ParseResult::Err { err: ParseError::InvalidEscape { pos } }
                }
            }
        }
        TokenKind::ArrayStart => {
            parse_array_body(input, tokens, idx + 1, gas, token.start)
        }
        TokenKind::ObjectStart => {
            parse_object_body(input, tokens, idx + 1, gas, token.start)
        }
        _ => {
            ParseResult::Err { err: ParseError::UnexpectedToken { pos: token.start } }
        }
    }
}

/// Parse array body after '['.
fn parse_array_body(input: &[u8], tokens: &[Token], cur_start: usize, gas: usize, open_start: usize) -> (result: ParseResult)
    requires
        cur_start <= tokens@.len(),
        cur_start >= 1,
        gas > 0,
        forall|i: int| #![auto] 0 <= i && i < tokens@.len() ==>
            tokens@[i].start < tokens@[i].end && tokens@[i].end <= input@.len(),
    ensures
        match result {
            ParseResult::Ok { value: _, next } => {
                next > cur_start - 1 && next <= tokens@.len()
            },
            ParseResult::Err { .. } => true,
        },
    decreases gas, 1nat,
{
    if cur_start < tokens.len() {
        match tokens[cur_start].kind {
            TokenKind::ArrayEnd => {
                return ParseResult::Ok {
                    value: JsonValue::Array { elements: Vec::new(), start: open_start, end: tokens[cur_start].end },
                    next: cur_start + 1,
                };
            }
            _ => {}
        }
    }

    let mut cur = cur_start;
    let mut elements: Vec<JsonValue> = Vec::new();

    loop
        invariant
            cur_start <= cur <= tokens@.len(),
            cur_start >= 1,
            gas > 0,
            forall|i: int| #![auto] 0 <= i && i < tokens@.len() ==>
                tokens@[i].start < tokens@[i].end && tokens@[i].end <= input@.len(),
        decreases tokens@.len() - cur,
    {
        if cur >= tokens.len() {
            return ParseResult::Err { err: ParseError::UnexpectedToken { pos: 0 } };
        }

        let sub_gas = if gas > 1 { gas - 1 } else { 0 };
        match parse_value(input, tokens, cur, sub_gas) {
            ParseResult::Ok { value, next } => {
                elements.push(value);
                cur = next;
            }
            ParseResult::Err { err } => {
                return ParseResult::Err { err };
            }
        }

        if cur >= tokens.len() {
            return ParseResult::Err { err: ParseError::UnexpectedToken { pos: 0 } };
        }
        match tokens[cur].kind {
            TokenKind::ArrayEnd => {
                return ParseResult::Ok {
                    value: JsonValue::Array { elements, start: open_start, end: tokens[cur].end },
                    next: cur + 1,
                };
            }
            TokenKind::Comma => {
                cur += 1;
            }
            _ => {
                return ParseResult::Err { err: ParseError::UnexpectedToken { pos: tokens[cur].start } };
            }
        }
    }
}

/// Spec: all decoded keys in entries are pairwise distinct
pub open spec fn keys_are_distinct(entries: Seq<ObjectEntry>) -> bool {
    forall|i: int, j: int|
        #![auto]
        0 <= i && i < j && j < entries.len()
        ==> !(entries[i].decoded_key@ =~= entries[j].decoded_key@)
}

/// Parse object body after '{'. Decodes keys and detects duplicates.
fn parse_object_body(input: &[u8], tokens: &[Token], cur_start: usize, gas: usize, open_start: usize) -> (result: ParseResult)
    requires
        cur_start <= tokens@.len(),
        cur_start >= 1,
        gas > 0,
        forall|i: int| #![auto] 0 <= i && i < tokens@.len() ==>
            tokens@[i].start < tokens@[i].end && tokens@[i].end <= input@.len(),
    ensures
        match result {
            ParseResult::Ok { value, next } => {
                next > cur_start - 1 && next <= tokens@.len()
                // Object values have distinct keys
                && (match value {
                    JsonValue::Object { entries, .. } => keys_are_distinct(entries@),
                    _ => true,
                })
            },
            ParseResult::Err { .. } => true,
        },
    decreases gas, 0nat,
{
    if cur_start < tokens.len() {
        match tokens[cur_start].kind {
            TokenKind::ObjectEnd => {
                let empty_entries: Vec<ObjectEntry> = Vec::new();
                assert(keys_are_distinct(empty_entries@));
                return ParseResult::Ok {
                    value: JsonValue::Object { entries: empty_entries, start: open_start, end: tokens[cur_start].end },
                    next: cur_start + 1,
                };
            }
            _ => {}
        }
    }

    let mut cur = cur_start;
    let mut entries: Vec<ObjectEntry> = Vec::new();

    loop
        invariant
            cur_start <= cur <= tokens@.len(),
            cur_start >= 1,
            gas > 0,
            forall|i: int| #![auto] 0 <= i && i < tokens@.len() ==>
                tokens@[i].start < tokens@[i].end && tokens@[i].end <= input@.len(),
            keys_are_distinct(entries@),
        decreases tokens@.len() - cur,
    {
        // Key (string)
        if cur >= tokens.len() {
            return ParseResult::Err { err: ParseError::UnexpectedToken { pos: 0 } };
        }
        let key_start;
        let key_end;
        match tokens[cur].kind {
            TokenKind::String => {
                key_start = tokens[cur].start;
                key_end = tokens[cur].end;
                cur = cur + 1;
            }
            _ => {
                return ParseResult::Err { err: ParseError::UnexpectedToken { pos: tokens[cur].start } };
            }
        }

        // Decode the key (strip quotes, resolve escapes)
        let decoded_key = if key_end >= 2 && key_start <= key_end - 2 {
            let content_start = key_start + 1;
            let content_end = key_end - 1;
            if content_start <= content_end && content_end <= input.len() {
                match decode_json_escapes_bytes(input, content_start, content_end) {
                    DecodeResult::Ok { bytes } => bytes,
                    DecodeResult::NoEscapes => {
                        // Copy raw bytes
                        let mut raw: Vec<u8> = Vec::new();
                        let mut k = content_start;
                        while k < content_end
                            invariant
                                content_start <= k <= content_end,
                                content_end <= input@.len(),
                            decreases content_end - k,
                        {
                            raw.push(input[k]);
                            k = k + 1;
                        }
                        raw
                    }
                    DecodeResult::Err { pos } => {
                        return ParseResult::Err { err: ParseError::InvalidEscape { pos } };
                    }
                }
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };

        // Duplicate key check: compare against all previous decoded keys
        let mut dup_idx: usize = 0;
        let mut found_dup = false;
        while dup_idx < entries.len() && !found_dup
            invariant
                dup_idx <= entries@.len(),
                found_dup ==> dup_idx < entries@.len(),
                // All entries before dup_idx differ from decoded_key
                !found_dup ==> forall|k: int| #![auto] 0 <= k && k < dup_idx as int ==>
                    !(entries@[k].decoded_key@ =~= decoded_key@),
            decreases entries@.len() - dup_idx, (!found_dup) as int,
        {
            if slices_equal(entries[dup_idx].decoded_key.as_slice(), decoded_key.as_slice()) {
                found_dup = true;
            } else {
                dup_idx = dup_idx + 1;
            }
        }
        if found_dup {
            let first_pos = entries[dup_idx].key_start;
            return ParseResult::Err { err: ParseError::DuplicateKey { first_pos, second_pos: key_start } };
        }
        // At this point: decoded_key differs from all existing entries' keys
        assert(forall|k: int| #![auto] 0 <= k && k < entries@.len() ==>
            !(entries@[k].decoded_key@ =~= decoded_key@));

        // Colon
        if cur >= tokens.len() {
            return ParseResult::Err { err: ParseError::UnexpectedToken { pos: 0 } };
        }
        match tokens[cur].kind {
            TokenKind::Colon => {
                cur = cur + 1;
            }
            _ => {
                return ParseResult::Err { err: ParseError::UnexpectedToken { pos: tokens[cur].start } };
            }
        }

        // Value
        if cur > tokens.len() {
            return ParseResult::Err { err: ParseError::UnexpectedToken { pos: 0 } };
        }
        let sub_gas = if gas > 1 { (gas - 1) as usize } else { 0 };
        match parse_value(input, tokens, cur, sub_gas) {
            ParseResult::Ok { value, next } => {
                entries.push(ObjectEntry { key_start, key_end, decoded_key, value });
                cur = next;
            }
            ParseResult::Err { err } => {
                return ParseResult::Err { err };
            }
        }

        // ',' or '}'
        if cur >= tokens.len() {
            return ParseResult::Err { err: ParseError::UnexpectedToken { pos: 0 } };
        }
        match tokens[cur].kind {
            TokenKind::ObjectEnd => {
                return ParseResult::Ok {
                    value: JsonValue::Object { entries, start: open_start, end: tokens[cur].end },
                    next: cur + 1,
                };
            }
            TokenKind::Comma => {
                cur = cur + 1;
            }
            _ => {
                return ParseResult::Err { err: ParseError::UnexpectedToken { pos: tokens[cur].start } };
            }
        }
    }
}

/// Parse a complete JSON document.
pub fn parse(input: &[u8], tokens: &[Token]) -> (result: ParseResult)
    requires
        forall|i: int| #![auto] 0 <= i && i < tokens@.len() ==>
            tokens@[i].start < tokens@[i].end && tokens@[i].end <= input@.len(),
    ensures
        match result {
            ParseResult::Ok { value: _, next } => next <= tokens@.len(),
            ParseResult::Err { .. } => true,
        },
{
    let gas = tokens.len();
    match parse_value(input, tokens, 0, gas) {
        ParseResult::Ok { value, next } => {
            if next == tokens.len() {
                ParseResult::Ok { value, next }
            } else if next < tokens.len() {
                ParseResult::Err { err: ParseError::UnexpectedToken { pos: tokens[next].start } }
            } else {
                ParseResult::Err { err: ParseError::UnexpectedToken { pos: 0 } }
            }
        }
        ParseResult::Err { err } => ParseResult::Err { err },
    }
}

} // verus!
