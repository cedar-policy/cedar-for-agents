use vstd::prelude::*;
use crate::json_spec::*;
use crate::tokenizer::*;

verus! {

// =============================================================================
// Fuel monotonicity lemmas for the parsing spec
//
// These prove that increasing fuel never changes a successful parse result.
// They are mechanical — each mirrors the structure of its corresponding spec
// function — but essential for connecting the exec parser (which uses a fixed
// gas for all siblings) to the spec (which tracks fuel).
// =============================================================================

/// Lemma: if spec_parse_value succeeds with fuel `f`, it returns the same result
/// with any fuel `f' >= f`.
pub proof fn lemma_parse_value_fuel_mono(input: Seq<u8>, tokens: Seq<Token>, idx: nat, f: nat, f2: nat)
    requires
        f <= f2,
        spec_parse_value(input, tokens, idx, f) is Some,
    ensures
        spec_parse_value(input, tokens, idx, f2) == spec_parse_value(input, tokens, idx, f),
    decreases f, tokens.len() - idx, 2nat,
{
    if f == 0 || idx >= tokens.len() {
        return;
    }
    let token = tokens[idx as int];
    match token.kind {
        TokenKind::Null | TokenKind::True | TokenKind::False | TokenKind::Number => {},
        TokenKind::String => {},
        TokenKind::ArrayStart => {
            lemma_parse_array_fuel_mono(input, tokens, idx + 1, f, f2);
        },
        TokenKind::ObjectStart => {
            lemma_parse_object_fuel_mono(input, tokens, idx + 1, f, f2);
        },
        _ => {},
    }
}

/// Lemma: fuel monotonicity for spec_parse_array.
pub proof fn lemma_parse_array_fuel_mono(input: Seq<u8>, tokens: Seq<Token>, idx: nat, f: nat, f2: nat)
    requires
        f <= f2,
        spec_parse_array(input, tokens, idx, f) is Some,
    ensures
        spec_parse_array(input, tokens, idx, f2) == spec_parse_array(input, tokens, idx, f),
    decreases f, tokens.len() - idx, 1nat,
{
    if f == 0 || idx >= tokens.len() {
        return;
    }
    if tokens[idx as int].kind == (TokenKind::ArrayEnd) {
        return;
    }
    lemma_parse_array_elements_fuel_mono(input, tokens, idx, Seq::empty(), f, f2);
}

/// Lemma: fuel monotonicity for spec_parse_array_elements.
pub proof fn lemma_parse_array_elements_fuel_mono(
    input: Seq<u8>, tokens: Seq<Token>, idx: nat, acc: Seq<JsonValueSpec>, f: nat, f2: nat,
)
    requires
        f <= f2,
        spec_parse_array_elements(input, tokens, idx, acc, f) is Some,
    ensures
        spec_parse_array_elements(input, tokens, idx, acc, f2)
            == spec_parse_array_elements(input, tokens, idx, acc, f),
    decreases f, tokens.len() - idx, 0nat,
{
    if f == 0 {
        return;
    }
    lemma_parse_value_fuel_mono(input, tokens, idx, (f - 1) as nat, (f2 - 1) as nat);
    match spec_parse_value(input, tokens, idx, (f - 1) as nat) {
        None => {},
        Some((val, next)) => {
            let new_acc = acc + seq![val];
            if next >= tokens.len() {
            } else if tokens[next as int].kind == (TokenKind::ArrayEnd) {
            } else if tokens[next as int].kind == (TokenKind::Comma) {
                if next + 1 <= idx {
                } else {
                    lemma_parse_array_elements_fuel_mono(input, tokens, next + 1, new_acc, f, f2);
                }
            }
        },
    }
}

/// Lemma: fuel monotonicity for spec_parse_object.
pub proof fn lemma_parse_object_fuel_mono(input: Seq<u8>, tokens: Seq<Token>, idx: nat, f: nat, f2: nat)
    requires
        f <= f2,
        spec_parse_object(input, tokens, idx, f) is Some,
    ensures
        spec_parse_object(input, tokens, idx, f2) == spec_parse_object(input, tokens, idx, f),
    decreases f, tokens.len() - idx, 1nat,
{
    if f == 0 || idx >= tokens.len() {
        return;
    }
    if tokens[idx as int].kind == (TokenKind::ObjectEnd) {
        return;
    }
    lemma_parse_object_members_fuel_mono(input, tokens, idx, Seq::empty(), f, f2);
}

/// Lemma: fuel monotonicity for spec_parse_object_members.
pub proof fn lemma_parse_object_members_fuel_mono(
    input: Seq<u8>, tokens: Seq<Token>, idx: nat, acc: Seq<(Seq<u8>, JsonValueSpec)>, f: nat, f2: nat,
)
    requires
        f <= f2,
        spec_parse_object_members(input, tokens, idx, acc, f) is Some,
    ensures
        spec_parse_object_members(input, tokens, idx, acc, f2)
            == spec_parse_object_members(input, tokens, idx, acc, f),
    decreases f, tokens.len() - idx, 0nat,
{
    if f == 0 || idx >= tokens.len() {
        return;
    }
    if !(tokens[idx as int].kind == (TokenKind::String)) {
        return;
    }
    let key_token = tokens[idx as int];
    match spec_decode_string_token(input, key_token.start as nat, key_token.end as nat) {
        None => {},
        Some(decoded_key) => {
            if spec_key_exists(acc, decoded_key) {
                return;
            }
            let after_key = idx + 1;
            if after_key >= tokens.len() {
                return;
            }
            if !(tokens[after_key as int].kind == (TokenKind::Colon)) {
                return;
            }
            let after_colon = after_key + 1;
            lemma_parse_value_fuel_mono(input, tokens, after_colon, (f - 1) as nat, (f2 - 1) as nat);
            match spec_parse_value(input, tokens, after_colon, (f - 1) as nat) {
                None => {},
                Some((val, next)) => {
                    let new_acc = acc + seq![(decoded_key, val)];
                    if next >= tokens.len() {
                    } else if tokens[next as int].kind == (TokenKind::ObjectEnd) {
                    } else if tokens[next as int].kind == (TokenKind::Comma) {
                        if next + 1 <= idx {
                        } else {
                            lemma_parse_object_members_fuel_mono(input, tokens, next + 1, new_acc, f, f2);
                        }
                    }
                },
            }
        },
    }
}

} // verus!
