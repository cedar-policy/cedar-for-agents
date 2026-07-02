use vstd::prelude::*;

verus! {

// =============================================================================
// Named ASCII byte constants for use in specifications
//
// These are open spec functions purely for readability.
// Verus inlines their bodies during verification.
// We use all caps to denote spec constants.
// =============================================================================

// --- Whitespace ---
pub open spec fn SPACE() -> u8 { 0x20 }
pub open spec fn TAB() -> u8 { 0x09 }
pub open spec fn NEWLINE() -> u8 { 0x0A }
pub open spec fn CR() -> u8 { 0x0D }

// --- Structural (JSON) ---
pub open spec fn QUOTE() -> u8 { 0x22 }
pub open spec fn COMMA() -> u8 { 0x2C }
pub open spec fn COLON() -> u8 { 0x3A }
pub open spec fn LBRACKET() -> u8 { 0x5B }
pub open spec fn RBRACKET() -> u8 { 0x5D }
pub open spec fn LBRACE() -> u8 { 0x7B }
pub open spec fn RBRACE() -> u8 { 0x7D }

// --- Escape-related ---
pub open spec fn BACKSLASH() -> u8 { 0x5C }
pub open spec fn SLASH() -> u8 { 0x2F }
pub open spec fn BACKSPACE() -> u8 { 0x08 }
pub open spec fn FORMFEED() -> u8 { 0x0C }

// --- Arithmetic / number-related ---
pub open spec fn DASH() -> u8 { 0x2D }
pub open spec fn PLUS() -> u8 { 0x2B }
pub open spec fn DOT() -> u8 { 0x2E }
pub open spec fn ZERO() -> u8 { 0x30 }
pub open spec fn NINE() -> u8 { 0x39 }

// --- Letters (for keywords and hex) ---
pub open spec fn UPPER_A() -> u8 { 0x41 }
pub open spec fn UPPER_E() -> u8 { 0x45 }
pub open spec fn UPPER_F() -> u8 { 0x46 }
pub open spec fn LOWER_A() -> u8 { 0x61 }
pub open spec fn LOWER_B() -> u8 { 0x62 }
pub open spec fn LOWER_E() -> u8 { 0x65 }
pub open spec fn LOWER_F() -> u8 { 0x66 }
pub open spec fn LOWER_L() -> u8 { 0x6C }
pub open spec fn LOWER_N() -> u8 { 0x6E }
pub open spec fn LOWER_R() -> u8 { 0x72 }
pub open spec fn LOWER_S() -> u8 { 0x73 }
pub open spec fn LOWER_T() -> u8 { 0x74 }
pub open spec fn LOWER_U() -> u8 { 0x75 }

// =============================================================================
// Character classification specs
// =============================================================================

/// Spec: byte is an ASCII digit '0'-'9'
pub open spec fn spec_is_ascii_digit(b: u8) -> bool {
    ZERO() <= b && b <= NINE()
}

/// Spec: byte is a hex digit (0-9, a-f, A-F)
pub open spec fn spec_is_hex_digit(b: u8) -> bool {
    (ZERO() <= b && b <= NINE())
    || (LOWER_A() <= b && b <= LOWER_F())
    || (UPPER_A() <= b && b <= UPPER_F())
}

/// Spec: byte is JSON whitespace (RFC 8259 §2: space, tab, newline, carriage return)
pub open spec fn spec_is_whitespace(b: u8) -> bool {
    b == SPACE() || b == TAB() || b == NEWLINE() || b == CR()
}

/// Spec: byte is a valid simple escape character after the backslash (RFC 8259 §7)
pub open spec fn spec_is_simple_escape(b: u8) -> bool {
    b == QUOTE()
    || b == BACKSLASH()
    || b == SLASH()
    || b == LOWER_B()
    || b == LOWER_F()
    || b == LOWER_N()
    || b == LOWER_R()
    || b == LOWER_T()
}

/// Spec: value of a hex digit byte (0-15), or 0 if not a hex digit.
/// Only meaningful when called on bytes known to be hex digits.
pub open spec fn spec_hex_val(b: u8) -> u8 {
    if ZERO() <= b && b <= NINE() { (b - ZERO()) as u8 }
    else if LOWER_A() <= b && b <= LOWER_F() { (b - LOWER_A() + 10) as u8 }
    else if UPPER_A() <= b && b <= UPPER_F() { (b - UPPER_A() + 10) as u8 }
    else { 0 }
}

/// Spec: 4 consecutive bytes starting at `pos` are all hex digits.
pub open spec fn spec_is_hex_quad(input: Seq<u8>, pos: nat) -> bool {
    pos + 4 <= input.len()
    && spec_is_hex_digit(input[pos as int])
    && spec_is_hex_digit(input[(pos + 1) as int])
    && spec_is_hex_digit(input[(pos + 2) as int])
    && spec_is_hex_digit(input[(pos + 3) as int])
}

// =============================================================================
// Unicode and UTF-8 encoding
//
// References:
//   - Unicode scalar values: The Unicode Standard, Chapter 3, §D76
//     https://www.unicode.org/versions/Unicode15.0.0/ch03.pdf
//   - UTF-8 encoding: RFC 3629 https://www.rfc-editor.org/rfc/rfc3629
//   - Surrogate pairs in JSON: RFC 8259 §7 https://www.rfc-editor.org/rfc/rfc8259#section-7
// =============================================================================

// --- Unicode code point boundaries ---

/// Maximum valid Unicode code point (U+10FFFF).
pub open spec fn MAX_CODE_POINT() -> u32 { 0x10FFFF }

/// First code point in the high surrogate range (U+D800).
pub open spec fn HIGH_SURROGATE_MIN() -> u32 { 0xD800 }

/// Last code point in the high surrogate range (U+DBFF).
pub open spec fn HIGH_SURROGATE_MAX() -> u32 { 0xDBFF }

/// First code point in the low surrogate range (U+DC00).
pub open spec fn LOW_SURROGATE_MIN() -> u32 { 0xDC00 }

/// Last code point in the low surrogate range (U+DFFF).
pub open spec fn LOW_SURROGATE_MAX() -> u32 { 0xDFFF }

/// Spec: code point is a high surrogate (U+D800..U+DBFF).
pub open spec fn is_high_surrogate(cp: u32) -> bool {
    HIGH_SURROGATE_MIN() <= cp && cp <= HIGH_SURROGATE_MAX()
}

/// Spec: code point is a low surrogate (U+DC00..U+DFFF).
pub open spec fn is_low_surrogate(cp: u32) -> bool {
    LOW_SURROGATE_MIN() <= cp && cp <= LOW_SURROGATE_MAX()
}

/// Spec: code point is any surrogate (high or low).
pub open spec fn is_surrogate(cp: u32) -> bool {
    HIGH_SURROGATE_MIN() <= cp && cp <= LOW_SURROGATE_MAX()
}

/// A Unicode scalar value: a code point that is not a surrogate (RFC 3629 §3).
pub open spec fn is_unicode_scalar(cp: u32) -> bool {
    cp <= MAX_CODE_POINT() && !is_surrogate(cp)
}

/// Combine a surrogate pair into a supplementary code point (RFC 8259 §7).
/// Formula: (hi - 0xD800) * 0x400 + (lo - 0xDC00) + 0x10000
pub open spec fn surrogate_pair_value(hi: u32, lo: u32) -> u32
    recommends is_high_surrogate(hi) && is_low_surrogate(lo),
{
    (0x10000u32 + (hi - HIGH_SURROGATE_MIN()) * 0x400 + (lo - LOW_SURROGATE_MIN())) as u32
}

/// Spec: the UTF-8 byte encoding of a single code point.
/// This is the readable, mathematical definition.
pub open spec fn spec_encode_code_point(cp: u32) -> Seq<u8>
    recommends is_unicode_scalar(cp),
{
    if cp <= 0x7F {
        seq![cp as u8]
    } else if cp <= 0x7FF {
        seq![
            (0xC0 | (cp >> 6)) as u8,
            (0x80 | (cp & 0x3F)) as u8
        ]
    } else if cp <= 0xFFFF {
        seq![
            (0xE0 | (cp >> 12)) as u8,
            (0x80 | ((cp >> 6) & 0x3F)) as u8,
            (0x80 | (cp & 0x3F)) as u8
        ]
    } else {
        seq![
            (0xF0 | (cp >> 18)) as u8,
            (0x80 | ((cp >> 12) & 0x3F)) as u8,
            (0x80 | ((cp >> 6) & 0x3F)) as u8,
            (0x80 | (cp & 0x3F)) as u8
        ]
    }
}

/// Spec: the encoded length for a code point.
pub open spec fn spec_encoded_len(cp: u32) -> nat
    recommends is_unicode_scalar(cp),
{
    if cp <= 0x7F { 1 }
    else if cp <= 0x7FF { 2 }
    else if cp <= 0xFFFF { 3 }
    else { 4 }
}

/// Spec: decode a UTF-8 byte sequence back into a code point.
/// Assumes the input is a well-formed single-character UTF-8 encoding.
/// Returns 0 for malformed input (only meaningful when called on valid encodings).
pub open spec fn spec_decode_code_point(bytes: Seq<u8>) -> u32 {
    if bytes.len() == 1 {
        bytes[0] as u32
    } else if bytes.len() == 2 {
        (((bytes[0] & 0x1F) as u32) << 6)
        | ((bytes[1] & 0x3F) as u32)
    } else if bytes.len() == 3 {
        (((bytes[0] & 0x0F) as u32) << 12)
        | (((bytes[1] & 0x3F) as u32) << 6)
        | ((bytes[2] & 0x3F) as u32)
    } else if bytes.len() == 4 {
        (((bytes[0] & 0x07) as u32) << 18)
        | (((bytes[1] & 0x3F) as u32) << 12)
        | (((bytes[2] & 0x3F) as u32) << 6)
        | ((bytes[3] & 0x3F) as u32)
    } else {
        0
    }
}

// =============================================================================
// UTF-8 encoding/decoding theorems
// =============================================================================

/// Theorem: encoding length matches spec_encoded_len.
proof fn lemma_encode_len(cp: u32)
    requires
        is_unicode_scalar(cp),
    ensures
        spec_encode_code_point(cp).len() == spec_encoded_len(cp),
{
}

/// Helper: the 2-byte encode→decode identity holds at the bit level.
proof fn lemma_2byte_roundtrip(cp: u32)
    requires
        0x80 <= cp && cp <= 0x7FF,
    ensures
        ((((0xC0u32 | (cp >> 6)) as u8) & 0x1F) as u32) << 6
        | ((((0x80u32 | (cp & 0x3F)) as u8) & 0x3F) as u32)
        == cp,
{
    assert(
        ((((0xC0u32 | (cp >> 6)) as u8) & 0x1Fu8) as u32) << 6u32
        | ((((0x80u32 | (cp & 0x3F)) as u8) & 0x3Fu8) as u32)
        == cp
    ) by (bit_vector)
        requires 0x80u32 <= cp && cp <= 0x7FFu32;
}

/// Helper: the 3-byte encode→decode identity holds at the bit level.
proof fn lemma_3byte_roundtrip(cp: u32)
    requires
        0x800 <= cp && cp <= 0xFFFF,
    ensures
        ((((0xE0u32 | (cp >> 12)) as u8) & 0x0F) as u32) << 12
        | ((((0x80u32 | ((cp >> 6) & 0x3F)) as u8) & 0x3F) as u32) << 6
        | ((((0x80u32 | (cp & 0x3F)) as u8) & 0x3F) as u32)
        == cp,
{
    assert(
        ((((0xE0u32 | (cp >> 12)) as u8) & 0x0Fu8) as u32) << 12u32
        | ((((0x80u32 | ((cp >> 6) & 0x3F)) as u8) & 0x3Fu8) as u32) << 6u32
        | ((((0x80u32 | (cp & 0x3F)) as u8) & 0x3Fu8) as u32)
        == cp
    ) by (bit_vector)
        requires 0x800u32 <= cp && cp <= 0xFFFFu32;
}

/// Helper: the 4-byte encode→decode identity holds at the bit level.
proof fn lemma_4byte_roundtrip(cp: u32)
    requires
        0x10000 <= cp && cp <= 0x10FFFF,
    ensures
        ((((0xF0u32 | (cp >> 18)) as u8) & 0x07) as u32) << 18
        | ((((0x80u32 | ((cp >> 12) & 0x3F)) as u8) & 0x3F) as u32) << 12
        | ((((0x80u32 | ((cp >> 6) & 0x3F)) as u8) & 0x3F) as u32) << 6
        | ((((0x80u32 | (cp & 0x3F)) as u8) & 0x3F) as u32)
        == cp,
{
    assert(
        ((((0xF0u32 | (cp >> 18)) as u8) & 0x07u8) as u32) << 18u32
        | ((((0x80u32 | ((cp >> 12) & 0x3F)) as u8) & 0x3Fu8) as u32) << 12u32
        | ((((0x80u32 | ((cp >> 6) & 0x3F)) as u8) & 0x3Fu8) as u32) << 6u32
        | ((((0x80u32 | (cp & 0x3F)) as u8) & 0x3Fu8) as u32)
        == cp
    ) by (bit_vector)
        requires 0x10000u32 <= cp && cp <= 0x10FFFFu32;
}

/// Theorem: encode→decode roundtrip.
/// Encoding a unicode scalar and then decoding the result gives back the
/// original code point.
proof fn lemma_encode_decode_roundtrip(cp: u32)
    requires
        is_unicode_scalar(cp),
    ensures
        spec_decode_code_point(spec_encode_code_point(cp)) == cp,
{
    if cp <= 0x7F {
    } else if cp <= 0x7FF {
        lemma_2byte_roundtrip(cp);
    } else if cp <= 0xFFFF {
        lemma_3byte_roundtrip(cp);
    } else {
        lemma_4byte_roundtrip(cp);
    }
}

/// Theorem: the encoded output has the correct leading byte pattern for its
/// length class, proving it is well-formed UTF-8.
proof fn lemma_encode_wellformed(cp: u32)
    requires
        is_unicode_scalar(cp),
    ensures ({
        let encoded = spec_encode_code_point(cp);
        if cp <= 0x7F {
            encoded.len() == 1
            && encoded[0] <= 0x7F
        } else if cp <= 0x7FF {
            encoded.len() == 2
            && (encoded[0] & 0xE0) == 0xC0
            && (encoded[1] & 0xC0) == 0x80
        } else if cp <= 0xFFFF {
            encoded.len() == 3
            && (encoded[0] & 0xF0) == 0xE0
            && (encoded[1] & 0xC0) == 0x80
            && (encoded[2] & 0xC0) == 0x80
        } else {
            encoded.len() == 4
            && (encoded[0] & 0xF8) == 0xF0
            && (encoded[1] & 0xC0) == 0x80
            && (encoded[2] & 0xC0) == 0x80
            && (encoded[3] & 0xC0) == 0x80
        }
    }),
{
    if cp <= 0x7F {
    } else if cp <= 0x7FF {
        assert((((0xC0u32 | (cp >> 6)) as u8) & 0xE0u8) == 0xC0u8) by (bit_vector)
            requires cp <= 0x7FFu32;
        assert((((0x80u32 | (cp & 0x3F)) as u8) & 0xC0u8) == 0x80u8) by (bit_vector);
    } else if cp <= 0xFFFF {
        assert((((0xE0u32 | (cp >> 12)) as u8) & 0xF0u8) == 0xE0u8) by (bit_vector)
            requires cp <= 0xFFFFu32;
        assert((((0x80u32 | ((cp >> 6) & 0x3F)) as u8) & 0xC0u8) == 0x80u8) by (bit_vector);
        assert((((0x80u32 | (cp & 0x3F)) as u8) & 0xC0u8) == 0x80u8) by (bit_vector);
    } else {
        assert((((0xF0u32 | (cp >> 18)) as u8) & 0xF8u8) == 0xF0u8) by (bit_vector)
            requires cp <= 0x10FFFFu32;
        assert((((0x80u32 | ((cp >> 12) & 0x3F)) as u8) & 0xC0u8) == 0x80u8) by (bit_vector);
        assert((((0x80u32 | ((cp >> 6) & 0x3F)) as u8) & 0xC0u8) == 0x80u8) by (bit_vector);
        assert((((0x80u32 | (cp & 0x3F)) as u8) & 0xC0u8) == 0x80u8) by (bit_vector);
    }
}

/// Theorem: the decoded code point of a well-formed encoding is a valid
/// unicode scalar (provided the original was).
proof fn lemma_decode_preserves_scalar(cp: u32)
    requires
        is_unicode_scalar(cp),
    ensures
        is_unicode_scalar(spec_decode_code_point(spec_encode_code_point(cp))),
{
    lemma_encode_decode_roundtrip(cp);
}

/// Spec: decode 4 hex digit bytes starting at `pos` into a u16.
/// Interprets input[pos..pos+4] as a big-endian hex number.
pub open spec fn spec_decode_hex4(input: Seq<u8>, pos: nat) -> u16
    recommends pos + 4 <= input.len(),
{
    (
        (spec_hex_val(input[pos as int]) as u16) * 0x1000
        + (spec_hex_val(input[(pos + 1) as int]) as u16) * 0x100
        + (spec_hex_val(input[(pos + 2) as int]) as u16) * 0x10
        + (spec_hex_val(input[(pos + 3) as int]) as u16)
    ) as u16
}

// =============================================================================
// Exec wrappers
// =============================================================================

/// Exec: check if byte is an ASCII digit
pub fn is_ascii_digit(b: u8) -> (result: bool)
    ensures
        result == spec_is_ascii_digit(b),
{
    0x30 <= b && b <= 0x39
}

/// Exec: check if byte is a hex digit
pub fn is_hex_digit(b: u8) -> (result: bool)
    ensures
        result == spec_is_hex_digit(b),
{
    (0x30 <= b && b <= 0x39) || (0x61 <= b && b <= 0x66) || (0x41 <= b && b <= 0x46)
}

/// Exec: check if byte is JSON whitespace (space, tab, newline, carriage return)
pub fn is_whitespace(b: u8) -> (result: bool)
    ensures
        result == spec_is_whitespace(b),
{
    b == 0x20 || b == 0x09 || b == 0x0A || b == 0x0D
}

/// Exec: check if byte is a valid simple escape character
pub fn is_simple_escape(b: u8) -> (result: bool)
    ensures
        result == spec_is_simple_escape(b),
{
    b == 0x22 || b == 0x5C || b == 0x2F || b == 0x62
        || b == 0x66 || b == 0x6E || b == 0x72 || b == 0x74
}

/// Exec: hex digit to value (0-15). Returns None if not a hex digit.
pub fn hex_val(b: u8) -> (result: Option<u8>)
    ensures
        match result {
            Some(v) => v <= 15 && v == spec_hex_val(b) && spec_is_hex_digit(b),
            None => !spec_is_hex_digit(b),
        },
{
    if 0x30 <= b && b <= 0x39 { Some((b - 0x30) as u8) }
    else if 0x61 <= b && b <= 0x66 { Some((b - 0x61 + 10) as u8) }
    else if 0x41 <= b && b <= 0x46 { Some((b - 0x41 + 10) as u8) }
    else { None }
}

/// Exec: append the UTF-8 encoding of `cp` to `out`.
/// Proven to produce exactly the bytes specified by `spec_encode_code_point`.
pub fn encode_code_point(cp: u32, out: &mut Vec<u8>)
    requires
        is_unicode_scalar(cp),
    ensures
        final(out)@ == old(out)@ + spec_encode_code_point(cp),
{
    if cp <= 0x7F {
        out.push(cp as u8);
    } else if cp <= 0x7FF {
        out.push((0xC0 | (cp >> 6)) as u8);
        out.push((0x80 | (cp & 0x3F)) as u8);
    } else if cp <= 0xFFFF {
        out.push((0xE0 | (cp >> 12)) as u8);
        out.push((0x80 | ((cp >> 6) & 0x3F)) as u8);
        out.push((0x80 | (cp & 0x3F)) as u8);
    } else {
        out.push((0xF0 | (cp >> 18)) as u8);
        out.push((0x80 | ((cp >> 12) & 0x3F)) as u8);
        out.push((0x80 | ((cp >> 6) & 0x3F)) as u8);
        out.push((0x80 | (cp & 0x3F)) as u8);
    }
}

/// Exec: decode 4 hex digit bytes into a u16.
pub fn decode_hex4(input: &[u8], pos: usize) -> (result: Option<u16>)
    requires
        pos + 4 <= input@.len(),
    ensures
        match result {
            Some(v) => {
                &&& v <= 0xFFFF
                &&& v == spec_decode_hex4(input@, pos as nat)
                &&& spec_is_hex_quad(input@, pos as nat)
            },
            None => {
                !spec_is_hex_quad(input@, pos as nat)
            },
        },
{
    let d0 = match hex_val(input[pos]) { Some(v) => v as u16, None => return None };
    let d1 = match hex_val(input[pos + 1]) { Some(v) => v as u16, None => return None };
    let d2 = match hex_val(input[pos + 2]) { Some(v) => v as u16, None => return None };
    let d3 = match hex_val(input[pos + 3]) { Some(v) => v as u16, None => return None };
    Some(d0 * 0x1000 + d1 * 0x100 + d2 * 0x10 + d3)
}

} // verus!
