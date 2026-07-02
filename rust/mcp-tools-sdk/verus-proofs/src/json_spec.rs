use vstd::prelude::*;
use crate::byte_specs::*;
use crate::escape::*;
use crate::parser::{JsonValue, ObjectEntry};
use crate::tokenizer::*;

verus! {

// =============================================================================
// JSON Value Specification (RFC 8259 §3)
//
// Abstract JSON value type — pure mathematical structure without positions
// or runtime details. This is what a correct parser *should* produce.
// =============================================================================

/// Spec: an abstract JSON value (RFC 8259 §3).
///
/// This mirrors the grammar:
///   value = false / null / true / object / array / number / string
///
/// Numbers are left as raw byte spans (we don't interpret numeric values).
/// Strings are decoded byte sequences (escapes resolved).
/// Objects are sequences of (key, value) pairs with unique keys.
/// Arrays are sequences of values.
pub enum JsonValueSpec {
    Null,
    Bool { val: bool },
    /// Number: raw bytes of the number literal (already proven valid RFC 8259 §6)
    Number { bytes: Seq<u8> },
    /// String: decoded content bytes (escape sequences resolved to UTF-8)
    String { decoded: Seq<u8> },
    /// Array: ordered sequence of values (RFC 8259 §5)
    Array { elements: Seq<JsonValueSpec> },
    /// Object: ordered sequence of (decoded_key, value) pairs with unique keys (RFC 8259 §4)
    Object { entries: Seq<(Seq<u8>, JsonValueSpec)> },
}

// =============================================================================
// Spec: parsing a value from a token stream
//
// This defines the *mathematical meaning* of parsing: given input bytes and
// a token sequence (produced by the verified tokenizer), what abstract value
// should result?
//
// The spec operates on tokens (not raw bytes) because the tokenizer is already
// proven to correctly segment the input. The parser's job is to recognize
// structure (nesting via [ ] { } and commas) and decode string content.
// =============================================================================

/// Spec: decode a string token's content.
/// Given a String token spanning input[start..end], the decoded content is
/// the escape-decoded bytes of input[start+1..end-1] (stripping quotes).
pub open spec fn spec_decode_string_token(input: Seq<u8>, start: nat, end: nat) -> Option<Seq<u8>>
    recommends start < end && end <= input.len(),
{
    if end - start < 2 {
        Some(Seq::empty())
    } else {
        spec_decode(input, (start + 1) as nat, (end - 1) as nat)
    }
}

/// Spec: parse one JSON value from tokens starting at index `idx`.
/// Returns `Some((value, next_idx))` on success, `None` on failure.
///
/// Uses `fuel` to ensure termination. Sufficient fuel always exists for
/// well-formed token streams (every recursive call advances idx strictly).
pub open spec fn spec_parse_value(input: Seq<u8>, tokens: Seq<Token>, idx: nat, fuel: nat) -> Option<(JsonValueSpec, nat)>
    decreases fuel, tokens.len() - idx, 2nat,
{
    if fuel == 0 || idx >= tokens.len() {
        None
    } else {
        let token = tokens[idx as int];
        match token.kind {
            TokenKind::Null => Some((JsonValueSpec::Null, idx + 1)),
            TokenKind::True => Some((JsonValueSpec::Bool { val: true }, idx + 1)),
            TokenKind::False => Some((JsonValueSpec::Bool { val: false }, idx + 1)),
            TokenKind::Number => Some((
                JsonValueSpec::Number { bytes: input.subrange(token.start as int, token.end as int) },
                idx + 1,
            )),
            TokenKind::String => {
                match spec_decode_string_token(input, token.start as nat, token.end as nat) {
                    Some(decoded) => Some((JsonValueSpec::String { decoded }, idx + 1)),
                    None => None,
                }
            },
            TokenKind::ArrayStart => spec_parse_array(input, tokens, idx + 1, fuel),
            TokenKind::ObjectStart => spec_parse_object(input, tokens, idx + 1, fuel),
            // Structural tokens (], }, ,, :) in value position → error
            _ => None,
        }
    }
}

/// Spec: parse array elements after '['. Looks for ']' (empty) or value *( ',' value ) ']'.
/// Returns Some((Array { elements }, next_idx_after_']')) or None.
pub open spec fn spec_parse_array(input: Seq<u8>, tokens: Seq<Token>, idx: nat, fuel: nat) -> Option<(JsonValueSpec, nat)>
    decreases fuel, tokens.len() - idx, 1nat,
{
    if fuel == 0 || idx >= tokens.len() {
        None
    } else if tokens[idx as int].kind == (TokenKind::ArrayEnd) {
        // Empty array
        Some((JsonValueSpec::Array { elements: Seq::empty() }, idx + 1))
    } else {
        spec_parse_array_elements(input, tokens, idx, Seq::empty(), fuel)
    }
}

/// Spec: parse comma-separated array elements, accumulating into `acc`.
/// Fuel decreases only for nested value parsing, not for sibling iteration.
pub open spec fn spec_parse_array_elements(
    input: Seq<u8>, tokens: Seq<Token>, idx: nat, acc: Seq<JsonValueSpec>, fuel: nat,
) -> Option<(JsonValueSpec, nat)>
    decreases fuel, tokens.len() - idx, 0nat,
{
    if fuel == 0 {
        None
    } else {
        match spec_parse_value(input, tokens, idx, (fuel - 1) as nat) {
            None => None,
            Some((val, next)) => {
                let new_acc = acc + seq![val];
                if next >= tokens.len() {
                    None
                } else if tokens[next as int].kind == (TokenKind::ArrayEnd) {
                    Some((JsonValueSpec::Array { elements: new_acc }, next + 1))
                } else if tokens[next as int].kind == (TokenKind::Comma) {
                    if next + 1 <= idx {
                        None // no progress — shouldn't happen with valid tokens
                    } else {
                        spec_parse_array_elements(input, tokens, next + 1, new_acc, fuel)
                    }
                } else {
                    None
                }
            },
        }
    }
}

/// Spec: parse object members after '{'. Looks for '}' (empty) or member *( ',' member ) '}'.
pub open spec fn spec_parse_object(input: Seq<u8>, tokens: Seq<Token>, idx: nat, fuel: nat) -> Option<(JsonValueSpec, nat)>
    decreases fuel, tokens.len() - idx, 1nat,
{
    if fuel == 0 || idx >= tokens.len() {
        None
    } else if tokens[idx as int].kind == (TokenKind::ObjectEnd) {
        // Empty object
        Some((JsonValueSpec::Object { entries: Seq::empty() }, idx + 1))
    } else {
        spec_parse_object_members(input, tokens, idx, Seq::empty(), fuel)
    }
}

/// Spec: parse comma-separated object members, accumulating into `acc`.
/// Each member is: string ':' value
/// Rejects duplicate keys.
/// Fuel decreases only for nested value parsing, not for sibling iteration.
pub open spec fn spec_parse_object_members(
    input: Seq<u8>, tokens: Seq<Token>, idx: nat, acc: Seq<(Seq<u8>, JsonValueSpec)>, fuel: nat,
) -> Option<(JsonValueSpec, nat)>
    decreases fuel, tokens.len() - idx, 0nat,
{
    if fuel == 0 || idx >= tokens.len() {
        None
    } else if !(tokens[idx as int].kind == (TokenKind::String)) {
        None
    } else {
        let key_token = tokens[idx as int];
        match spec_decode_string_token(input, key_token.start as nat, key_token.end as nat) {
            None => None,
            Some(decoded_key) => {
                // Check for duplicate key
                if spec_key_exists(acc, decoded_key) {
                    None
                } else {
                    let after_key = idx + 1;
                    // Colon
                    if after_key >= tokens.len() {
                        None
                    } else if !(tokens[after_key as int].kind == (TokenKind::Colon)) {
                        None
                    } else {
                        let after_colon = after_key + 1;
                        // Value
                        match spec_parse_value(input, tokens, after_colon, (fuel - 1) as nat) {
                            None => None,
                            Some((val, next)) => {
                                let new_acc = acc + seq![(decoded_key, val)];
                                if next >= tokens.len() {
                                    None
                                } else if tokens[next as int].kind == (TokenKind::ObjectEnd) {
                                    Some((JsonValueSpec::Object { entries: new_acc }, next + 1))
                                } else if tokens[next as int].kind == (TokenKind::Comma) {
                                    if next + 1 <= idx {
                                        None // no progress
                                    } else {
                                        spec_parse_object_members(input, tokens, next + 1, new_acc, fuel)
                                    }
                                } else {
                                    None
                                }
                            },
                        }
                    }
                }
            },
        }
    }
}

/// Spec: does a key already exist in the accumulated entries?
pub open spec fn spec_key_exists(entries: Seq<(Seq<u8>, JsonValueSpec)>, key: Seq<u8>) -> bool {
    exists|i: int| 0 <= i && i < entries.len() && entries[i].0 =~= key
}

// =============================================================================
// Relating exec values to spec values
// =============================================================================

/// Spec: an exec JsonValue matches a spec JsonValueSpec given the input bytes.
/// This is the core correctness relation that the parser must satisfy.
pub open spec fn value_matches_spec(v: JsonValue, s: JsonValueSpec, input: Seq<u8>) -> bool
    decreases v,
{
    match (v, s) {
        (JsonValue::Null { .. }, JsonValueSpec::Null) => true,
        (JsonValue::Bool { val: v_val, .. }, JsonValueSpec::Bool { val: s_val }) => v_val == s_val,
        (JsonValue::Number { start, end, .. }, JsonValueSpec::Number { bytes }) => {
            start < end && end <= input.len()
            && bytes =~= input.subrange(start as int, end as int)
        },
        (JsonValue::String { decoded, .. }, JsonValueSpec::String { decoded: s_decoded }) => {
            decoded@ =~= s_decoded
        },
        (JsonValue::Array { elements, .. }, JsonValueSpec::Array { elements: s_elements }) => {
            elements@.len() == s_elements.len()
            && (forall|i: int| 0 <= i && i < elements@.len() ==>
                value_matches_spec(#[trigger] elements@[i], s_elements[i], input))
        },
        (JsonValue::Object { entries, .. }, JsonValueSpec::Object { entries: s_entries }) => {
            entries@.len() == s_entries.len()
            && (forall|i: int| 0 <= i && i < entries@.len() ==> {
                let e = #[trigger] entries@[i];
                let s = s_entries[i];
                e.decoded_key@ =~= s.0
                && value_matches_spec(e.value, s.1, input)
            })
        },
        _ => false,
    }
}

/// Spec: a complete JSON document is ws + value + ws (RFC 8259 §2).
/// Since our tokenizer already strips whitespace, this just means:
/// parse a single value that consumes ALL tokens.
/// Uses tokens.len() as fuel (sufficient for any well-formed input).
pub open spec fn spec_parse_json(input: Seq<u8>, tokens: Seq<Token>) -> Option<JsonValueSpec> {
    match spec_parse_value(input, tokens, 0, tokens.len()) {
        Some((val, next)) => if next == tokens.len() { Some(val) } else { None },
        None => None,
    }
}

} // verus!
