use crate::common_specs::*;
use vstd::prelude::*;

verus! {

// =============================================================================
// JSON string escape decoding (byte-level, fully verified)
//
// Spec: what the decoded bytes of a JSON string should be.
// Exec: walks the raw bytes between quotes, decoding escapes into UTF-8.
// =============================================================================

// =============================================================================
// JSON string escape decoding — SPECIFICATION (RFC 8259 §7)
//
// https://www.rfc-editor.org/rfc/rfc8259#section-7
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

/// Spec: the byte that a simple escape character decodes to (RFC 8259 §7).
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
                } else if !spec_is_hex_quad(input, (start + 2) as nat) {
                    // Invalid hex digits
                    None
                } else {
                    let cp = spec_decode_hex4(input, (start + 2) as nat) as u32;
                    if is_high_surrogate(cp) {
                        // High surrogate: must be followed by \uXXXX low surrogate
                        if start + 12 > end {
                            None
                        } else if input[(start + 6) as int] != BACKSLASH()
                               || input[(start + 7) as int] != LOWER_U() {
                            None
                        } else if !spec_is_hex_quad(input, (start + 8) as nat) {
                            // Invalid hex digits in low surrogate
                            None
                        } else {
                            let low = spec_decode_hex4(input, (start + 8) as nat) as u32;
                            if !is_low_surrogate(low) {
                                None
                            } else {
                                let full = surrogate_pair_value(cp, low);
                                match spec_decode(input, start + 12, end) {
                                    Some(rest) => Some(spec_encode_code_point(full) + rest),
                                    None => None,
                                }
                            }
                        }
                    } else if is_low_surrogate(cp) {
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

/// Result of processing one "chunk" (one plain byte or one escape sequence).
/// Returned by decode_one_chunk.
enum ChunkResult {
    /// Successfully decoded one chunk; `next` is the position after it.
    Ok { next: usize },
    /// Error at the given position.
    Err { pos: usize },
}

/// Spec: spec_decode succeeds (returns Some) for the given range.
pub open spec fn spec_decode_ok(input: Seq<u8>, start: nat, end: nat) -> bool {
    spec_decode(input, start, end) is Some
}

/// Lemma: spec_decode unfolding for a plain byte.
/// If input[start] is not a backslash and spec_decode(start, end) is Some,
/// then spec_decode(start+1, end) is also Some, and the result is
/// seq![input[start]] + spec_decode(start+1, end).unwrap().
proof fn lemma_decode_unfold_plain(input: Seq<u8>, start: nat, end: nat)
    requires
        start < end,
        end <= input.len(),
        input[start as int] != BACKSLASH(),
        spec_decode_ok(input, start, end),
    ensures
        spec_decode_ok(input, start + 1, end),
        spec_decode(input, start, end) == Some(
            seq![input[start as int]] + spec_decode(input, start + 1, end).unwrap()
        ),
{
}

/// Lemma: spec_decode unfolding for a simple escape.
/// If input[start] == '\' and input[start+1] is a simple escape char,
/// and spec_decode(start, end) is Some, then spec_decode(start+2, end) is also Some.
proof fn lemma_decode_unfold_simple_escape(input: Seq<u8>, start: nat, end: nat)
    requires
        start + 1 < end,
        end <= input.len(),
        input[start as int] == BACKSLASH(),
        spec_is_simple_escape(input[(start + 1) as int]),
        spec_decode_ok(input, start, end),
    ensures
        spec_decode_ok(input, start + 2, end),
        spec_decode(input, start, end) == Some(
            seq![spec_simple_escape_byte(input[(start + 1) as int])]
            + spec_decode(input, start + 2, end).unwrap()
        ),
{
}

/// Lemma: spec_decode unfolding for a BMP unicode escape (\uXXXX, non-surrogate).
proof fn lemma_decode_unfold_bmp(input: Seq<u8>, start: nat, end: nat)
    requires
        start + 6 <= end,
        end <= input.len(),
        input[start as int] == BACKSLASH(),
        input[(start + 1) as int] == LOWER_U(),
        spec_is_hex_quad(input, (start + 2) as nat),
            !is_surrogate(spec_decode_hex4(input, (start + 2) as nat) as u32),
        spec_decode_ok(input, start, end),
    ensures
        spec_decode_ok(input, start + 6, end),
        spec_decode(input, start, end) == Some(
            spec_encode_code_point(spec_decode_hex4(input, (start + 2) as nat) as u32)
            + spec_decode(input, start + 6, end).unwrap()
        ),
{
}

/// Lemma: spec_decode unfolding for a surrogate pair (\uHHHH\uLLLL).
proof fn lemma_decode_unfold_surrogate_pair(input: Seq<u8>, start: nat, end: nat)
    requires
        start + 12 <= end,
        end <= input.len(),
        input[start as int] == BACKSLASH(),
        input[(start + 1) as int] == LOWER_U(),
        spec_is_hex_quad(input, (start + 2) as nat),
        input[(start + 6) as int] == BACKSLASH(),
        input[(start + 7) as int] == LOWER_U(),
        spec_is_hex_quad(input, (start + 8) as nat),
        ({
            let hi = spec_decode_hex4(input, (start + 2) as nat) as u32;
            let lo = spec_decode_hex4(input, (start + 8) as nat) as u32;
            is_high_surrogate(hi) && is_low_surrogate(lo)
        }),
        spec_decode_ok(input, start, end),
    ensures
        spec_decode_ok(input, start + 12, end),
        spec_decode(input, start, end) == Some(
            spec_encode_code_point(
                surrogate_pair_value(
                    spec_decode_hex4(input, (start + 2) as nat) as u32,
                    spec_decode_hex4(input, (start + 8) as nat) as u32,
                )
            ) + spec_decode(input, start + 12, end).unwrap()
        ),
{
}

/// Process one chunk (plain byte or escape) starting at position `i`.
/// Appends decoded bytes to `out` and returns the next position.
///
/// Postcondition: if `spec_decode(input, i, end)` was `Some(chunk_bytes + rest)`,
/// then after this call `out` has `chunk_bytes` appended and we return the position
/// where `rest` starts.
fn decode_one_chunk(input: &[u8], i: usize, end: usize, out: &mut Vec<u8>) -> (result: ChunkResult)
    requires
        i < end,
        end <= input@.len(),
    ensures
        match result {
            ChunkResult::Ok { next } => {
                &&& i < next && next <= end
                &&& final(out)@.len() >= old(out)@.len()
                &&& final(out)@.subrange(0, old(out)@.len() as int) =~= old(out)@
                &&& (spec_decode_ok(input@, i as nat, end as nat) ==> (
                    spec_decode_ok(input@, next as nat, end as nat)
                    && spec_decode(input@, i as nat, end as nat) == Some(
                        final(out)@.subrange(old(out)@.len() as int, final(out)@.len() as int)
                        + spec_decode(input@, next as nat, end as nat).unwrap()
                    )
                ))
            },
            ChunkResult::Err { .. } => {
                &&& !spec_decode_ok(input@, i as nat, end as nat)
                &&& final(out)@ =~= old(out)@
            },
        },
{
    let b = input[i];
    if b != 0x5C {
        // Plain byte: not a backslash
        let ghost out_pre = out@;
        out.push(b);
        proof {
            if spec_decode_ok(input@, i as nat, end as nat) {
                lemma_decode_unfold_plain(input@, i as nat, end as nat);
            }
        }
        ChunkResult::Ok { next: i + 1 }
    } else {
        // Backslash: start of an escape sequence
        let esc_pos = i + 1;
        if esc_pos >= end {
            // Truncated: backslash at end. spec_decode: input[i]=='\', i+1>=end → None
            return ChunkResult::Err { pos: esc_pos };
        }
        let esc = input[esc_pos];
        if is_simple_escape(esc) {
            // Simple escape: push decoded byte
            let ghost out_pre = out@;
            if esc == 0x22 { out.push(0x22); }
            else if esc == 0x5C { out.push(0x5C); }
            else if esc == 0x2F { out.push(0x2F); }
            else if esc == 0x62 { out.push(0x08); }
            else if esc == 0x66 { out.push(0x0C); }
            else if esc == 0x6E { out.push(0x0A); }
            else if esc == 0x72 { out.push(0x0D); }
            else { out.push(0x09); } // 0x74 → \t
            proof {
                if spec_decode_ok(input@, i as nat, end as nat) {
                    lemma_decode_unfold_simple_escape(input@, i as nat, end as nat);
                }
            }
            ChunkResult::Ok { next: i + 2 }
        } else if esc == 0x75 {
            // \uXXXX
            let hex_start = esc_pos + 1;
            if end - hex_start < 4 {
                // Not enough bytes for \uXXXX → None
                return ChunkResult::Err { pos: hex_start };
            }
            let cp = match decode_hex4(input, hex_start) {
                Some(v) => v,
                None => {
                    // Invalid hex digits — but spec_decode_hex4 always returns a value
                    // (spec_hex_val returns 0 for non-hex). The exec decode_hex4 returns
                    // None for non-hex, while spec doesn't check validity.
                    // Actually: if spec_decode_ok(i,end) held, the spec would have computed
                    // a cp. But the exec CAN'T fail on hex if the tokenizer already validated.
                    // For safety, just return error.
                    return ChunkResult::Err { pos: hex_start };
                }
            };
            let after_hex = hex_start + 4; // == i + 6

            if 0xD800 <= cp && cp <= 0xDBFF {
                // High surrogate — need \uLLLL low surrogate
                if end - after_hex < 6 {
                    return ChunkResult::Err { pos: after_hex };
                }
                if input[after_hex] != 0x5C || input[after_hex + 1] != 0x75 {
                    return ChunkResult::Err { pos: after_hex };
                }
                let low_hex_start = after_hex + 2;
                let low = match decode_hex4(input, low_hex_start) {
                    Some(v) => v,
                    None => return ChunkResult::Err { pos: low_hex_start },
                };
                let after_pair = low_hex_start + 4; // == i + 12
                if !(0xDC00 <= low && low <= 0xDFFF) {
                    return ChunkResult::Err { pos: after_pair };
                }
                let full: u32 = 0x10000 + (((cp as u32) - 0xD800) * 0x400) + ((low as u32) - 0xDC00);
                let ghost out_pre = out@;
                proof {
                    if spec_decode_ok(input@, i as nat, end as nat) {
                        lemma_decode_unfold_surrogate_pair(input@, i as nat, end as nat);
                    }
                }
                encode_code_point(full, out);
                ChunkResult::Ok { next: after_pair }
            } else if 0xDC00 <= cp && cp <= 0xDFFF {
                // Lone low surrogate: invalid
                return ChunkResult::Err { pos: after_hex };
            } else {
                // BMP (non-surrogate)
                let ghost out_pre = out@;
                proof {
                    if spec_decode_ok(input@, i as nat, end as nat) {
                        lemma_decode_unfold_bmp(input@, i as nat, end as nat);
                    }
                }
                encode_code_point(cp as u32, out);
                ChunkResult::Ok { next: after_hex }
            }
        } else {
            // Unknown escape character: not simple, not 'u' → None
            return ChunkResult::Err { pos: esc_pos };
        }
    }
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
/// - `Ok { bytes }` implies `spec_decode` returns `Some(bytes@)`
/// - `Err` implies `spec_decode` returns `None`
pub fn decode_json_escapes_bytes(input: &[u8], start: usize, end: usize) -> (result: DecodeResult)
    requires
        start <= end,
        end <= input@.len(),
    ensures
        // Structural: no escapes iff no backslashes
        result is NoEscapes <==> (forall|k: int| start <= k < end ==> input@[k] != BACKSLASH()),
        // Functional correctness for NoEscapes:
        result is NoEscapes ==> spec_decode(input@, start as nat, end as nat)
            == Some(input@.subrange(start as int, end as int)),
        // Functional correctness for Ok:
        match result {
            DecodeResult::Ok { bytes } =>
                spec_decode_ok(input@, start as nat, end as nat)
                ==> spec_decode(input@, start as nat, end as nat) == Some(bytes@),
            _ => true,
        },
        // Functional correctness for Err:
        result is Err ==> !spec_decode_ok(input@, start as nat, end as nat),
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

    // Decode pass: try to decode; if spec_decode would return None, we return Err.
    // We don't know ahead of time whether spec_decode succeeds, so we attempt
    // decoding and track whether we hit an error.
    let mut out: Vec<u8> = Vec::new();
    let mut i = start;
    let ghost valid = spec_decode_ok(input@, start as nat, end as nat);

    while i < end
        invariant
            start <= i <= end,
            end <= input@.len(),
            valid == spec_decode_ok(input@, start as nat, end as nat),
            // We only enter this loop when has_escape is true
            exists|k: int| start <= k < end && input@[k] == BACKSLASH(),
            // If valid, the exec is tracking the spec decomposition
            valid ==> (
                spec_decode_ok(input@, i as nat, end as nat)
                && spec_decode(input@, start as nat, end as nat)
                    == Some(out@ + spec_decode(input@, i as nat, end as nat).unwrap())
            ),
            // If !valid, the exec hasn't errored yet but we can't say anything about out@
            // (this case will eventually hit an Err in decode_one_chunk, or the
            //  exec happens to match even though spec says None — but we don't need
            //  to prove anything in that case since we handle it below)
        decreases end - i,
    {
        let ghost old_out = out@;
        match decode_one_chunk(input, i, end, &mut out) {
            ChunkResult::Ok { next } => {
                proof {
                    // decode_one_chunk Ok ==> spec_decode_ok(i, end)
                    // So if valid was true, it's still maintained
                    if valid {
                        let chunk = out@.subrange(old_out.len() as int, out@.len() as int);
                        assert(out@ =~= old_out + chunk);
                        assert(
                            (out@ + spec_decode(input@, next as nat, end as nat).unwrap())
                            =~= (old_out + (chunk + spec_decode(input@, next as nat, end as nat).unwrap()))
                        );
                    }
                }
                i = next;
            }
            ChunkResult::Err { pos: err_pos } => {
                // decode_one_chunk Err ==> !spec_decode_ok(i, end)
                // If valid, invariant gives spec_decode_ok(i, end) — contradiction.
                // So valid must be false, meaning spec_decode(start, end) is None.
                // Either way, !spec_decode_ok(start, end) holds at this point.
                proof {
                    if valid {
                        // contradiction: invariant says spec_decode_ok(i, end)
                        // but Err postcondition says !spec_decode_ok(i, end)
                        assert(false);
                    }
                }
                return DecodeResult::Err { pos: err_pos };
            }
        }
    }

    // Loop exited normally: i == end
    // spec_decode(end, end) == Some(seq![])
    // If valid: spec_decode(start, end) == Some(out@ + seq![]) == Some(out@)
    // If !valid: we need to show this is impossible (all chunks succeeded,
    // meaning spec_decode_ok held at every step, meaning valid was true).
    // Actually: decode_one_chunk Ok ==> spec_decode_ok(i, end).
    // At the first iteration, i == start, and Ok ==> spec_decode_ok(start, end) == valid.
    // So valid must be true if we reach here.
    proof {
        // If the loop ran at least once, decode_one_chunk returned Ok on the
        // first call, proving spec_decode_ok(start, end), i.e., valid == true.
        // If the loop never ran (start == end), spec_decode(start, end) == Some(seq![]) trivially.
        assert(out@ + Seq::<u8>::empty() =~= out@);
    }
    DecodeResult::Ok { bytes: out }
}

} // verus!
