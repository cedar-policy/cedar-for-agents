/// UTF-8 validation for JSON string content.
///
/// Provides exec validation of one UTF-8 character, proven to match
/// vstd's `valid_first_scalar` spec (RFC 3629 / Unicode Standard Table 3-7).
use vstd::prelude::*;
use vstd::utf8::*;

verus! {

// =============================================================================
// Bit-vector helper lemmas (cannot use Seq indexing inside by(bit_vector))
// =============================================================================

proof fn lemma_2byte_not_overlong(b0: u8, b1: u8)
    requires 0xC2 <= b0 && b0 <= 0xDF, 0x80 <= b1 && b1 <= 0xBF,
    ensures not_overlong_encoding(codepoint_width_2(b0, b1), 2),
{
    assert(not_overlong_encoding(codepoint_width_2(b0, b1), 2)) by (bit_vector)
        requires 0xC2u8 <= b0 && b0 <= 0xDFu8, 0x80u8 <= b1 && b1 <= 0xBFu8;
}

proof fn lemma_2byte_not_surrogate(b0: u8, b1: u8)
    requires 0xC2 <= b0 && b0 <= 0xDF, 0x80 <= b1 && b1 <= 0xBF,
    ensures not_surrogate(codepoint_width_2(b0, b1)),
{
    assert(not_surrogate(codepoint_width_2(b0, b1))) by (bit_vector)
        requires 0xC2u8 <= b0 && b0 <= 0xDFu8, 0x80u8 <= b1 && b1 <= 0xBFu8;
}

proof fn lemma_3byte_not_overlong(b0: u8, b1: u8, b2: u8)
    requires
        0xE0 <= b0 && b0 <= 0xEF,
        0x80 <= b1 && b1 <= 0xBF,
        0x80 <= b2 && b2 <= 0xBF,
        (b0 != 0xE0 || b1 >= 0xA0),
    ensures not_overlong_encoding(codepoint_width_3(b0, b1, b2), 3),
{
    assert(not_overlong_encoding(codepoint_width_3(b0, b1, b2), 3)) by (bit_vector)
        requires 0xE0u8 <= b0 && b0 <= 0xEFu8,
                 0x80u8 <= b1 && b1 <= 0xBFu8,
                 0x80u8 <= b2 && b2 <= 0xBFu8,
                 (b0 != 0xE0u8 || b1 >= 0xA0u8);
}

proof fn lemma_3byte_not_surrogate(b0: u8, b1: u8, b2: u8)
    requires
        0xE0 <= b0 && b0 <= 0xEF,
        0x80 <= b1 && b1 <= 0xBF,
        0x80 <= b2 && b2 <= 0xBF,
        (b0 != 0xED || b1 <= 0x9F),
    ensures not_surrogate(codepoint_width_3(b0, b1, b2)),
{
    assert(not_surrogate(codepoint_width_3(b0, b1, b2))) by (bit_vector)
        requires 0xE0u8 <= b0 && b0 <= 0xEFu8,
                 0x80u8 <= b1 && b1 <= 0xBFu8,
                 0x80u8 <= b2 && b2 <= 0xBFu8,
                 (b0 != 0xEDu8 || b1 <= 0x9Fu8);
}

proof fn lemma_3byte_overlong(b0: u8, b1: u8, b2: u8)
    requires b0 == 0xE0, 0x80 <= b1 && b1 < 0xA0, 0x80 <= b2 && b2 <= 0xBF,
    ensures !not_overlong_encoding(codepoint_width_3(b0, b1, b2), 3),
{
    assert(!not_overlong_encoding(codepoint_width_3(b0, b1, b2), 3)) by (bit_vector)
        requires b0 == 0xE0u8, 0x80u8 <= b1 && b1 < 0xA0u8, 0x80u8 <= b2 && b2 <= 0xBFu8;
}

proof fn lemma_3byte_surrogate(b0: u8, b1: u8, b2: u8)
    requires b0 == 0xED, 0xA0 <= b1 && b1 <= 0xBF, 0x80 <= b2 && b2 <= 0xBF,
    ensures !not_surrogate(codepoint_width_3(b0, b1, b2)),
{
    assert(!not_surrogate(codepoint_width_3(b0, b1, b2))) by (bit_vector)
        requires b0 == 0xEDu8, 0xA0u8 <= b1 && b1 <= 0xBFu8, 0x80u8 <= b2 && b2 <= 0xBFu8;
}

proof fn lemma_4byte_not_overlong(b0: u8, b1: u8, b2: u8, b3: u8)
    requires
        0xF0 <= b0 && b0 <= 0xF4,
        0x80 <= b1 && b1 <= 0xBF,
        0x80 <= b2 && b2 <= 0xBF,
        0x80 <= b3 && b3 <= 0xBF,
        (b0 != 0xF0 || b1 >= 0x90),
        (b0 != 0xF4 || b1 <= 0x8F),
    ensures not_overlong_encoding(codepoint_width_4(b0, b1, b2, b3), 4),
{
    assert(not_overlong_encoding(codepoint_width_4(b0, b1, b2, b3), 4)) by (bit_vector)
        requires 0xF0u8 <= b0 && b0 <= 0xF4u8,
                 0x80u8 <= b1 && b1 <= 0xBFu8,
                 0x80u8 <= b2 && b2 <= 0xBFu8,
                 0x80u8 <= b3 && b3 <= 0xBFu8,
                 (b0 != 0xF0u8 || b1 >= 0x90u8),
                 (b0 != 0xF4u8 || b1 <= 0x8Fu8);
}

proof fn lemma_4byte_not_surrogate(b0: u8, b1: u8, b2: u8, b3: u8)
    requires
        0xF0 <= b0 && b0 <= 0xF4,
        0x80 <= b1 && b1 <= 0xBF,
        0x80 <= b2 && b2 <= 0xBF,
        0x80 <= b3 && b3 <= 0xBF,
        (b0 != 0xF0 || b1 >= 0x90),
        (b0 != 0xF4 || b1 <= 0x8F),
    ensures not_surrogate(codepoint_width_4(b0, b1, b2, b3)),
{
    assert(not_surrogate(codepoint_width_4(b0, b1, b2, b3))) by (bit_vector)
        requires 0xF0u8 <= b0 && b0 <= 0xF4u8,
                 0x80u8 <= b1 && b1 <= 0xBFu8,
                 0x80u8 <= b2 && b2 <= 0xBFu8,
                 0x80u8 <= b3 && b3 <= 0xBFu8,
                 (b0 != 0xF0u8 || b1 >= 0x90u8),
                 (b0 != 0xF4u8 || b1 <= 0x8Fu8);
}

proof fn lemma_4byte_overlong_f0(b0: u8, b1: u8, b2: u8, b3: u8)
    requires b0 == 0xF0, 0x80 <= b1 && b1 < 0x90, 0x80 <= b2 && b2 <= 0xBF, 0x80 <= b3 && b3 <= 0xBF,
    ensures !not_overlong_encoding(codepoint_width_4(b0, b1, b2, b3), 4),
{
    assert(!not_overlong_encoding(codepoint_width_4(b0, b1, b2, b3), 4)) by (bit_vector)
        requires b0 == 0xF0u8, 0x80u8 <= b1 && b1 < 0x90u8, 0x80u8 <= b2 && b2 <= 0xBFu8, 0x80u8 <= b3 && b3 <= 0xBFu8;
}

proof fn lemma_4byte_over_max_f4(b0: u8, b1: u8, b2: u8, b3: u8)
    requires b0 == 0xF4, 0x90 <= b1 && b1 <= 0xBF, 0x80 <= b2 && b2 <= 0xBF, 0x80 <= b3 && b3 <= 0xBF,
    ensures !not_overlong_encoding(codepoint_width_4(b0, b1, b2, b3), 4),
{
    assert(!not_overlong_encoding(codepoint_width_4(b0, b1, b2, b3), 4)) by (bit_vector)
        requires b0 == 0xF4u8, 0x90u8 <= b1 && b1 <= 0xBFu8, 0x80u8 <= b2 && b2 <= 0xBFu8, 0x80u8 <= b3 && b3 <= 0xBFu8;
}

proof fn lemma_c0c1_overlong(b0: u8, b1: u8)
    requires 0xC0 <= b0 && b0 <= 0xC1, 0x80 <= b1 && b1 <= 0xBF,
    ensures !not_overlong_encoding(codepoint_width_2(b0, b1), 2),
{
    assert(!not_overlong_encoding(codepoint_width_2(b0, b1), 2)) by (bit_vector)
        requires 0xC0u8 <= b0 && b0 <= 0xC1u8, 0x80u8 <= b1 && b1 <= 0xBFu8;
}

proof fn lemma_f5f7_over_max(b0: u8, b1: u8, b2: u8, b3: u8)
    requires 0xF5 <= b0 && b0 <= 0xF7, 0x80 <= b1 && b1 <= 0xBF, 0x80 <= b2 && b2 <= 0xBF, 0x80 <= b3 && b3 <= 0xBF,
    ensures !not_overlong_encoding(codepoint_width_4(b0, b1, b2, b3), 4),
{
    assert(!not_overlong_encoding(codepoint_width_4(b0, b1, b2, b3), 4)) by (bit_vector)
        requires 0xF5u8 <= b0 && b0 <= 0xF7u8, 0x80u8 <= b1 && b1 <= 0xBFu8, 0x80u8 <= b2 && b2 <= 0xBFu8, 0x80u8 <= b3 && b3 <= 0xBFu8;
}

// =============================================================================
// Main validation function
// =============================================================================

/// Result of validating one UTF-8 character.
pub enum Utf8CharResult {
    /// Valid character consuming `len` bytes (1-4).
    Ok { len: usize },
    /// Invalid byte sequence.
    Err,
}

/// Validate one UTF-8 character starting at `pos` in `input[pos..end)`.
/// On success, returns the number of bytes consumed (1-4).
/// Proven: Ok <==> vstd's `valid_first_scalar` holds on the subrange.
pub fn validate_utf8_char(input: &[u8], pos: usize, end: usize) -> (result: Utf8CharResult)
    requires
        pos < end,
        end <= input@.len(),
    ensures
        match result {
            Utf8CharResult::Ok { len } => {
                &&& 1 <= len <= 4
                &&& pos + len <= end
                &&& valid_first_scalar(input@.subrange(pos as int, end as int))
                &&& length_of_first_scalar(input@.subrange(pos as int, end as int)) == len as int
            },
            Utf8CharResult::Err => {
                !valid_first_scalar(input@.subrange(pos as int, end as int))
            },
        },
{
    let b0 = input[pos];
    let ghost sub = input@.subrange(pos as int, end as int);

    // 1-byte: [00-7F]
    if b0 <= 0x7F {
        proof {
            assert(is_leading_byte_width_1(sub[0]));
            assert(valid_leading_and_continuation_bytes_first_codepoint(sub));
            assert(decode_first_codepoint(sub) == codepoint_width_1(sub[0]));
            assert(not_overlong_encoding(decode_first_codepoint(sub), 1));
            assert(not_surrogate(decode_first_codepoint(sub)));
        }
        return Utf8CharResult::Ok { len: 1 };
    }

    // 2-byte: [C2-DF][80-BF]
    if 0xC2 <= b0 && b0 <= 0xDF {
        if end - pos < 2 {
            proof {
                assert(!is_leading_byte_width_1(sub[0]));
                assert(sub.len() < 2);
                assert(!valid_leading_and_continuation_bytes_first_codepoint(sub));
                assert(!valid_first_scalar(sub));
            }
            return Utf8CharResult::Err;
        }
        let b1 = input[pos + 1];
        if !(0x80 <= b1 && b1 <= 0xBF) {
            proof {
                assert(!is_leading_byte_width_1(sub[0]));
                assert(is_leading_byte_width_2(sub[0]));
                assert(!is_continuation_byte(sub[1]));
                assert(!valid_leading_and_continuation_bytes_first_codepoint(sub));
                assert(!valid_first_scalar(sub));
            }
            return Utf8CharResult::Err;
        }
        proof {
            assert(is_leading_byte_width_2(sub[0]));
            assert(is_continuation_byte(sub[1]));
            assert(valid_leading_and_continuation_bytes_first_codepoint(sub));
            assert(decode_first_codepoint(sub) == codepoint_width_2(sub[0], sub[1]));
            lemma_2byte_not_overlong(sub[0], sub[1]);
            lemma_2byte_not_surrogate(sub[0], sub[1]);
        }
        return Utf8CharResult::Ok { len: 2 };
    }

    // 3-byte: [E0-EF][80-BF][80-BF]
    if 0xE0 <= b0 && b0 <= 0xEF {
        if end - pos < 3 {
            proof {
                assert(!is_leading_byte_width_1(sub[0]));
                assert(!is_leading_byte_width_2(sub[0]));
                assert(is_leading_byte_width_3(sub[0]));
                assert(sub.len() < 3);
                assert(!valid_leading_and_continuation_bytes_first_codepoint(sub));
                assert(!valid_first_scalar(sub));
            }
            return Utf8CharResult::Err;
        }
        let b1 = input[pos + 1];
        let b2 = input[pos + 2];
        if !(0x80 <= b1 && b1 <= 0xBF) || !(0x80 <= b2 && b2 <= 0xBF) {
            proof {
                assert(!is_leading_byte_width_1(sub[0]));
                assert(!is_leading_byte_width_2(sub[0]));
                assert(is_leading_byte_width_3(sub[0]));
                assert(!is_continuation_byte(sub[1]) || !is_continuation_byte(sub[2]));
                assert(!valid_leading_and_continuation_bytes_first_codepoint(sub));
                assert(!valid_first_scalar(sub));
            }
            return Utf8CharResult::Err;
        }
        if b0 == 0xE0 && b1 < 0xA0 {
            proof {
                assert(valid_leading_and_continuation_bytes_first_codepoint(sub));
                assert(decode_first_codepoint(sub) == codepoint_width_3(sub[0], sub[1], sub[2]));
                lemma_3byte_overlong(sub[0], sub[1], sub[2]);
                assert(!valid_first_scalar(sub));
            }
            return Utf8CharResult::Err;
        }
        if b0 == 0xED && b1 > 0x9F {
            proof {
                assert(valid_leading_and_continuation_bytes_first_codepoint(sub));
                assert(decode_first_codepoint(sub) == codepoint_width_3(sub[0], sub[1], sub[2]));
                lemma_3byte_surrogate(sub[0], sub[1], sub[2]);
                assert(!valid_first_scalar(sub));
            }
            return Utf8CharResult::Err;
        }
        proof {
            assert(is_leading_byte_width_3(sub[0]));
            assert(is_continuation_byte(sub[1]));
            assert(is_continuation_byte(sub[2]));
            assert(valid_leading_and_continuation_bytes_first_codepoint(sub));
            assert(decode_first_codepoint(sub) == codepoint_width_3(sub[0], sub[1], sub[2]));
            lemma_3byte_not_overlong(sub[0], sub[1], sub[2]);
            lemma_3byte_not_surrogate(sub[0], sub[1], sub[2]);
        }
        return Utf8CharResult::Ok { len: 3 };
    }

    // 4-byte: [F0-F4][80-BF][80-BF][80-BF]
    if 0xF0 <= b0 && b0 <= 0xF4 {
        if end - pos < 4 {
            proof {
                assert(!is_leading_byte_width_1(sub[0]));
                assert(!is_leading_byte_width_2(sub[0]));
                assert(!is_leading_byte_width_3(sub[0]));
                assert(is_leading_byte_width_4(sub[0]));
                assert(sub.len() < 4);
                assert(!valid_leading_and_continuation_bytes_first_codepoint(sub));
                assert(!valid_first_scalar(sub));
            }
            return Utf8CharResult::Err;
        }
        let b1 = input[pos + 1];
        let b2 = input[pos + 2];
        let b3 = input[pos + 3];
        if !(0x80 <= b1 && b1 <= 0xBF) || !(0x80 <= b2 && b2 <= 0xBF) || !(0x80 <= b3 && b3 <= 0xBF) {
            proof {
                assert(!is_leading_byte_width_1(sub[0]));
                assert(!is_leading_byte_width_2(sub[0]));
                assert(!is_leading_byte_width_3(sub[0]));
                assert(is_leading_byte_width_4(sub[0]));
                assert(!is_continuation_byte(sub[1]) || !is_continuation_byte(sub[2]) || !is_continuation_byte(sub[3]));
                assert(!valid_leading_and_continuation_bytes_first_codepoint(sub));
                assert(!valid_first_scalar(sub));
            }
            return Utf8CharResult::Err;
        }
        if b0 == 0xF0 && b1 < 0x90 {
            proof {
                assert(valid_leading_and_continuation_bytes_first_codepoint(sub));
                assert(decode_first_codepoint(sub) == codepoint_width_4(sub[0], sub[1], sub[2], sub[3]));
                lemma_4byte_overlong_f0(sub[0], sub[1], sub[2], sub[3]);
                assert(!valid_first_scalar(sub));
            }
            return Utf8CharResult::Err;
        }
        if b0 == 0xF4 && b1 > 0x8F {
            proof {
                assert(valid_leading_and_continuation_bytes_first_codepoint(sub));
                assert(decode_first_codepoint(sub) == codepoint_width_4(sub[0], sub[1], sub[2], sub[3]));
                lemma_4byte_over_max_f4(sub[0], sub[1], sub[2], sub[3]);
                assert(!valid_first_scalar(sub));
            }
            return Utf8CharResult::Err;
        }
        proof {
            assert(is_leading_byte_width_4(sub[0]));
            assert(is_continuation_byte(sub[1]));
            assert(is_continuation_byte(sub[2]));
            assert(is_continuation_byte(sub[3]));
            assert(valid_leading_and_continuation_bytes_first_codepoint(sub));
            assert(decode_first_codepoint(sub) == codepoint_width_4(sub[0], sub[1], sub[2], sub[3]));
            lemma_4byte_not_overlong(sub[0], sub[1], sub[2], sub[3]);
            lemma_4byte_not_surrogate(sub[0], sub[1], sub[2], sub[3]);
        }
        return Utf8CharResult::Ok { len: 4 };
    }

    // Invalid leading byte: 0x80-0xBF, 0xC0-0xC1, 0xF5-0xFF
    proof {
        assert(!is_leading_byte_width_1(sub[0]));
        // is_leading_byte_width_3 requires 0xE0-0xEF — not possible here
        assert(!is_leading_byte_width_3(sub[0]));

        if is_leading_byte_width_2(sub[0]) {
            // b0 must be 0xC0 or 0xC1 (since C2-DF handled above)
            if sub.len() >= 2 && is_continuation_byte(sub[1]) {
                lemma_c0c1_overlong(sub[0], sub[1]);
            }
        }
        if is_leading_byte_width_4(sub[0]) {
            // b0 must be 0xF5-0xF7 (since F0-F4 handled above)
            if sub.len() >= 4 && is_continuation_byte(sub[1])
                && is_continuation_byte(sub[2]) && is_continuation_byte(sub[3])
            {
                lemma_f5f7_over_max(sub[0], sub[1], sub[2], sub[3]);
            }
        }
    }
    Utf8CharResult::Err
}

} // verus!
