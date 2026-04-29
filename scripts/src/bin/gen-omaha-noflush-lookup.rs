//! Generator for `phe-omaha-assets/src/noflush_lookup.bin`.
//!
//! Path 1 of `OmahaHighRule::evaluate` is invoked when no 5-card
//! flush combo is reachable from the (4 hole, 5 board) layout —
//! i.e., no suit has both ≥2 hole and ≥3 board cards. Under that
//! premise the answer depends only on the *rank multisets* of hole
//! and board: pick the best (2 from hole, 3 from board) sub-hand by
//! Hold'em-high rank.
//!
//! This generator enumerates every valid pair of
//!   - 4-card rank multiset over 13 ranks (`C(16, 4) = 1820`)
//!   - 5-card rank multiset over 13 ranks (`C(17, 5) = 6188`)
//! and for each pair runs the 60-combo enumeration through the
//! existing Hold'em rank-only lookup (`OFFSETS + LOOKUP`), recording
//! the maximum.
//!
//! Output: a flat `[u16; NUM_HOLE * NUM_BOARD]` written little-endian
//! to `crates/omaha-assets/src/noflush_lookup.bin`. At runtime,
//! `phe-omaha`'s `evaluate_no_flush_path` computes
//! `hole_idx * NUM_BOARD + board_idx` (combinatorial number system
//! over the sorted rank lists) and does a single load.
//!
//! Slots that are unreachable (per-rank deck constraint: hole_count +
//! board_count ≤ 4) stay 0 — they are never indexed at runtime.

use phe_core::{OFFSETS, OFFSET_SHIFT, RANK_BASES};
use phe_holdem_assets::LOOKUP;
use std::fs::File;
use std::io::Write;

const NUM_RANKS: usize = 13;
const NUM_HOLE: usize = 1820; // C(16, 4)
const NUM_BOARD: usize = 6188; // C(17, 5)

const HOLE_PAIRS: [(usize, usize); 6] =
    [(0, 1), (0, 2), (0, 3), (1, 2), (1, 3), (2, 3)];

const BOARD_TRIPLES: [(usize, usize, usize); 10] = [
    (0, 1, 2),
    (0, 1, 3),
    (0, 1, 4),
    (0, 2, 3),
    (0, 2, 4),
    (0, 3, 4),
    (1, 2, 3),
    (1, 2, 4),
    (1, 3, 4),
    (2, 3, 4),
];

/// Pre-computed binomial table covering all values needed by the
/// hole/board encoders. `BINOM[a][b] = C(a, b)`. We only need
/// `a <= 16, b <= 5`, so use 17x6.
fn binom_table() -> [[u32; 6]; 17] {
    let mut t = [[0u32; 6]; 17];
    for a in 0..17 {
        t[a][0] = 1;
        for b in 1..6 {
            t[a][b] = if a == 0 { 0 } else { t[a - 1][b - 1] + t[a - 1][b] };
        }
    }
    t
}

/// Multiset combinatorial-number-system encoder for sorted-ascending
/// 4 ranks. Returns dense index in `[0, NUM_HOLE)`.
#[inline]
fn hole_index(sorted: &[usize; 4], binom: &[[u32; 6]; 17]) -> usize {
    // y_i = x_i + (i-1), y is strict-increasing.
    // index = C(y_1, 1) + C(y_2, 2) + C(y_3, 3) + C(y_4, 4).
    (binom[sorted[0]][1]
        + binom[sorted[1] + 1][2]
        + binom[sorted[2] + 2][3]
        + binom[sorted[3] + 3][4]) as usize
}

/// Multiset combinatorial-number-system encoder for sorted-ascending
/// 5 ranks. Returns dense index in `[0, NUM_BOARD)`.
#[inline]
fn board_index(sorted: &[usize; 5], binom: &[[u32; 6]; 17]) -> usize {
    (binom[sorted[0]][1]
        + binom[sorted[1] + 1][2]
        + binom[sorted[2] + 2][3]
        + binom[sorted[3] + 3][4]
        + binom[sorted[4] + 4][5]) as usize
}

/// 5-card non-flush evaluator using the existing Hold'em rank-only
/// perfect hash. Caller passes 5 ranks (multiset). The result is the
/// canonical Hold'em-high u16 rank for the 5-card hand interpreted as
/// non-flush.
#[inline]
fn eval_no_flush_5(ranks: [usize; 5]) -> u16 {
    let mut rk: u32 = 0;
    for r in ranks {
        rk = rk.wrapping_add(RANK_BASES[r] as u32);
    }
    let rk = rk as usize;
    let offset = OFFSETS[rk >> OFFSET_SHIFT] as usize;
    LOOKUP[rk.wrapping_add(offset)]
}

fn main() {
    let binom = binom_table();
    let mut table = vec![0u16; NUM_HOLE * NUM_BOARD];

    eprintln!(
        "enumerating {} hole multisets x up to {} board multisets...",
        NUM_HOLE, NUM_BOARD,
    );

    let mut filled: u64 = 0;

    // Per-rank counts after assigning a hole multiset: needed to
    // enforce `hole_count[r] + board_count[r] <= 4` when iterating
    // board multisets.
    let mut hole_count = [0u8; NUM_RANKS];

    // Iterate all 4-card rank multisets in ascending-rank order.
    for h0 in 0..NUM_RANKS {
        for h1 in h0..NUM_RANKS {
            for h2 in h1..NUM_RANKS {
                for h3 in h2..NUM_RANKS {
                    let hole = [h0, h1, h2, h3];
                    // Per-rank deck constraint applied also to hole
                    // alone (≤4 of one rank).
                    hole_count.fill(0);
                    for &r in &hole {
                        hole_count[r] += 1;
                    }
                    if hole_count.iter().any(|&c| c > 4) {
                        continue;
                    }

                    let h_idx = hole_index(&hole, &binom);

                    // Iterate all 5-card rank multisets.
                    for b0 in 0..NUM_RANKS {
                        for b1 in b0..NUM_RANKS {
                            for b2 in b1..NUM_RANKS {
                                for b3 in b2..NUM_RANKS {
                                    for b4 in b3..NUM_RANKS {
                                        let board = [b0, b1, b2, b3, b4];

                                        // Per-rank deck constraint
                                        // (board ≤4 + combined ≤4).
                                        let mut combined =
                                            hole_count;
                                        let mut bad = false;
                                        for &r in &board {
                                            combined[r] += 1;
                                            if combined[r] > 4 {
                                                bad = true;
                                                break;
                                            }
                                        }
                                        if bad {
                                            continue;
                                        }

                                        let b_idx =
                                            board_index(&board, &binom);

                                        // 60-combo max via non-flush eval.
                                        let mut best: u16 = 0;
                                        for &(i, j) in &HOLE_PAIRS {
                                            for &(a, b, c) in
                                                &BOARD_TRIPLES
                                            {
                                                let r = eval_no_flush_5([
                                                    hole[i], hole[j],
                                                    board[a], board[b],
                                                    board[c],
                                                ]);
                                                if r > best {
                                                    best = r;
                                                }
                                            }
                                        }

                                        table[h_idx * NUM_BOARD + b_idx] =
                                            best;
                                        filled += 1;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    eprintln!("filled {} valid (hole, board) pairs", filled);

    // Serialize as little-endian u16.
    let path = "crates/omaha-assets/src/noflush_lookup.bin";
    let mut file = File::create(path).expect("open output file");
    let mut buf = Vec::with_capacity(table.len() * 2);
    for v in &table {
        buf.extend_from_slice(&v.to_le_bytes());
    }
    file.write_all(&buf).expect("write output");
    eprintln!(
        "wrote {} ({} bytes)",
        path,
        buf.len(),
    );
}
