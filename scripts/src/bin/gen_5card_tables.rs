//! Generate 5-card-only OFFSETS_5C / LOOKUP_5C tables for phe-omaha.
//!
//! Goal: shrink the perfect-hash working set from ~192 KB (5/6/7-card
//! support) down to ~30-40 KB so it fits L1d on Alder Lake P-core
//! (48 KB). The Omaha 60-combo loop only ever evaluates 5-card sub-
//! hands, so we don't need 6/7-card support.
//!
//! Algorithm: same single-displacement perfect hash as the production
//! generator (ref Czech-Havas-Majewski 1997 §5.2). We re-bucket only
//! the 5-card rank multisets via FFD with a tighter `OFFSET_SHIFT`
//! parameter, then read the actual rank values out of the existing
//! `phe-holdem-assets::LOOKUP` (which already has the right answer for
//! every 5-card multiset).
//!
//! Output: `crates/omaha/src/lookup_5card.rs` containing
//! `OFFSETS_5C: [i32; N]` and `LOOKUP_5C: [u16; M]` and the
//! `OFFSET_SHIFT_5C: usize` chosen.

#![allow(clippy::needless_range_loop)]

use phe_core::{NUMBER_OF_RANKS, OFFSETS, OFFSET_SHIFT, RANK_BASES};
use phe_holdem_assets::LOOKUP;
use std::cmp::max;
use std::collections::BTreeSet;
use std::fs::File;
use std::io::Write;

/// Offset-shift for the 5-card-only perfect hash. Larger = smaller
/// OFFSETS table but more collisions per bucket → harder for FFD to
/// find a no-collision packing. 12 leaves us with ~4474 OFFSETS
/// entries (~17.5 KB) and gave a successful FFD pack on first try.
const OFFSET_SHIFT_5C: u32 = 12;

fn main() {
    // 1) Enumerate all 5-card rank multisets and their rank_keys.
    let mut keys = BTreeSet::new();
    for i in 0..NUMBER_OF_RANKS {
        for j in i..NUMBER_OF_RANKS {
            for k in j..NUMBER_OF_RANKS {
                for m in k..NUMBER_OF_RANKS {
                    for n in max(m, i + 1)..NUMBER_OF_RANKS {
                        // Reject 5 of one rank (impossible from a deck).
                        // The constraint `n >= i + 1` above already
                        // forces at least 2 distinct ranks.
                        let key = RANK_BASES[i]
                            + RANK_BASES[j]
                            + RANK_BASES[k]
                            + RANK_BASES[m]
                            + RANK_BASES[n];
                        keys.insert(key);
                    }
                }
            }
        }
    }
    let keys: Vec<u64> = keys.into_iter().collect();
    let max_key = *keys.iter().max().unwrap();
    eprintln!(
        "5-card unique multisets: {}, max rank_key: {:#x}",
        keys.len(),
        max_key
    );

    // 2) Bucket by `key >> OFFSET_SHIFT_5C`.
    let bucket_count = ((max_key >> OFFSET_SHIFT_5C) + 1) as usize;
    eprintln!(
        "OFFSET_SHIFT_5C = {}, bucket_count = {}",
        OFFSET_SHIFT_5C, bucket_count
    );

    let bucket_size = 1u64 << OFFSET_SHIFT_5C;
    let mut buckets: Vec<(usize, Vec<u64>)> = (0..bucket_count).map(|i| (i, Vec::new())).collect();
    for &k in &keys {
        let row = (k >> OFFSET_SHIFT_5C) as usize;
        let col = k & (bucket_size - 1);
        buckets[row].1.push(col);
    }
    for (_, cols) in &mut buckets {
        cols.sort_unstable();
    }

    // 3) FFD: process largest buckets first; place each at the smallest
    //    offset that doesn't conflict with already-placed entries.
    buckets.sort_by_key(|(_, cols)| std::cmp::Reverse(cols.len()));

    // `filled` tracks which positions in the final hash space are taken.
    // Worst case it spans `bucket_count << OFFSET_SHIFT_5C` slots.
    let mut filled: Vec<bool> = vec![false; bucket_count << OFFSET_SHIFT_5C];
    let mut offsets: Vec<i32> = vec![i32::MIN; bucket_count];
    let mut least_empty: usize = 0;

    for (idx, cols) in &buckets {
        if cols.is_empty() {
            break;
        }
        // Start probing at `least_empty - cols[0]` so the first column
        // tries the lowest available position.
        let start = least_empty as i64 - cols[0] as i64;
        let mut chosen: i64 = start;
        'search: for off in start.. {
            for &c in cols {
                let pos = c as i64 + off;
                if pos < 0 || (pos as usize) >= filled.len() {
                    continue 'search;
                }
                if filled[pos as usize] {
                    continue 'search;
                }
            }
            chosen = off;
            break;
        }
        offsets[*idx] = chosen as i32;
        for &c in cols {
            let pos = (c as i64 + chosen) as usize;
            filled[pos] = true;
        }
        while least_empty < filled.len() && filled[least_empty] {
            least_empty += 1;
        }
    }
    // Buckets the FFD never touched (empty col list): leave offset = 0
    // so the index into LOOKUP just becomes the rank_key, which won't
    // be visited at runtime anyway. Convert i32::MIN sentinels.
    for (i, off) in offsets.iter_mut().enumerate() {
        if *off == i32::MIN {
            *off = 0;
        } else {
            // The original generator stores offsets as `chosen - i*bucket_size`
            // so that `LOOKUP[OFFSETS[bucket] + key]` = `LOOKUP[chosen + col]`.
            *off -= (i << OFFSET_SHIFT_5C) as i32;
        }
    }

    let image_size = filled.iter().rposition(|&b| b).unwrap_or(0) + 1;
    eprintln!("hash image size: {}", image_size);

    // 4) Build LOOKUP_5C by querying the existing production LOOKUP.
    let mut lookup_5c: Vec<u16> = vec![0; image_size];
    for &k in &keys {
        // production hash → existing LOOKUP value
        let prod_hash = (OFFSETS[(k >> OFFSET_SHIFT) as usize] as i64 + k as i64) as usize;
        let val = LOOKUP[prod_hash];

        // new 5-card hash → store at compact index
        let bucket = (k >> OFFSET_SHIFT_5C) as usize;
        let new_hash = (offsets[bucket] as i64 + k as i64) as usize;
        lookup_5c[new_hash] = val;
    }

    // 5) Write output.
    let path = "crates/omaha/src/lookup_5card.rs";
    let mut file = File::create(path).unwrap();
    writeln!(
        file,
        "//! Auto-generated by `phe-scripts gen-5card-tables`. Do not edit by hand."
    )
    .unwrap();
    writeln!(
        file,
        "//!\n\
         //! 5-card-only perfect-hash for Omaha's inner kernel: same\n\
         //! single-displacement scheme as `phe-core::OFFSETS` /\n\
         //! `phe-holdem-assets::LOOKUP`, but enumerates only 5-card\n\
         //! rank multisets so the table fits L1d on Alder Lake P-core."
    )
    .unwrap();
    writeln!(file).unwrap();
    writeln!(
        file,
        "pub const OFFSET_SHIFT_5C: u32 = {};",
        OFFSET_SHIFT_5C
    )
    .unwrap();
    writeln!(file).unwrap();
    writeln!(
        file,
        "pub const OFFSETS_5C: [i32; {}] = {:?};",
        offsets.len(),
        offsets
    )
    .unwrap();
    writeln!(file).unwrap();
    writeln!(
        file,
        "pub const LOOKUP_5C: [u16; {}] = {:?};",
        lookup_5c.len(),
        lookup_5c
    )
    .unwrap();
    eprintln!("wrote {}", path);

    let offsets_kb = offsets.len() * 4 / 1024;
    let lookup_kb = lookup_5c.len() * 2 / 1024;
    eprintln!(
        "size summary: OFFSETS_5C ~{} KB, LOOKUP_5C ~{} KB, total ~{} KB",
        offsets_kb,
        lookup_kb,
        offsets_kb + lookup_kb
    );
}
