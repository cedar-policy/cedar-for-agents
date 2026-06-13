use vstd::prelude::*;

verus! {

/// Spec: two byte sequences are equal
pub open spec fn seq_equal(a: Seq<u8>, b: Seq<u8>) -> bool {
    a =~= b
}

/// Exec: compare two byte slices for equality
pub fn slices_equal(a: &[u8], b: &[u8]) -> (result: bool)
    ensures
        result == (a@ =~= b@),
{
    if a.len() != b.len() {
        return false;
    }
    let mut i: usize = 0;
    while i < a.len()
        invariant
            i <= a@.len(),
            a@.len() == b@.len(),
            forall|j: int| 0 <= j && j < i as int ==> a@[j] == b@[j],
        decreases a@.len() - i,
    {
        if a[i] != b[i] {
            return false;
        }
        i = i + 1;
    }
    assert(a@ =~= b@);
    true
}

/// Check if a list of decoded keys contains any duplicates.
/// Returns `Ok(())` if all keys are unique, or `Err(index)` with the index
/// of the first key that is a duplicate of an earlier key.
///
/// Proven: if Ok, all pairs of keys are distinct.
pub fn check_no_duplicate_keys(keys: &[Vec<u8>]) -> (result: Result<(), usize>)
    ensures
        match result {
            Ok(()) => {
                forall|i: int, j: int|
                    0 <= i && i < j && j < keys@.len()
                    ==> !(keys@[i]@ =~= keys@[j]@)
            },
            Err(dup_idx) => {
                dup_idx < keys@.len()
                && exists|earlier: int|
                    0 <= earlier && earlier < dup_idx as int
                    && keys@[earlier]@ =~= keys@[dup_idx as int]@
            },
        },
{
    let mut i: usize = 0;
    while i < keys.len()
        invariant
            i <= keys@.len(),
            // All keys in [0, i) are pairwise distinct
            forall|j: int, k: int|
                0 <= j && j < k && k < i as int
                ==> !(keys@[j]@ =~= keys@[k]@),
        decreases keys@.len() - i,
    {
        let mut j: usize = 0;
        while j < i
            invariant
                j <= i,
                i < keys@.len(),
                forall|k: int| 0 <= k && k < j as int ==> !(keys@[k]@ =~= keys@[i as int]@),
            decreases i - j,
        {
            if slices_equal(keys[j].as_slice(), keys[i].as_slice()) {
                return Err(i);
            }
            j = j + 1;
        }
        i = i + 1;
    }
    Ok(())
}

} // verus!
