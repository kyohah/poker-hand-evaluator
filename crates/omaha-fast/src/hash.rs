//! Lex-rank hashes for k-multisets and k-subsets, ported from
//! HenryRLee `cpp/src/hash.c`.
//!
//! Hot-path array accesses use `get_unchecked` since the C reference
//! is itself UB on out-of-range inputs and the only callers
//! (`evaluator_plo4.c` semantics) have already validated the indices.
//! Bounds in valid PLO4 input:
//! * `q[i]` ∈ `[0, 4]` (max 4 cards per rank in a 52-card deck);
//!   `DP` first dim is 5.
//! * `len - i - 1` ∈ `[0, 12]`; `DP` second dim is 14.
//! * `k` ∈ `[0, 5]`; `DP`/`CHOOSE` third dim is 10.
//! * `i` ∈ `[0, 14]` for `hash_binary`; `binary` is at most 15 bits.

pub use crate::dp::{CHOOSE, DP};

/// Hashes a 5-multiset (k=5) over 13 ranks given as a quinary
/// histogram. `k` must equal `q.iter().sum::<u8>() as i32`.
///
/// Original early-exit form: bails out as soon as `k` reaches 0,
/// typically after 5-7 iterations. Best for the single-hand eval
/// path where each call is on the critical latency path.
#[inline(always)]
pub fn hash_quinary(q: &[u8; 13], mut k: i32) -> u32 {
    let mut sum: u32 = 0;
    let len = 13usize;
    unsafe {
        for i in 0..len {
            let qi = *q.get_unchecked(i);
            sum += *DP
                .get_unchecked(qi as usize)
                .get_unchecked(len - i - 1)
                .get_unchecked(k as usize);
            k -= qi as i32;
            if k <= 0 {
                break;
            }
        }
    }
    sum
}

/// Branchless variant of `hash_quinary` — every 13-iteration runs to
/// completion, with contributions from `k <= 0` masked out via a
/// cmov-style select. Slightly more work per call than the early-exit
/// form, but the lack of branches lets LLVM unroll / vectorize the
/// outer loop in batch contexts. Use this from the batch's pass-1
/// where 100 000+ calls in a tight loop benefit from outer-loop
/// vectorization.
#[inline(always)]
pub fn hash_quinary_branchless(q: &[u8; 13], k_init: i32) -> u32 {
    let mut sum: u32 = 0;
    let mut k = k_init;
    unsafe {
        for i in 0..13 {
            let qi = *q.get_unchecked(i) as usize;
            let kk = k.clamp(0, 9) as usize;
            let raw = *DP.get_unchecked(qi).get_unchecked(12 - i).get_unchecked(kk);
            let mask: u32 = ((k > 0) as u32).wrapping_neg();
            sum = sum.wrapping_add(raw & mask);
            k -= *q.get_unchecked(i) as i32;
        }
    }
    sum
}

/// Hashes a k-subset of 15 bit positions to its lex rank.
#[inline(always)]
pub fn hash_binary(binary: i32, mut k: i32) -> u32 {
    let mut sum: u32 = 0;
    let len: i32 = 15;
    // SAFETY: (len-i-1) ∈ [0,14], k ∈ [0,5] — within CHOOSE's
    // [53][10] bounds. The CHOOSE table is statically larger than
    // needed for these indices.
    unsafe {
        for i in 0..len {
            if binary & (1 << i) != 0 {
                if (len - i - 1) >= k {
                    sum += *CHOOSE
                        .get_unchecked((len - i - 1) as usize)
                        .get_unchecked(k as usize);
                }
                k -= 1;
                if k == 0 {
                    break;
                }
            }
        }
    }
    sum
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_quinary_zero_k() {
        let q = [0u8; 13];
        assert_eq!(hash_quinary(&q, 0), 0);
    }

    #[test]
    fn hash_quinary_quad_2s() {
        let mut q = [0u8; 13];
        q[0] = 4;
        let h = hash_quinary(&q, 4);
        assert!(h < 2000, "quad-2s lex rank should be small, got {h}");
    }

    #[test]
    fn hash_binary_lowest_5() {
        // CHOOSE[14][5]+CHOOSE[13][4]+CHOOSE[12][3]+CHOOSE[11][2]+CHOOSE[10][1]
        // = 2002 + 715 + 220 + 55 + 10 = 3002
        assert_eq!(hash_binary(0b11111, 5), 3002);
    }

    #[test]
    fn hash_binary_highest_5() {
        let bin: i32 = 0b111_1100_0000_0000;
        assert_eq!(hash_binary(bin, 5), 0);
    }
}
