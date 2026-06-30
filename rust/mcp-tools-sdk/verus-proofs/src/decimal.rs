use vstd::prelude::*;

verus! {

// =============================================================================
// Decimal overflow checking
// =============================================================================

/// Spec: 10^n for n in 0..=4
pub open spec fn spec_pow10(n: nat) -> int
    decreases n,
{
    if n == 0 { 1 }
    else { 10 * spec_pow10((n - 1) as nat) }
}

proof fn lemma_pow10_values()
    ensures
        spec_pow10(0) == 1,
        spec_pow10(1) == 10,
        spec_pow10(2) == 100,
        spec_pow10(3) == 1000,
        spec_pow10(4) == 10000,
{
}

/// Exec: compute 10^n for n in 0..=4.
pub fn pow10(n: u32) -> (result: i64)
    requires
        n <= 4,
    ensures
        result == spec_pow10(n as nat),
        result > 0,
{
    proof { lemma_pow10_values(); }
    if n == 0 { 1 }
    else if n == 1 { 10 }
    else if n == 2 { 100 }
    else if n == 3 { 1000 }
    else { 10000 }
}

/// Checks whether the scaled decimal value fits in i64.
///
/// Returns `true` iff `int_val * 10000 + frac_digits * 10^(4-frac_len) * sign` fits in i64.
pub fn decimal_overflow_check(int_val: i64, frac_digits: i64, frac_len: u32, is_neg: bool) -> (result: bool)
    requires
        1 <= frac_len <= 4,
        0 <= frac_digits <= 9999,
    ensures
        result ==> {
            let frac_signed = if is_neg { -frac_digits * spec_pow10((4 - frac_len) as nat) }
                              else { frac_digits * spec_pow10((4 - frac_len) as nat) };
            let scaled = int_val as int * 10000 + frac_signed;
            i64::MIN <= scaled <= i64::MAX
        },
{
    proof { lemma_pow10_values(); }
    let frac_scale: i64 = pow10(4 - frac_len);
    let frac_val: i64 = if is_neg {
        -frac_digits * frac_scale
    } else {
        frac_digits * frac_scale
    };

    let scaled_int: i64 = match int_val.checked_mul(10000) {
        Some(v) => v,
        None => return false,
    };

    scaled_int.checked_add(frac_val).is_some()
}

} // verus!
