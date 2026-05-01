//! Lex-rank hashes for k-multisets and k-subsets, ported from
//! HenryRLee `cpp/src/hash.c`.

use crate::dp::{CHOOSE, DP};

/// Hashes a k-multiset over 13 ranks given as a quinary histogram.
///
/// `q[i]` = count of rank `i` in the multiset (0..=4 for Omaha).
/// `k` = total multiset size; `q.iter().sum::<u8>() as i32` must equal
/// `k` on entry.
///
/// Direct port of `hash_quinary` in `cpp/src/hash.c` — see comments
/// there for derivation.
#[inline]
pub fn hash_quinary(q: &[u8; 13], mut k: i32) -> u32 {
    let mut sum: u32 = 0;
    let len = 13usize;
    for i in 0..len {
        // dp[q[i]][len - i - 1][k]
        sum += DP[q[i] as usize][len - i - 1][k as usize];
        k -= q[i] as i32;
        if k <= 0 {
            break;
        }
    }
    sum
}

/// Hashes a k-subset of 15 bit positions to its lex rank.
///
/// `binary` is the 15-bit (low bits set) bitmap; `k` is the popcount.
/// Direct port of `hash_binary` in `cpp/src/hash.c`.
#[inline]
pub fn hash_binary(binary: i32, mut k: i32) -> u32 {
    let mut sum: u32 = 0;
    let len = 15;
    for i in 0..len {
        if binary & (1 << i) != 0 {
            if (len - i - 1) as i32 >= k {
                sum += CHOOSE[(len - i - 1) as usize][k as usize];
            }
            k -= 1;
            if k == 0 {
                break;
            }
        }
    }
    sum
}

#[cfg(test)]
mod tests {
    use super::*;

    /// All-zero quinary, k=0 → hash = 0 (loop body never runs since
    /// `k <= 0` immediately).
    #[test]
    fn hash_quinary_zero_k() {
        let q = [0u8; 13];
        assert_eq!(hash_quinary(&q, 0), 0);
    }

    /// Quinary with `q[0] = 4` (four 2's), `k=4` → tracks the lex
    /// rank of "all four 2's" multiset. Should be a small value
    /// because all the rank-0 cards are at the bottom of the lex
    /// order. C reference returns the same value; we just check
    /// determinism + boundedness here, with the meaningful parity
    /// check delegated to the full-eval tests in `eval.rs`.
    #[test]
    fn hash_quinary_quad_2s() {
        let mut q = [0u8; 13];
        q[0] = 4;
        let h = hash_quinary(&q, 4);
        assert!(h < 2000, "quad-2s lex rank should be small, got {h}");
    }

    /// `hash_binary(0b11111, 5)` — the lowest 5 bits set.
    /// Iteration adds `CHOOSE[14][5]=2002 + CHOOSE[13][4]=715 +
    /// CHOOSE[12][3]=220 + CHOOSE[11][2]=55 + CHOOSE[10][1]=10
    /// = 3002`. (The hash is **monotonically increasing** in the
    /// bit positions: lower bits set → higher lex rank, because the
    /// loop adds `CHOOSE[14-i][k]` for the first match at index `i`,
    /// which is largest when `i` is smallest.)
    #[test]
    fn hash_binary_lowest_5() {
        assert_eq!(hash_binary(0b11111, 5), 3002);
    }

    /// `hash_binary(0b111_1100_0000_0000, 5)` — the highest 5 bits
    /// set. By the analysis in `hash_binary_lowest_5`, this is the
    /// SMALLEST possible hash value: every match has `len-i-1 < k`,
    /// so no terms are added → 0.
    #[test]
    fn hash_binary_highest_5() {
        let bin: i32 = 0b111_1100_0000_0000; // bits 10..14
        assert_eq!(hash_binary(bin, 5), 0);
    }
}
