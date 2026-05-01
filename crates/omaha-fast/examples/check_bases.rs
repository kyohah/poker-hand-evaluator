//! Phase 2a feasibility check.
//!
//! Q: Do `phe_core::RANK_BASES` give unique sums for every 4-card and
//! every 5-card rank multiset over 13 ranks (each rank 0..=4 copies)?
//! If yes, we can replace `hash_quinary`'s O(13) loop with a simple
//! cumulative add — saving ~5-10 ns / eval.

use phe_core::RANK_BASES;
use std::collections::HashMap;

fn main() {
    println!("RANK_BASES = {:?}", RANK_BASES);
    println!();

    for k in [4usize, 5] {
        let mut seen: HashMap<u64, [u8; 13]> = HashMap::new();
        let mut count = 0usize;
        let mut max_sum = 0u64;
        let mut min_sum = u64::MAX;

        // Enumerate every k-multiset of 13 ranks with each rank in 0..=4.
        let mut q = [0u8; 13];
        enumerate(&mut q, 0, k as u8, &mut |q| {
            // sum of bases according to q
            let mut s = 0u64;
            for r in 0..13 {
                s = s.wrapping_add(RANK_BASES[r] * q[r] as u64);
            }
            count += 1;
            max_sum = max_sum.max(s);
            min_sum = min_sum.min(s);
            if let Some(prev) = seen.insert(s, *q) {
                if prev != *q {
                    println!("  COLLISION at sum 0x{s:x}: {prev:?} vs {q:?}");
                }
            }
        });

        let unique = seen.len();
        println!("k = {k}:");
        println!("  total multisets: {count}");
        println!("  distinct sums:   {unique}");
        println!("  collisions:      {}", count - unique);
        println!("  sum range:       0x{:x} … 0x{:x}", min_sum, max_sum);
        println!("  range as decimal: {min_sum} … {max_sum}");
        println!();
    }
}

/// Enumerate k-multisets of 13 ranks with each rank in 0..=4 by
/// filling `q[i]` with how many copies of rank i.
fn enumerate(q: &mut [u8; 13], i: usize, remaining: u8, f: &mut impl FnMut(&[u8; 13])) {
    if i == 13 {
        if remaining == 0 {
            f(q);
        }
        return;
    }
    let max_for_rank = remaining.min(4);
    for c in 0..=max_for_rank {
        q[i] = c;
        enumerate(q, i + 1, remaining - c, f);
    }
    q[i] = 0;
}
