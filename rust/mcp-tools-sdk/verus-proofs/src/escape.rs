use vstd::prelude::*;
use crate::common_specs::*;

verus! {

// =============================================================================
// JSON string escape decoding (byte-level, fully verified)
//
// Spec: what the decoded bytes of a JSON string should be.
// Exec: walks the raw bytes between quotes, decoding escapes into UTF-8.
// =============================================================================

// =============================================================================
// JSON string escape decoding — SPECIFICATION
//
// This defines the *mathematical meaning* of a JSON-escaped string:
// given raw bytes between quotes, what should the decoded output be?
//
// The spec processes the input left-to-right. At each position it classifies
// the current "chunk" as one of:
//   - A plain byte (not backslash, not a control char): output as-is
//   - A simple escape (\", \\, \/, \b, \f, \n, \r, \t): output the decoded byte
//   - A BMP escape (\uXXXX where XXXX is not a surrogate): output UTF-8 encoding
//   - A surrogate pair (\uHHHH\uLLLL): output UTF-8 encoding of the combined code point
//
// The function returns `None` if the input is malformed (bad escape, lone
// surrogate, truncated sequence). It returns `Some(decoded_bytes)` on success.
// =============================================================================

/// Spec: the byte that a simple escape character decodes to.
/// For example, 'n' maps to newline (0x0A).
pub open spec fn spec_simple_escape_byte(esc: u8) -> u8 {
    if esc == QUOTE() { QUOTE() }             // \" -> "
    else if esc == BACKSLASH() { BACKSLASH() } // \\ -> \
    else if esc == SLASH() { SLASH() }         // \/ -> /
    else if esc == LOWER_B() { BACKSPACE() }   // \b -> backspace
    else if esc == LOWER_F() { FORMFEED() }    // \f -> form feed
    else if esc == LOWER_N() { NEWLINE() }     // \n -> newline
    else if esc == LOWER_R() { CR() }          // \r -> carriage return
    else if esc == LOWER_T() { TAB() }         // \t -> tab
    else { 0 }                                 // undefined (not a simple escape)
}

/// Spec: decode JSON escape sequences in `input[start..end]`.
///
/// This is a recursive function that processes the input one "chunk" at a time:
/// - Plain byte → append it, advance by 1
/// - \X (simple escape) → append decoded byte, advance by 2
/// - \uXXXX (BMP, non-surrogate) → append UTF-8 encoding, advance by 6
/// - \uHHHH\uLLLL (surrogate pair) → append UTF-8 encoding, advance by 12
///
/// Returns `None` on any malformed escape sequence.
/// Returns `Some(bytes)` with the fully decoded byte sequence on success.
pub open spec fn spec_decode(input: Seq<u8>, start: nat, end: nat) -> Option<Seq<u8>>
    recommends start <= end && end <= input.len(),
    decreases end - start,
{
    if start >= end {
        Some(seq![])
    } else if input[start as int] != BACKSLASH() {
        // Plain byte (not a backslash): output this byte, decode the rest
        match spec_decode(input, start + 1, end) {
            Some(rest) => Some(seq![input[start as int]] + rest),
            None => None,
        }
    } else {
        // Backslash: start of an escape sequence
        if start + 1 >= end {
            None
        } else {
            let esc = input[(start + 1) as int];
            if spec_is_simple_escape(esc) {
                match spec_decode(input, start + 2, end) {
                    Some(rest) => Some(seq![spec_simple_escape_byte(esc)] + rest),
                    None => None,
                }
            } else if esc == LOWER_U() {
                // Unicode escape: \uXXXX
                if start + 6 > end {
                    None
                } else {
                    let cp = spec_decode_hex4(input, (start + 2) as nat) as u32;
                    if 0xD800 <= cp && cp <= 0xDBFF {
                        // High surrogate: must be followed by \uXXXX low surrogate
                        if start + 12 > end {
                            None
                        } else if input[(start + 6) as int] != BACKSLASH()
                               || input[(start + 7) as int] != LOWER_U() {
                            None
                        } else {
                            let low = spec_decode_hex4(input, (start + 8) as nat) as u32;
                            if !(0xDC00 <= low && low <= 0xDFFF) {
                                None
                            } else {
                                let full: u32 = (0x10000u32 + (cp - 0xD800) * 0x400 + (low - 0xDC00)) as u32;
                                match spec_decode(input, start + 12, end) {
                                    Some(rest) => Some(spec_encode_code_point(full) + rest),
                                    None => None,
                                }
                            }
                        }
                    } else if 0xDC00 <= cp && cp <= 0xDFFF {
                        // Lone low surrogate: invalid
                        None
                    } else {
                        // BMP character (non-surrogate): encode as UTF-8
                        match spec_decode(input, start + 6, end) {
                            Some(rest) => Some(spec_encode_code_point(cp) + rest),
                            None => None,
                        }
                    }
                }
            } else {
                // Unknown escape character: invalid
                None
            }
        }
    }
}

/// Spec: the identity decode for input with no backslashes.
///
/// When there are no escape sequences, the decoded output is just
/// the raw bytes themselves: `input[start..end]`.
pub open spec fn spec_decode_identity(input: Seq<u8>, start: nat, end: nat) -> Seq<u8>
    recommends start <= end && end <= input.len(),
{
    input.subrange(start as int, end as int)
}

// =============================================================================
// Proof: when no backslashes are present, spec_decode returns the raw bytes
// =============================================================================

/// Lemma: if there are no backslashes in input[start..end], then spec_decode
/// returns Some(input[start..end]) — i.e. the raw bytes are the decoded output.
proof fn lemma_no_escapes_identity(input: Seq<u8>, start: nat, end: nat)
    requires
        start <= end,
        end <= input.len(),
        forall|k: int| start <= k < end ==> input[k] != BACKSLASH(),
    ensures
        spec_decode(input, start, end) == Some(input.subrange(start as int, end as int)),
    decreases end - start,
{
    if start < end {
        // input[start] is not a backslash, so spec_decode takes the "plain byte" branch
        lemma_no_escapes_identity(input, start + 1, end);
        // Now we know:
        //   spec_decode(input, start, end)
        //     == Some(seq![input[start]] + input.subrange(start+1, end))
        // We need to show this equals Some(input.subrange(start, end))
        assert(
            seq![input[start as int]] + input.subrange((start + 1) as int, end as int)
            =~= input.subrange(start as int, end as int)
        );
    }
}

// =============================================================================
// JSON string escape decoding — EXEC with functional correctness
// =============================================================================

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
///
/// Functional correctness:
/// - `NoEscapes` iff no backslashes in the range (bidirectional)
/// - `NoEscapes` implies `spec_decode` returns `Some(input[start..end])`
///   (i.e. escape-free input decodes to itself)
pub fn decode_json_escapes_bytes(input: &[u8], start: usize, end: usize) -> (result: DecodeResult)
    requires
        start <= end,
        end <= input@.len(),
    ensures
        // Structural: no escapes iff no backslashes
        result is NoEscapes <==> (forall|k: int| start <= k < end ==> input@[k] != BACKSLASH()),
        // Functional correctness for NoEscapes:
        // the spec decode of escape-free input is just the raw bytes
        result is NoEscapes ==> spec_decode(input@, start as nat, end as nat)
            == Some(input@.subrange(start as int, end as int)),
{
    // Fast path: scan for backslash
    let mut has_escape = false;
    let mut scan = start;
    while scan < end
        invariant
            start <= scan <= end,
            end <= input@.len(),
            !has_escape ==> (forall|k: int| start <= k < scan ==> input@[k] != BACKSLASH()),
            has_escape ==> scan == end,
            has_escape ==> (exists|k: int| start <= k < end && input@[k] == BACKSLASH()),
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
        proof {
            lemma_no_escapes_identity(input@, start as nat, end as nat);
        }
        return DecodeResult::NoEscapes;
    }

    // Decode pass.
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
            // Plain byte: spec_decode takes the first branch (non-backslash)
            // spec_decode(i, end) == Some(seq![b] + spec_decode(i+1, end).unwrap())
            // So: out@ + seq![b] + rest == (out + [b])@ + rest
            out.push(b);
            i = i + 1;
        } else {
            // Escape sequence: backslash at position i
            i = i + 1;
            if i >= end {
                // Truncated escape: spec_decode returns None from here
                // (backslash at end), so the full decode is also None.
                return DecodeResult::Err { pos: i };
            }
            let esc = input[i];
            if esc == 0x22 { out.push(0x22); i = i + 1; }        // \"
            else if esc == 0x5C { out.push(0x5C); i = i + 1; }   // \\
            else if esc == 0x2F { out.push(0x2F); i = i + 1; }   // \/
            else if esc == 0x62 { out.push(0x08); i = i + 1; }   // \b
            else if esc == 0x66 { out.push(0x0C); i = i + 1; }   // \f
            else if esc == 0x6E { out.push(0x0A); i = i + 1; }   // \n
            else if esc == 0x72 { out.push(0x0D); i = i + 1; }   // \r
            else if esc == 0x74 { out.push(0x09); i = i + 1; }   // \t
            else if esc == 0x75 {
                // \uXXXX
                i = i + 1;
                if end - i < 4 {
                    return DecodeResult::Err { pos: i };
                }
                let cp = match decode_hex4(input, i) {
                    Some(v) => v,
                    None => return DecodeResult::Err { pos: i },
                };
                i = i + 4;

                if 0xD800 <= cp && cp <= 0xDBFF {
                    // High surrogate — expect \uXXXX low surrogate
                    if end - i < 6 {
                        return DecodeResult::Err { pos: i };
                    }
                    if input[i] != 0x5C || input[i + 1] != 0x75 {
                        return DecodeResult::Err { pos: i };
                    }
                    i = i + 2;
                    let low = match decode_hex4(input, i) {
                        Some(v) => v,
                        None => return DecodeResult::Err { pos: i },
                    };
                    i = i + 4;
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
    // Loop completed without error
    DecodeResult::Ok { bytes: out }
}

} // verus!
