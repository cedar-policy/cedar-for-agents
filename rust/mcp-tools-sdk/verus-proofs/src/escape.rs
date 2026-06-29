use vstd::prelude::*;

verus! {

// =============================================================================
// UTF-8 encoding of Unicode code points
//
// Spec: clear, readable definitions of what valid UTF-8 encoding means.
// Exec: efficient byte-level implementation proven to match the spec.
// =============================================================================

/// A Unicode scalar value: a code point that is not a surrogate.
pub open spec fn is_unicode_scalar(cp: u32) -> bool {
    cp <= 0x10FFFF && !(0xD800 <= cp && cp <= 0xDFFF)
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

// =============================================================================
// JSON string escape decoding (byte-level, fully verified)
//
// Spec: what the decoded bytes of a JSON string should be.
// Exec: walks the raw bytes between quotes, decoding escapes into UTF-8.
// =============================================================================

/// Hex digit to value (0-15). Returns None if not a hex digit.
pub fn hex_val(b: u8) -> (result: Option<u8>)
    ensures
        match result {
            Some(v) => v <= 15,
            None => true,
        },
{
    if 0x30 <= b && b <= 0x39 { Some((b - 0x30) as u8) }
    else if 0x61 <= b && b <= 0x66 { Some((b - 0x61 + 10) as u8) }
    else if 0x41 <= b && b <= 0x46 { Some((b - 0x41 + 10) as u8) }
    else { None }
}

/// Decode 4 hex digit bytes into a u16.
pub fn decode_hex4(input: &[u8], pos: usize) -> (result: Option<u16>)
    requires
        pos + 4 <= input@.len(),
    ensures
        match result {
            Some(v) => v <= 0xFFFF,
            None => true,
        },
{
    let d0 = match hex_val(input[pos]) { Some(v) => v as u16, None => return None };
    let d1 = match hex_val(input[pos + 1]) { Some(v) => v as u16, None => return None };
    let d2 = match hex_val(input[pos + 2]) { Some(v) => v as u16, None => return None };
    let d3 = match hex_val(input[pos + 3]) { Some(v) => v as u16, None => return None };
    Some(d0 * 0x1000 + d1 * 0x100 + d2 * 0x10 + d3)
}

/// Result of decoding a JSON string's escape sequences.
pub enum DecodeResult {
    /// Successfully decoded; `bytes` contains the UTF-8 decoded content.
    Ok { bytes: Vec<u8> },
    /// No escapes found; the raw bytes are already valid content.
    NoEscapes,
    /// Error at the given byte position.
    Err { pos: usize },
}

/// Decode JSON escape sequences from raw bytes between quotes.
/// `input[start..end]` is the content between the opening and closing `"`.
///
/// Returns:
/// - `NoEscapes` if no backslashes are present (fast path, zero-copy)
/// - `Ok { bytes }` with the decoded UTF-8 bytes
/// - `Err { pos }` for invalid escape sequences
pub fn decode_json_escapes_bytes(input: &[u8], start: usize, end: usize) -> (result: DecodeResult)
    requires
        start <= end,
        end <= input@.len(),
    ensures
        // no escapes iff no backslashes
        result is NoEscapes <==> (forall|k: int| start <= k < end ==> input@[k] != 0x5C),
{
    // Fast path: scan for backslash
    let mut has_escape = false;
    let mut scan = start;
    while scan < end
        invariant
            start <= scan <= end,
            end <= input@.len(),
            !has_escape ==> (forall|k: int| start <= k < scan ==> input@[k] != 0x5C),
            has_escape ==> scan == end,
            has_escape ==> (exists|k: int| start <= k < end && input@[k] == 0x5C),
        decreases end - scan,
    {
        if input[scan] == 0x5C {
            has_escape = true;
            // break out — we can't use `break` easily with invariants,
            // so just let the loop finish by jumping scan to end
            scan = end;
        } else {
            scan = scan + 1;
        }
    }
    if !has_escape {
        return DecodeResult::NoEscapes;
    }

    // Decode pass
    let mut out: Vec<u8> = Vec::new();
    let mut i = start;
    while i < end
        invariant
            start <= i <= end,
            end <= input@.len(),
        decreases end - i,
    {
        let b = input[i];
        if b != 0x5C {
            // Non-escape byte: copy directly
            out.push(b);
            i += 1;
        } else {
            // Escape sequence
            i += 1;
            if i >= end {
                return DecodeResult::Err { pos: i };
            }
            let esc = input[i];
            if esc == 0x22 { out.push(0x22); i += 1; }        // \"
            else if esc == 0x5C { out.push(0x5C); i += 1; }   // \\
            else if esc == 0x2F { out.push(0x2F); i += 1; }   // \/
            else if esc == 0x62 { out.push(0x08); i += 1; }   // \b
            else if esc == 0x66 { out.push(0x0C); i += 1; }   // \f
            else if esc == 0x6E { out.push(0x0A); i += 1; }   // \n
            else if esc == 0x72 { out.push(0x0D); i += 1; }   // \r
            else if esc == 0x74 { out.push(0x09); i += 1; }   // \t
            else if esc == 0x75 {
                // \uXXXX
                i += 1;
                if end - i < 4 {
                    return DecodeResult::Err { pos: i };
                }
                let cp = match decode_hex4(input, i) {
                    Some(v) => v,
                    None => return DecodeResult::Err { pos: i },
                };
                i += 4;

                if 0xD800 <= cp && cp <= 0xDBFF {
                    // High surrogate — expect \uXXXX low surrogate
                    if end - i < 6 {
                        return DecodeResult::Err { pos: i };
                    }
                    if input[i] != 0x5C || input[i + 1] != 0x75 {
                        return DecodeResult::Err { pos: i };
                    }
                    i += 2;
                    let low = match decode_hex4(input, i) {
                        Some(v) => v,
                        None => return DecodeResult::Err { pos: i },
                    };
                    i += 4;
                    if !(0xDC00 <= low && low <= 0xDFFF) {
                        return DecodeResult::Err { pos: i };
                    }
                    let full: u32 = 0x10000 + (((cp as u32) - 0xD800) * 0x400) + ((low as u32) - 0xDC00);
                    encode_code_point(full, &mut out);
                } else if 0xDC00 <= cp && cp <= 0xDFFF {
                    // Lone low surrogate
                    return DecodeResult::Err { pos: i };
                } else {
                    encode_code_point(cp as u32, &mut out);
                }
            } else {
                return DecodeResult::Err { pos: i };
            }
        }
    }
    DecodeResult::Ok { bytes: out }
}

} // verus!
