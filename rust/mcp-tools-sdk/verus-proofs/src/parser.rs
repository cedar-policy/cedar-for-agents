use crate::dedup::slices_equal;
use crate::escape::{decode_json_escapes_bytes, DecodeResult};
use crate::json_spec::*;
use crate::tokenizer::{tokenize_all, Token, TokenKind, TokenizeError};
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
/// Proven to match `spec_decode_string_token`: on Ok the decoded bytes equal
/// the spec's escape-decoded content; on Err the spec also fails.
fn decode_string_token(input: &[u8], start: usize, end: usize) -> (result: DecodeStringResult)
    requires
        start < end,
        end <= input@.len(),
    ensures
        match result {
            DecodeStringResult::Ok { bytes } =>
                spec_decode_string_token(input@, start as nat, end as nat) == Some(bytes@),
            DecodeStringResult::Err { .. } =>
                spec_decode_string_token(input@, start as nat, end as nat) is None,
        },
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
            // NoEscapes means spec_decode == Some(input[content_start..content_end])
            let mut raw: Vec<u8> = Vec::new();
            let mut k = content_start;
            while k < content_end
                invariant
                    content_start <= k <= content_end,
                    content_end <= input@.len(),
                    raw@ =~= input@.subrange(content_start as int, k as int),
                decreases content_end - k,
            {
                raw.push(input[k]);
                proof {
                    assert(raw@ =~= input@.subrange(content_start as int, (k + 1) as int));
                }
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
            ParseResult::Ok { value, next } => {
                &&& next > idx && next <= tokens@.len()
                &&& spec_parse_value(input@, tokens@, idx as nat, gas as nat) is Some
                &&& spec_parse_value(input@, tokens@, idx as nat, gas as nat).unwrap().1 == next as nat
                &&& value_matches_spec(value,
                        spec_parse_value(input@, tokens@, idx as nat, gas as nat).unwrap().0,
                        input@)
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
            proof {
                assert(spec_parse_value(input@, tokens@, idx as nat, gas as nat)
                    == Some((JsonValueSpec::Null, (idx + 1) as nat)));
            }
            ParseResult::Ok { value: JsonValue::Null { start: token.start, end: token.end }, next: idx + 1 }
        }
        TokenKind::True => {
            proof {
                assert(spec_parse_value(input@, tokens@, idx as nat, gas as nat)
                    == Some((JsonValueSpec::Bool { val: true }, (idx + 1) as nat)));
            }
            ParseResult::Ok { value: JsonValue::Bool { val: true, start: token.start, end: token.end }, next: idx + 1 }
        }
        TokenKind::False => {
            proof {
                assert(spec_parse_value(input@, tokens@, idx as nat, gas as nat)
                    == Some((JsonValueSpec::Bool { val: false }, (idx + 1) as nat)));
            }
            ParseResult::Ok { value: JsonValue::Bool { val: false, start: token.start, end: token.end }, next: idx + 1 }
        }
        TokenKind::Number => {
            proof {
                assert(spec_parse_value(input@, tokens@, idx as nat, gas as nat)
                    == Some((JsonValueSpec::Number {
                        bytes: input@.subrange(token.start as int, token.end as int),
                    }, (idx + 1) as nat)));
            }
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
            ParseResult::Ok { value, next } => {
                &&& next > cur_start - 1 && next <= tokens@.len()
                &&& spec_parse_array(input@, tokens@, cur_start as nat, gas as nat) is Some
                &&& spec_parse_array(input@, tokens@, cur_start as nat, gas as nat).unwrap().1 == next as nat
                &&& value_matches_spec(value,
                        spec_parse_array(input@, tokens@, cur_start as nat, gas as nat).unwrap().0,
                        input@)
            },
            ParseResult::Err { .. } => true,
        },
    decreases gas, 1nat,
{
    if cur_start < tokens.len() {
        match tokens[cur_start].kind {
            TokenKind::ArrayEnd => {
                proof {
                    // spec_parse_array with ArrayEnd at idx returns empty array
                    assert(spec_parse_array(input@, tokens@, cur_start as nat, gas as nat)
                        == Some((JsonValueSpec::Array { elements: Seq::empty() }, (cur_start + 1) as nat)));
                }
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
    let ghost mut spec_acc: Seq<JsonValueSpec> = Seq::empty();

    loop
        invariant
            cur_start <= cur <= tokens@.len(),
            cur_start >= 1,
            gas > 0,
            forall|i: int| #![auto] 0 <= i && i < tokens@.len() ==>
                tokens@[i].start < tokens@[i].end && tokens@[i].end <= input@.len(),
            // The exec elements match the spec accumulator
            elements@.len() == spec_acc.len(),
            forall|i: int| 0 <= i && i < elements@.len() ==>
                value_matches_spec(#[trigger] elements@[i], spec_acc[i], input@),
            // Connection: spec from current position with accumulated elements
            // equals the overall spec_parse_array result
            spec_parse_array_elements(input@, tokens@, cur as nat, spec_acc, gas as nat)
                == spec_parse_array(input@, tokens@, cur_start as nat, gas as nat),
        decreases tokens@.len() - cur,
    {
        if cur >= tokens.len() {
            return ParseResult::Err { err: ParseError::UnexpectedToken { pos: 0 } };
        }

        let sub_gas = if gas > 1 { gas - 1 } else { 0 };
        match parse_value(input, tokens, cur, sub_gas) {
            ParseResult::Ok { value, next } => {
                proof {
                    // sub_gas == gas - 1, matching the fuel-1 used by spec_parse_array_elements
                    let spec_val = spec_parse_value(input@, tokens@, cur as nat, sub_gas as nat).unwrap().0;
                    spec_acc = spec_acc + seq![spec_val];
                }
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
                &&& next > cur_start - 1 && next <= tokens@.len()
                &&& (match value {
                    JsonValue::Object { entries, .. } => keys_are_distinct(entries@),
                    _ => true,
                })
                &&& spec_parse_object(input@, tokens@, cur_start as nat, gas as nat) is Some
                &&& spec_parse_object(input@, tokens@, cur_start as nat, gas as nat).unwrap().1 == next as nat
                &&& value_matches_spec(value,
                        spec_parse_object(input@, tokens@, cur_start as nat, gas as nat).unwrap().0,
                        input@)
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
                proof {
                    assert(spec_parse_object(input@, tokens@, cur_start as nat, gas as nat)
                        == Some((JsonValueSpec::Object { entries: Seq::empty() }, (cur_start + 1) as nat)));
                }
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
    let ghost mut spec_acc: Seq<(Seq<u8>, JsonValueSpec)> = Seq::empty();

    loop
        invariant
            cur_start <= cur <= tokens@.len(),
            cur_start >= 1,
            gas > 0,
            forall|i: int| #![auto] 0 <= i && i < tokens@.len() ==>
                tokens@[i].start < tokens@[i].end && tokens@[i].end <= input@.len(),
            keys_are_distinct(entries@),
            // Exec entries match spec accumulator
            entries@.len() == spec_acc.len(),
            forall|i: int| 0 <= i && i < entries@.len() ==> {
                let e = #[trigger] entries@[i];
                let s = spec_acc[i];
                e.decoded_key@ =~= s.0
                && value_matches_spec(e.value, s.1, input@)
            },
            // Connection: spec from current position equals overall result
            spec_parse_object_members(input@, tokens@, cur as nat, spec_acc, gas as nat)
                == spec_parse_object(input@, tokens@, cur_start as nat, gas as nat),
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
        let decoded_key = match decode_string_token(input, key_start, key_end) {
            DecodeStringResult::Ok { bytes } => bytes,
            DecodeStringResult::Err { pos } => {
                return ParseResult::Err { err: ParseError::InvalidEscape { pos } };
            }
        };
        proof {
            assert(spec_decode_string_token(input@, key_start as nat, key_end as nat) == Some(decoded_key@));
        }

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
        proof {
            // Connect to spec: !spec_key_exists(spec_acc, decoded_key@)
            // We know entries@[i].decoded_key@ =~= spec_acc[i].0 for all i (invariant)
            // We know entries@[i].decoded_key@ != decoded_key@ for all i (from dup check)
            // Therefore spec_acc[i].0 != decoded_key@ for all i
            assert forall|i: int| 0 <= i && i < spec_acc.len()
                implies !(#[trigger] spec_acc[i].0 =~= decoded_key@)
            by {
                assert(entries@[i].decoded_key@ =~= spec_acc[i].0);
                assert(!(entries@[i].decoded_key@ =~= decoded_key@));
            }
            assert(!spec_key_exists(spec_acc, decoded_key@));
        }

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
                proof {
                    let spec_val = spec_parse_value(input@, tokens@, cur as nat, sub_gas as nat).unwrap().0;
                    spec_acc = spec_acc + seq![(decoded_key@, spec_val)];
                }
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

/// Parse a complete JSON document from tokens.
fn parse(input: &[u8], tokens: &[Token]) -> (result: ParseResult)
    requires
        forall|i: int| #![auto] 0 <= i && i < tokens@.len() ==>
            tokens@[i].start < tokens@[i].end && tokens@[i].end <= input@.len(),
    ensures
        match result {
            ParseResult::Ok { value, next } => {
                &&& next == tokens@.len()
                &&& spec_parse_json(input@, tokens@) is Some
                &&& value_matches_spec(value, spec_parse_json(input@, tokens@).unwrap(), input@)
            },
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

// =============================================================================
// Top-level entry point: tokenize + parse in one step
// =============================================================================

/// Error from `parse_json`: either a tokenization error or a parse error.
#[allow(inconsistent_fields)]
pub enum ParseJsonError {
    Tokenize { err: TokenizeError },
    Parse { err: ParseError },
}

/// Tokenize and parse a complete JSON document from raw bytes.
///
/// This is the primary entry point that combines tokenization and parsing
/// in verified code, eliminating the need for unverified glue between the
/// tokenizer and parser.
///
/// Functional correctness: on success, the returned value matches the
/// mathematical specification `spec_parse_json` applied to the tokenized input.
pub fn parse_json(input: &[u8]) -> (result: Result<JsonValue, ParseJsonError>)
    ensures
        match result {
            Ok(value) => exists|tokens: Seq<Token>|
                spec_parse_json(input@, tokens) is Some
                && value_matches_spec(value, #[trigger] spec_parse_json(input@, tokens).unwrap(), input@),
            Err(_) => true,
        },
{
    let tokens = match tokenize_all(input) {
        Ok(tokens) => tokens,
        Err(err) => {
            return Err(ParseJsonError::Tokenize { err });
        }
    };
    match parse(input, tokens.as_slice()) {
        ParseResult::Ok { value, next: _ } => Ok(value),
        ParseResult::Err { err } => Err(ParseJsonError::Parse { err }),
    }
}

} // verus!
