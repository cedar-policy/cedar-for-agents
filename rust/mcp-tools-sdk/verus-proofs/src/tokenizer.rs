use crate::common_specs::*;
use vstd::prelude::*;

verus! {

// =============================================================================
// Skip whitespace
// =============================================================================

/// Spec: the index of the first non-whitespace byte at or after `pos`
pub open spec fn spec_skip_whitespace(input: Seq<u8>, pos: nat) -> nat
    decreases input.len() - pos,
{
    if pos >= input.len() {
        pos
    } else if spec_is_whitespace(input[pos as int]) {
        spec_skip_whitespace(input, pos + 1)
    } else {
        pos
    }
}

/// Proof: skip_whitespace always returns a position >= the input position
proof fn lemma_skip_whitespace_geq(input: Seq<u8>, pos: nat)
    ensures
        spec_skip_whitespace(input, pos) >= pos,
    decreases input.len() - pos,
{
    if pos < input.len() && spec_is_whitespace(input[pos as int]) {
        lemma_skip_whitespace_geq(input, pos + 1);
    }
}

/// Proof: skip_whitespace returns a position <= input.len()
proof fn lemma_skip_whitespace_bounded(input: Seq<u8>, pos: nat)
    requires
        pos <= input.len(),
    ensures
        spec_skip_whitespace(input, pos) <= input.len(),
    decreases input.len() - pos,
{
    if pos < input.len() && spec_is_whitespace(input[pos as int]) {
        lemma_skip_whitespace_bounded(input, pos + 1);
    }
}

/// Proof: the byte at the position returned by skip_whitespace is not whitespace
/// (if it's within bounds)
proof fn lemma_skip_whitespace_result_not_ws(input: Seq<u8>, pos: nat)
    requires
        pos <= input.len(),
    ensures ({
        let result = spec_skip_whitespace(input, pos);
        result < input.len() ==> !spec_is_whitespace(input[result as int])
    }),
    decreases input.len() - pos,
{
    if pos < input.len() && spec_is_whitespace(input[pos as int]) {
        lemma_skip_whitespace_result_not_ws(input, pos + 1);
    }
}

/// Proof: all bytes in [pos, skip_whitespace(pos)) are whitespace
proof fn lemma_skip_whitespace_all_ws(input: Seq<u8>, pos: nat)
    requires
        pos <= input.len(),
    ensures
        forall|k: nat| pos <= k && k < spec_skip_whitespace(input, pos) ==>
            spec_is_whitespace(#[trigger] input[k as int]),
    decreases input.len() - pos,
{
    if pos < input.len() && spec_is_whitespace(input[pos as int]) {
        lemma_skip_whitespace_all_ws(input, pos + 1);
    }
}

/// Exec: advance past whitespace, returning the index of first non-whitespace byte
pub fn skip_whitespace(input: &[u8], pos: usize) -> (result: usize)
    requires
        pos <= input@.len(),
    ensures
        result == spec_skip_whitespace(input@, pos as nat),
        result >= pos,
        result <= input@.len(),
        result < input@.len() ==> !spec_is_whitespace(input@[result as int]),
{
    proof {
        lemma_skip_whitespace_geq(input@, pos as nat);
        lemma_skip_whitespace_bounded(input@, pos as nat);
        lemma_skip_whitespace_result_not_ws(input@, pos as nat);
    }
    let mut i = pos;
    while i < input.len() && is_whitespace(input[i])
        invariant
            pos <= i <= input.len(),
            spec_skip_whitespace(input@, pos as nat) == spec_skip_whitespace(input@, i as nat),
        decreases input.len() - i,
    {
        i = i + 1;
    }
    i
}

// =============================================================================
// Consume digits (for number literal parsing)
// =============================================================================

/// Spec: the index after consuming a run of ASCII digits starting at `pos`
pub open spec fn spec_consume_digits(input: Seq<u8>, pos: nat) -> nat
    decreases input.len() - pos,
{
    if pos >= input.len() {
        pos
    } else if spec_is_ascii_digit(input[pos as int]) {
        spec_consume_digits(input, pos + 1)
    } else {
        pos
    }
}

/// Proof: consume_digits returns a position >= the input position
proof fn lemma_consume_digits_geq(input: Seq<u8>, pos: nat)
    ensures
        spec_consume_digits(input, pos) >= pos,
    decreases input.len() - pos,
{
    if pos < input.len() && spec_is_ascii_digit(input[pos as int]) {
        lemma_consume_digits_geq(input, pos + 1);
    }
}

/// Proof: consume_digits returns a position <= input.len()
proof fn lemma_consume_digits_bounded(input: Seq<u8>, pos: nat)
    requires
        pos <= input.len(),
    ensures
        spec_consume_digits(input, pos) <= input.len(),
    decreases input.len() - pos,
{
    if pos < input.len() && spec_is_ascii_digit(input[pos as int]) {
        lemma_consume_digits_bounded(input, pos + 1);
    }
}

/// Proof: all bytes in [pos, consume_digits(pos)) are ASCII digits
proof fn lemma_consume_digits_all_digits(input: Seq<u8>, pos: nat)
    requires
        pos <= input.len(),
    ensures
        forall|i: nat| pos <= i && i < spec_consume_digits(input, pos) ==>
            spec_is_ascii_digit(#[trigger] input[i as int]),
    decreases input.len() - pos,
{
    if pos < input.len() && spec_is_ascii_digit(input[pos as int]) {
        lemma_consume_digits_all_digits(input, pos + 1);
    }
}

/// Proof: the byte at consume_digits(pos) is not a digit (if within bounds).
/// This is the "stopping condition" — the scan stopped because it hit a non-digit.
proof fn lemma_consume_digits_stops_at_non_digit(input: Seq<u8>, pos: nat)
    requires
        pos <= input.len(),
    ensures ({
        let result = spec_consume_digits(input, pos);
        result < input.len() ==> !spec_is_ascii_digit(input[result as int])
    }),
    decreases input.len() - pos,
{
    if pos < input.len() && spec_is_ascii_digit(input[pos as int]) {
        lemma_consume_digits_stops_at_non_digit(input, pos + 1);
    }
}

/// Exec: consume a run of ASCII digits, returning the position after the last digit
pub fn consume_digits(input: &[u8], pos: usize) -> (result: usize)
    requires
        pos <= input@.len(),
    ensures
        result == spec_consume_digits(input@, pos as nat),
        result >= pos,
        result <= input@.len(),
        // All consumed bytes are digits
        forall|i: nat| pos as nat <= i && i < result as nat ==>
            spec_is_ascii_digit(#[trigger] input@[i as int]),
{
    proof {
        lemma_consume_digits_geq(input@, pos as nat);
        lemma_consume_digits_bounded(input@, pos as nat);
        lemma_consume_digits_all_digits(input@, pos as nat);
    }
    let mut i = pos;
    while i < input.len() && is_ascii_digit(input[i])
        invariant
            pos <= i <= input.len(),
            spec_consume_digits(input@, pos as nat) == spec_consume_digits(input@, i as nat),
            forall|j: nat| pos as nat <= j && j < i as nat ==>
                spec_is_ascii_digit(#[trigger] input@[j as int]),
        decreases input.len() - i,
    {
        i = i + 1;
    }
    i
}

// =============================================================================
// Consume hex digits (for \uXXXX escape sequences)
// =============================================================================

/// Exec: check and consume exactly 4 hex digits. Returns Some(pos+4) on success.
pub fn consume_four_hex_digits(input: &[u8], pos: usize) -> (result: Option<usize>)
    requires
        pos <= input@.len(),
    ensures
        match result {
            Some(end) => {
                end == pos + 4
                && spec_is_hex_quad(input@, pos as nat)
            },
            None => !spec_is_hex_quad(input@, pos as nat),
        },
{
    if input.len() - pos < 4 {
        return None;
    }
    if is_hex_digit(input[pos]) && is_hex_digit(input[pos + 1])
        && is_hex_digit(input[pos + 2]) && is_hex_digit(input[pos + 3])
    {
        Some(pos + 4)
    } else {
        None
    }
}

// =============================================================================
// Number literal parsing (RFC 8259 §6)
// Grammar: number = [ "-" ] int [ frac ] [ exp ]
//          int    = "0" / ( digit1-9 *digit )
//          frac   = "." 1*digit
//          exp    = ("e"/"E") ["+"/"-"] 1*digit
// =============================================================================

/// Result of consuming a number literal: Ok(end_position) or Err(position of error)
pub enum NumberResult {
    Ok { end: usize },
    Err { pos: usize },
}

/// Consume the integer part of a number literal starting at `pos`.
/// Expects pos to point at the first digit (after optional '-').
/// Returns the position after the integer part, or None on error.
pub fn consume_int_part(input: &[u8], pos: usize) -> (result: Option<usize>)
    requires
        pos <= input@.len(),
    ensures
        match result {
            Some(end) => pos < end && end <= input@.len(),
            None => true,
        },
{
    if pos >= input.len() {
        return None;
    }
    if input[pos] == 0x30 {
        // '0': must not be followed by another digit (no leading zeros)
        let next = pos + 1;
        if next < input.len() && is_ascii_digit(input[next]) {
            None // leading zero error
        } else {
            Some(next)
        }
    } else if input[pos] >= 0x31 && input[pos] <= 0x39 {
        // '1'-'9': consume remaining digits
        let end = consume_digits(input, pos + 1);
        Some(end)
    } else {
        None // not a digit
    }
}

/// Consume the fractional part of a number literal, if present.
/// `pos` should point at the potential '.'.
/// Returns the position after the fractional part (unchanged if no '.' present).
pub fn consume_frac_part(input: &[u8], pos: usize) -> (result: Option<usize>)
    requires
        pos <= input@.len(),
    ensures
        match result {
            Some(end) => pos <= end && end <= input@.len(),
            None => true,
        },
{
    if pos >= input.len() || input[pos] != 0x2E {
        // no '.' — no fractional part, that's fine
        return Some(pos);
    }
    // We have '.', must be followed by at least one digit
    let after_dot = pos + 1;
    if after_dot >= input.len() || !is_ascii_digit(input[after_dot]) {
        return None; // '.' not followed by digit
    }
    let end = consume_digits(input, after_dot);
    Some(end)
}

/// Consume the exponent part of a number literal, if present.
/// `pos` should point at the potential 'e'/'E'.
/// Returns the position after the exponent part (unchanged if no 'e'/'E').
pub fn consume_exp_part(input: &[u8], pos: usize) -> (result: Option<usize>)
    requires
        pos <= input@.len(),
    ensures
        match result {
            Some(end) => pos <= end && end <= input@.len(),
            None => true,
        },
{
    if pos >= input.len() || (input[pos] != 0x65 && input[pos] != 0x45) {
        // no 'e' or 'E'
        return Some(pos);
    }
    let mut cur = pos + 1;
    // optional sign
    if cur < input.len() && (input[cur] == 0x2B || input[cur] == 0x2D) {
        cur = cur + 1;
    }
    // must have at least one digit
    if cur >= input.len() || !is_ascii_digit(input[cur]) {
        return None;
    }
    let end = consume_digits(input, cur);
    Some(end)
}

/// Consume a complete JSON number literal.
/// `pos` is the start position (may point at '-' or first digit).
/// Returns Ok(end) where end is position after the number, or Err on invalid input.
pub fn consume_number(input: &[u8], pos: usize) -> (result: NumberResult)
    requires
        pos <= input@.len(),
    ensures
        match result {
            NumberResult::Ok { end } => pos < end && end <= input@.len(),
            NumberResult::Err { .. } => true,
        },
{
    let mut cur = pos;

    // Optional leading '-'
    if cur < input.len() && input[cur] == 0x2D {
        cur = cur + 1;
    }

    // Integer part (required)
    cur = match consume_int_part(input, cur) {
        Some(end) => end,
        None => return NumberResult::Err { pos: cur },
    };

    // Fractional part (optional)
    cur = match consume_frac_part(input, cur) {
        Some(end) => end,
        None => return NumberResult::Err { pos: cur },
    };

    // Exponent part (optional)
    cur = match consume_exp_part(input, cur) {
        Some(end) => end,
        None => return NumberResult::Err { pos: cur },
    };

    // Must have consumed at least one byte beyond the starting position
    if cur > pos {
        NumberResult::Ok { end: cur }
    } else {
        NumberResult::Err { pos }
    }
}

// =============================================================================
// String literal parsing (RFC 8259 §7)
// Expects pos to point just AFTER the opening '"'.
// Returns position just AFTER the closing '"'.
// =============================================================================

/// Result of consuming a string literal
pub enum StringResult {
    Ok { end: usize },
    UnterminatedString,
    InvalidEscape { pos: usize },
}

/// Consume a JSON string literal body (after opening '"').
/// Returns the position just after the closing '"'.
pub fn consume_string(input: &[u8], pos: usize) -> (result: StringResult)
    requires
        pos <= input@.len(),
    ensures
        match result {
            StringResult::Ok { end } => pos <= end && end <= input@.len(),
            _ => true,
        },
{
    let mut i = pos;
    while i < input.len()
        invariant
            pos <= i <= input.len(),
        decreases input.len() - i,
    {
        let b = input[i];
        if b == 0x22 {
            // closing '"'
            return StringResult::Ok { end: i + 1 };
        } else if b == 0x5C {
            // backslash — escape sequence
            i += 1;
            if i >= input.len() {
                return StringResult::UnterminatedString;
            }
            let esc = input[i];
            if is_simple_escape(esc) {
                i += 1;
            } else if esc == 0x75 {
                // 'u' — unicode escape \uXXXX
                i += 1;
                match consume_four_hex_digits(input, i) {
                    Some(end) => { i = end; }
                    None => { return StringResult::InvalidEscape { pos: i }; }
                }
            } else {
                return StringResult::InvalidEscape { pos: i };
            }
        } else if b < 0x20 {
            // control characters not allowed in JSON strings
            return StringResult::InvalidEscape { pos: i };
        } else {
            i += 1;
        }
    }
    StringResult::UnterminatedString
}

// =============================================================================
// Keyword matching (true, false, null)
// =============================================================================

/// Consume a known keyword starting at `pos`.
/// Returns Some(pos + keyword.len()) if the bytes match, None otherwise.
pub fn consume_keyword(input: &[u8], pos: usize, keyword: &[u8]) -> (result: Option<usize>)
    requires
        pos <= input@.len(),
        keyword@.len() > 0,
    ensures
        match result {
            Some(end) => {
                end == pos + keyword@.len()
                && end <= input@.len()
                && input@.subrange(pos as int, end as int) =~= keyword@
            },
            None => true,
        },
{
    if input.len() - pos < keyword.len() {
        return None;
    }
    let end = pos + keyword.len();
    let mut i = 0;
    while i < keyword.len()
        invariant
            0 <= i <= keyword.len(),
            pos + keyword.len() <= input.len(),
            end == pos + keyword.len(),
            forall|j: nat| j < i as nat ==> input@[(pos + j) as int] == keyword@[j as int],
        decreases keyword.len() - i,
    {
        if input[pos + i] != keyword[i] {
            return None;
        }
        i += 1;
    }
    assert(input@.subrange(pos as int, end as int) =~= keyword@);
    Some(end)
}

// =============================================================================
// Token types
// =============================================================================

/// The kind of a JSON token
pub enum TokenKind {
    Null,
    True,
    False,
    Number,
    String,
    ArrayStart,   // [
    ArrayEnd,     // ]
    ObjectStart,  // {
    ObjectEnd,    // }
    Comma,        // ,
    Colon,        // :
}

/// A token with its kind and span [start, end) in the input
pub struct Token {
    pub kind: TokenKind,
    pub start: usize,
    pub end: usize,
}

/// Result of get_token: either a token, EOF, or an error at a position
pub enum TokenResult {
    Ok { token: Token },
    Eof,
    ErrUnexpectedEof { pos: usize },
    ErrInvalidNumber { pos: usize },
    ErrInvalidEscape { pos: usize },
    ErrUnexpectedToken { pos: usize },
}

// =============================================================================
// Token content specification
// =============================================================================

/// Spec: a token's content is consistent with its kind and the input bytes.
///
/// This connects token *kinds* to their *content*, ensuring that:
/// - Keyword tokens (`true`, `false`, `null`) have exactly the expected bytes
/// - String tokens are delimited by `"` (opening quote at start)
/// - Single-character structural tokens have the correct byte
/// - Number tokens start with '-' or a digit
pub open spec fn token_content_valid(token: Token, input: Seq<u8>) -> bool {
    // Common structural requirement: non-empty span within bounds
    &&& token.start < token.end
    &&& token.end <= input.len()
    // Kind-specific content constraints:
    &&& match token.kind {
        TokenKind::True => {
            token.end - token.start == 4
            && input[token.start as int] == LOWER_T()
            && input[(token.start + 1) as int] == LOWER_R()
            && input[(token.start + 2) as int] == LOWER_U()
            && input[(token.start + 3) as int] == LOWER_E()
        },
        TokenKind::False => {
            token.end - token.start == 5
            && input[token.start as int] == LOWER_F()
            && input[(token.start + 1) as int] == LOWER_A()
            && input[(token.start + 2) as int] == LOWER_L()
            && input[(token.start + 3) as int] == LOWER_S()
            && input[(token.start + 4) as int] == LOWER_E()
        },
        TokenKind::Null => {
            token.end - token.start == 4
            && input[token.start as int] == LOWER_N()
            && input[(token.start + 1) as int] == LOWER_U()
            && input[(token.start + 2) as int] == LOWER_L()
            && input[(token.start + 3) as int] == LOWER_L()
        },
        TokenKind::String => {
            input[token.start as int] == QUOTE()
        },
        TokenKind::Number => {
            input[token.start as int] == DASH()
            || spec_is_ascii_digit(input[token.start as int])
        },
        TokenKind::ArrayStart  => token.end - token.start == 1 && input[token.start as int] == LBRACKET(),
        TokenKind::ArrayEnd    => token.end - token.start == 1 && input[token.start as int] == RBRACKET(),
        TokenKind::ObjectStart => token.end - token.start == 1 && input[token.start as int] == LBRACE(),
        TokenKind::ObjectEnd   => token.end - token.start == 1 && input[token.start as int] == RBRACE(),
        TokenKind::Comma       => token.end - token.start == 1 && input[token.start as int] == COMMA(),
        TokenKind::Colon       => token.end - token.start == 1 && input[token.start as int] == COLON(),
    }
}

// =============================================================================
// get_token: the main tokenizer dispatch
// =============================================================================

/// Consume one JSON token from `input` starting at position `pos`.
///
/// Properties proven:
/// - **Progress**: if Ok, token.end > pos (strictly advances)
/// - **Bounded**: token.start and token.end are within [pos, input.len()]
/// - **Non-overlapping**: token.start >= pos, so successive calls from
///   the previous token.end produce non-overlapping spans
/// - **Content validity**: token kind matches the actual bytes at the span
///   (keywords have exact bytes, strings start with `"`, etc.)
pub fn get_token(input: &[u8], pos: usize) -> (result: TokenResult)
    requires
        pos <= input@.len(),
    ensures
        match result {
            TokenResult::Ok { token } => {
                token.end > pos
                && token.start >= pos
                && token.end <= input@.len()
                && token.end > token.start
                && token_content_valid(token, input@)
                // Bytes between pos and token.start are all whitespace
                && (forall|k: int| pos <= k < token.start ==>
                    spec_is_whitespace(#[trigger] input@[k]))
            },
            TokenResult::Eof => {
                // All remaining bytes from pos are whitespace (nothing left but space)
                forall|k: int| pos <= k < input@.len() ==>
                    spec_is_whitespace(#[trigger] input@[k])
            },
            TokenResult::ErrUnexpectedEof { .. } => true,
            TokenResult::ErrInvalidNumber { .. } => true,
            TokenResult::ErrInvalidEscape { .. } => true,
            TokenResult::ErrUnexpectedToken { .. } => true,
        },
{
    proof {
        lemma_skip_whitespace_all_ws(input@, pos as nat);
    }
    let start = skip_whitespace(input, pos);

    if start >= input.len() {
        return TokenResult::Eof;
    }

    let b = input[start];

    // Single-character tokens
    if b == 0x5B { // [
        return TokenResult::Ok { token: Token { kind: TokenKind::ArrayStart, start, end: start + 1 } };
    }
    if b == 0x5D { // ]
        return TokenResult::Ok { token: Token { kind: TokenKind::ArrayEnd, start, end: start + 1 } };
    }
    if b == 0x7B { // {
        return TokenResult::Ok { token: Token { kind: TokenKind::ObjectStart, start, end: start + 1 } };
    }
    if b == 0x7D { // }
        return TokenResult::Ok { token: Token { kind: TokenKind::ObjectEnd, start, end: start + 1 } };
    }
    if b == 0x2C { // ,
        return TokenResult::Ok { token: Token { kind: TokenKind::Comma, start, end: start + 1 } };
    }
    if b == 0x3A { // :
        return TokenResult::Ok { token: Token { kind: TokenKind::Colon, start, end: start + 1 } };
    }

    // Keywords
    if b == 0x74 { // 't' -> true
        let kw: [u8; 4] = [0x74, 0x72, 0x75, 0x65]; // "true"
        match consume_keyword(input, start, kw.as_slice()) {
            Some(end) => return TokenResult::Ok { token: Token { kind: TokenKind::True, start, end } },
            None => {
                if input.len() - start < 4 {
                    return TokenResult::ErrUnexpectedEof { pos: start };
                }
                return TokenResult::ErrUnexpectedToken { pos: start };
            }
        }
    }
    if b == 0x66 { // 'f' -> false
        let kw: [u8; 5] = [0x66, 0x61, 0x6C, 0x73, 0x65]; // "false"
        match consume_keyword(input, start, kw.as_slice()) {
            Some(end) => return TokenResult::Ok { token: Token { kind: TokenKind::False, start, end } },
            None => {
                if input.len() - start < 5 {
                    return TokenResult::ErrUnexpectedEof { pos: start };
                }
                return TokenResult::ErrUnexpectedToken { pos: start };
            }
        }
    }
    if b == 0x6E { // 'n' -> null
        let kw: [u8; 4] = [0x6E, 0x75, 0x6C, 0x6C]; // "null"
        match consume_keyword(input, start, kw.as_slice()) {
            Some(end) => return TokenResult::Ok { token: Token { kind: TokenKind::Null, start, end } },
            None => {
                if input.len() - start < 4 {
                    return TokenResult::ErrUnexpectedEof { pos: start };
                }
                return TokenResult::ErrUnexpectedToken { pos: start };
            }
        }
    }

    // Number: starts with '-' or digit
    if b == 0x2D || (0x30..=0x39).contains(&b) {
        match consume_number(input, start) {
            NumberResult::Ok { end } => return TokenResult::Ok { token: Token { kind: TokenKind::Number, start, end } },
            NumberResult::Err { pos: err_pos } => {
                // If error pos is at or past the end, it's EOF
                if err_pos >= input.len() {
                    return TokenResult::ErrUnexpectedEof { pos: err_pos };
                }
                return TokenResult::ErrInvalidNumber { pos: err_pos };
            }
        }
    }

    // String: starts with '"'
    if b == 0x22 {
        match consume_string(input, start + 1) {
            StringResult::Ok { end } => return TokenResult::Ok { token: Token { kind: TokenKind::String, start, end } },
            StringResult::UnterminatedString => return TokenResult::ErrUnexpectedEof { pos: start },
            StringResult::InvalidEscape { pos: err_pos } => return TokenResult::ErrInvalidEscape { pos: err_pos },
        }
    }

    // Unrecognized byte
    TokenResult::ErrUnexpectedToken { pos: start }
}

// =============================================================================
// tokenize_all: repeated get_token proving non-overlapping and termination
// =============================================================================

/// Error from tokenization, preserving both kind and position.
pub enum TokenizeError {
    UnexpectedEof { pos: usize },
    InvalidNumber { pos: usize },
    InvalidEscape { pos: usize },
    UnexpectedToken { pos: usize },
}

/// Tokenize the entire input, collecting tokens into a Vec.
///
/// Proven properties:
/// - Terminates (position strictly increases each iteration)
/// - All token spans are within bounds
/// - Tokens are non-overlapping and ordered (each starts >= previous end)
/// - Every token's content is valid for its kind (keywords match bytes, etc.)
/// - Gaps between tokens (and before first / after last) are all whitespace
pub fn tokenize_all(input: &[u8]) -> (result: Result<Vec<Token>, TokenizeError>)
    ensures
        match result {
            Ok(tokens) => {
                // All tokens have valid, non-empty spans within the input
                &&& forall|i: int| 0 <= i && i < tokens@.len() ==> {
                    let t = #[trigger] tokens@[i];
                    t.start < t.end && t.end <= input@.len()
                }
                // Tokens are ordered and non-overlapping
                &&& forall|i: int, j: int| 0 <= i && i < j && j < tokens@.len() ==> {
                    (#[trigger] tokens@[i]).end <= (#[trigger] tokens@[j]).start
                }
                // Every token's kind matches the actual input bytes at its span
                &&& forall|i: int| 0 <= i && i < tokens@.len() ==> {
                    token_content_valid(#[trigger] tokens@[i], input@)
                }
                // Gaps between tokens are all whitespace
                &&& forall|i: int| #[trigger] tokens@[i].start >= 0 && 0 <= i && i < tokens@.len() ==>
                    forall|k: int| #![auto]
                        (if i == 0 { 0int } else { tokens@[i - 1].end as int }) <= k
                        && k < tokens@[i].start
                        ==> spec_is_whitespace(input@[k])
                // Trailing bytes after the last token are all whitespace
                &&& forall|k: int|
                    (if tokens@.len() > 0 { tokens@[tokens@.len() - 1].end as int } else { 0int }) <= k
                    && k < input@.len()
                    ==> spec_is_whitespace(#[trigger] input@[k])
            },
            Err(_) => true,
        },
{
    let mut tokens: Vec<Token> = Vec::new();
    let mut pos: usize = 0;

    while pos <= input.len()
        invariant
            pos <= input.len(),
            forall|i: int| 0 <= i && i < tokens@.len() ==> {
                let t = #[trigger] tokens@[i];
                t.start < t.end && t.end <= input@.len()
            },
            forall|i: int, j: int| 0 <= i && i < j && j < tokens@.len() ==> {
                (#[trigger] tokens@[i]).end <= (#[trigger] tokens@[j]).start
            },
            forall|i: int| 0 <= i && i < tokens@.len() ==> {
                token_content_valid(#[trigger] tokens@[i], input@)
            },
            tokens@.len() > 0 ==> tokens@[tokens@.len() - 1].end <= pos,
            // Bytes in [0, first_token.start) and between consecutive tokens are whitespace.
            // Expressed as: for each token i, bytes in [prev_end, token_i.start) are whitespace.
            forall|i: int| #[trigger] tokens@[i].start >= 0 && 0 <= i && i < tokens@.len() ==>
                forall|k: int| #![auto]
                    (if i == 0 { 0int } else { tokens@[i - 1].end as int }) <= k
                    && k < tokens@[i].start
                    ==> spec_is_whitespace(input@[k]),
            // Everything in [last_token.end, pos) is whitespace
            forall|k: int|
                (if tokens@.len() > 0 { tokens@[tokens@.len() - 1].end as int } else { 0int }) <= k
                && k < pos
                ==> spec_is_whitespace(#[trigger] input@[k]),
        decreases input.len() - pos,
    {
        match get_token(input, pos) {
            TokenResult::Ok { token } => {
                let new_pos = token.end;
                tokens.push(token);
                pos = new_pos;
            }
            TokenResult::Eof => {
                return Ok(tokens);
            }
            TokenResult::ErrUnexpectedEof { pos: p } => {
                return Err(TokenizeError::UnexpectedEof { pos: p });
            }
            TokenResult::ErrInvalidNumber { pos: p } => {
                return Err(TokenizeError::InvalidNumber { pos: p });
            }
            TokenResult::ErrInvalidEscape { pos: p } => {
                return Err(TokenizeError::InvalidEscape { pos: p });
            }
            TokenResult::ErrUnexpectedToken { pos: p } => {
                return Err(TokenizeError::UnexpectedToken { pos: p });
            }
        }
    }
    Ok(tokens)
}

} // verus!
