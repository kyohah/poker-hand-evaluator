//! Generates the 2-7 lowball lookup tables for `phe-deuce-seven-assets`.
//!
//! ## Why 5 cards only
//!
//! The holdem-shape lookup design assumes that whenever 5+ cards of one
//! suit are present, the best 5-card hand is determined by the flush
//! pattern alone (so `lookup_flush[flush_key]` is a function of
//! `flush_key`). That holds for Hold'em high (Flush > all non-Flush
//! categories that fit alongside 5+ same-suit cards), but breaks for
//! 2-7 lowball: the *worst* Hold'em hand wins, so the player will
//! often prefer to drop a same-suit card and use a non-flush sub-hand.
//! Two 7-card hands with the same flush_key but different non-flush
//! cards then produce different best-2-7 answers, which is a true
//! lookup-table collision.
//!
//! 2-7 is realistically played as a 5-card draw (single / triple draw),
//! so we restrict the table to 5-card hands. 6/7-card evaluation panics
//! at the wrapper level rather than silently returning a wrong answer.
//!
//! ## Pipeline (5-card only)
//!  1. Enumerate every 5-card hand, score it with the naive evaluator
//!     under `WheelMode::NoPair` (A-2-3-4-5 = no-pair, not a straight).
//!     Bucket packed ranks by category.
//!  2. Within each category, sort packed ranks ascending and assign
//!     within-category indices 0..N. Build a `u32 -> u16`
//!     compactification map where the u16 is `(category << 12) + index`.
//!  3. Enumerate 5-card hands once more, store the compact u16 in
//!     `LOOKUP` (perfect-hash keyed) or `LOOKUP_FLUSH` (13-bit pattern
//!     keyed) per the same flush-vs-rank dispatch as Hold'em.
//!
//! Output: `crates/deuce-seven-assets/src/lookup.rs`.

use phe_core::{CARDS, FLUSH_MASK, NUMBER_OF_CARDS, OFFSETS, OFFSET_SHIFT, SUIT_SHIFT};
use phe_scripts::naive_high::{eval_5cards, WheelMode};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs::File;
use std::io::Write;

const MODE: WheelMode = WheelMode::NoPair;

#[inline]
fn add_card(key: u64, mask: u64, card: usize) -> (u64, u64) {
    let (k, m) = unsafe { *CARDS.get_unchecked(card) };
    (key.wrapping_add(k), mask.wrapping_add(m))
}

/// Build the `u32 packed -> u16 compact` map by enumerating all C(52,5)
/// 5-card hands.
fn build_compactification() -> HashMap<u32, u16> {
    let mut by_category: BTreeMap<u8, BTreeSet<u32>> = BTreeMap::new();

    for i in 0..(NUMBER_OF_CARDS - 4) {
        for j in (i + 1)..(NUMBER_OF_CARDS - 3) {
            for k in (j + 1)..(NUMBER_OF_CARDS - 2) {
                for m in (k + 1)..(NUMBER_OF_CARDS - 1) {
                    for n in (m + 1)..NUMBER_OF_CARDS {
                        let v = eval_5cards(&[i, j, k, m, n], MODE);
                        let cat = (v >> 26) as u8;
                        by_category.entry(cat).or_default().insert(v);
                    }
                }
            }
        }
    }

    let mut map = HashMap::new();
    for (cat, ranks) in &by_category {
        for (idx, &r) in ranks.iter().enumerate() {
            assert!(idx < 4096, "category {} overflowed 12-bit index", cat);
            let compact = ((*cat as u16) << 12) | (idx as u16);
            map.insert(r, compact);
        }
        eprintln!("  category {}: {} distinct ranks", cat, ranks.len());
    }
    map
}

fn store(
    lookup: &mut HashMap<usize, u16>,
    lookup_flush: &mut HashMap<usize, u16>,
    key: u64,
    mask: u64,
    val: u16,
) {
    let is_flush = key & FLUSH_MASK;
    if is_flush > 0 {
        let flush_key = (mask >> (4 * is_flush.leading_zeros())) as u16;
        if let Some(prev) = lookup_flush.insert(flush_key as usize, val) {
            assert_eq!(prev, val, "flush collision")
        }
    } else {
        let rank_key = key as u32 as usize;
        let offset = OFFSETS[rank_key >> OFFSET_SHIFT] as usize;
        let hash_key = rank_key.wrapping_add(offset);
        if let Some(prev) = lookup.insert(hash_key, val) {
            assert_eq!(prev, val, "rank collision")
        }
    }
}

fn main() {
    eprintln!("phase 1: building 32-bit -> 16-bit compactification map");
    let compact = build_compactification();
    eprintln!(
        "  total distinct 32-bit ranks: {} (expect 7463 for 2-7 with wheel-as-no-pair)",
        compact.len()
    );

    eprintln!("phase 2: filling lookup tables");
    let mut lookup: HashMap<usize, u16> = HashMap::new();
    let mut lookup_flush: HashMap<usize, u16> = HashMap::new();
    let initial_key = 0x3333u64 << SUIT_SHIFT;

    // 5-cards only — see module-doc rationale.
    for i in 0..(NUMBER_OF_CARDS - 4) {
        let (key, mask) = add_card(initial_key, 0, i);
        for j in (i + 1)..(NUMBER_OF_CARDS - 3) {
            let (key, mask) = add_card(key, mask, j);
            for k in (j + 1)..(NUMBER_OF_CARDS - 2) {
                let (key, mask) = add_card(key, mask, k);
                for m in (k + 1)..(NUMBER_OF_CARDS - 1) {
                    let (key, mask) = add_card(key, mask, m);
                    for n in (m + 1)..NUMBER_OF_CARDS {
                        let (key, mask) = add_card(key, mask, n);
                        let raw = eval_5cards(&[i, j, k, m, n], MODE);
                        store(&mut lookup, &mut lookup_flush, key, mask, compact[&raw]);
                    }
                }
            }
        }
    }
    eprintln!(
        "  5-cards done: {} rank entries, {} flush entries",
        lookup.len(),
        lookup_flush.len()
    );

    let max_rank = *lookup.keys().max().unwrap();
    let max_flush = *lookup_flush.keys().max().unwrap();
    let mut lookup_vec = vec![0u16; max_rank + 1];
    let mut lookup_flush_vec = vec![0u16; max_flush + 1];

    for (k, v) in &lookup {
        lookup_vec[*k] = *v;
    }
    for (k, v) in &lookup_flush {
        lookup_flush_vec[*k] = *v;
    }

    let path = "crates/deuce-seven-assets/src/lookup.rs";
    let mut file = File::create(path).unwrap();
    writeln!(
        file,
        "//! Auto-generated by scripts/gen-deuce-seven-lookup. Do not edit by hand."
    )
    .unwrap();
    writeln!(file).unwrap();
    writeln!(
        file,
        "pub const LOOKUP: [u16; {}] = {:?};",
        lookup_vec.len(),
        lookup_vec
    )
    .unwrap();
    writeln!(file).unwrap();
    writeln!(
        file,
        "pub const LOOKUP_FLUSH: [u16; {}] = {:?};",
        lookup_flush_vec.len(),
        lookup_flush_vec
    )
    .unwrap();
    eprintln!("wrote {}", path);
}
