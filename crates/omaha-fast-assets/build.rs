//! Generates `flush_plo4.bin` and `noflush_plo4.bin` into `OUT_DIR`.
//!
//! Replaces the previously-committed textual `pub const FLUSH_PLO4`
//! and `pub const NOFLUSH_PLO4` arrays (~30 MB binary, ~90 MB
//! textual). Algorithm: enumerate every reachable
//! (board, hole) rank pattern, run the 60-combo "best of (2 from
//! hole, 3 from board)" PLO sub-hand selection, score each via
//! the existing `NO_FLUSH_5` / `FLUSH_5` 5-card Cactus-Kev rank
//! tables, take the min (lower = stronger), index by the same
//! `hash_quinary` / `hash_binary` perfect-hash scheme that
//! `evaluator_plo4.c` uses at runtime.
//!
//! Runs once per fresh `cargo build`; `OUT_DIR` is cached by cargo
//! across incremental builds. Total build cost on a modern x86_64
//! is ~5-10 s.

use std::env;
use std::fs;
use std::io::Write;
use std::path::PathBuf;

const NUM_RANKS: usize = 13;

// PLO sub-hand enumeration: 6 hole pairs × 10 board triples = 60.
const HOLE_PAIRS: [(usize, usize); 6] = [(0, 1), (0, 2), (0, 3), (1, 2), (1, 3), (2, 3)];

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

// Padding constants from `evaluator_plo4.c`. Used when scb < 5 or
// sch < 4: extends the per-suit rank bitmap into the high bits so
// `hash_binary` produces a 5- or 4-card subset rank that includes
// "imaginary" padding positions (which are never actually selected
// in the 60-combo enumeration since they correspond to no real card).
const PADDING: [u32; 3] = [0x0000, 0x2000, 0x6000];

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/no_flush_5.bin");
    println!("cargo:rerun-if-changed=src/flush_5.bin");

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());

    let no_flush_5 = read_u16_le(&manifest.join("src").join("no_flush_5.bin"), 6175);
    let flush_5 = read_u16_le(&manifest.join("src").join("flush_5.bin"), 8192);

    let dp = build_dp();
    let choose = build_choose();

    let noflush_plo4 = gen_noflush_plo4(&no_flush_5, &dp);
    let flush_plo4 = gen_flush_plo4(&flush_5, &choose);

    write_u16_le(&out_dir.join("noflush_plo4.bin"), &noflush_plo4);
    write_u16_le(&out_dir.join("flush_plo4.bin"), &flush_plo4);
}

fn read_u16_le(path: &std::path::Path, expected_len: usize) -> Vec<u16> {
    let bytes = fs::read(path).unwrap_or_else(|e| panic!("read {}: {}", path.display(), e));
    assert_eq!(
        bytes.len(),
        expected_len * 2,
        "{} expected {} bytes, got {}",
        path.display(),
        expected_len * 2,
        bytes.len()
    );
    bytes
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect()
}

fn write_u16_le(path: &std::path::Path, data: &[u16]) {
    let mut f =
        fs::File::create(path).unwrap_or_else(|e| panic!("create {}: {}", path.display(), e));
    let mut buf = Vec::with_capacity(data.len() * 2);
    for &v in data {
        buf.extend_from_slice(&v.to_le_bytes());
    }
    f.write_all(&buf).unwrap();
}

// ------------------------------------------------------------------
// DP and CHOOSE tables (small, computed from formulas).
// ------------------------------------------------------------------

/// `DP[q][n][k]` matches `crates/omaha-fast/src/dp.rs::DP` exactly:
/// it gives the lex-rank-prefix sum used in `hash_quinary`.
///
/// Concretely, `DP[q][n][k] = sum over j=0..q-1 of MS(n, k-j)` where
/// `MS(n, k)` = number of `n`-multisets with values in `[0, 4]`
/// summing to `k` (per-rank deck constraint of 4).
fn build_dp() -> Vec<Vec<Vec<u32>>> {
    // ms[n][k]: multisets of n positions, each value in [0, 4], sum = k.
    let mut ms = vec![vec![0u32; 14]; 14];
    ms[0][0] = 1;
    for n in 1..14 {
        for k in 0..14 {
            let mut s = 0u32;
            for j in 0..=4u32 {
                if k >= j as usize {
                    s += ms[n - 1][k - j as usize];
                }
            }
            ms[n][k] = s;
        }
    }
    // dp[q][n][k] = sum_{j=0}^{q-1} ms[n][k-j].
    let mut dp = vec![vec![vec![0u32; 10]; 14]; 5];
    for (q, dp_q) in dp.iter_mut().enumerate() {
        for (n, dp_qn) in dp_q.iter_mut().enumerate() {
            for (k, dp_qnk) in dp_qn.iter_mut().enumerate() {
                let mut s = 0u32;
                for j in 0..q {
                    if k >= j {
                        s += ms[n][k - j];
                    }
                }
                *dp_qnk = s;
            }
        }
    }
    dp
}

/// Pascal's triangle: `CHOOSE[n][k] = C(n, k)`.
fn build_choose() -> Vec<Vec<u32>> {
    let mut t = vec![vec![0u32; 10]; 53];
    for n in 0..53 {
        t[n][0] = 1;
        for k in 1..10 {
            if n == 0 {
                t[n][k] = 0;
            } else {
                t[n][k] = t[n - 1][k - 1] + t[n - 1][k];
            }
        }
    }
    t
}

// ------------------------------------------------------------------
// Perfect-hash functions (port of `crates/omaha-fast/src/hash.rs`).
// ------------------------------------------------------------------

fn hash_quinary(q: &[u8; 13], k_init: i32, dp: &[Vec<Vec<u32>>]) -> u32 {
    let mut sum: u32 = 0;
    let mut k = k_init;
    for i in 0..13 {
        let qi = q[i] as usize;
        sum = sum.wrapping_add(dp[qi][12 - i][k as usize]);
        k -= qi as i32;
        if k <= 0 {
            break;
        }
    }
    sum
}

fn hash_binary(binary: u32, mut k: i32, choose: &[Vec<u32>]) -> u32 {
    let mut sum: u32 = 0;
    let len: i32 = 15;
    for i in 0..len {
        if binary & (1 << i) != 0 {
            let n = len - i - 1;
            if n >= k {
                sum = sum.wrapping_add(choose[n as usize][k as usize]);
            }
            k -= 1;
            if k == 0 {
                break;
            }
        }
    }
    sum
}

// ------------------------------------------------------------------
// NOFLUSH_PLO4 generation.
// ------------------------------------------------------------------

const NUM_HOLE_QUINARY: usize = 1820; // C(16, 4)
const NUM_BOARD_QUINARY: usize = 6175; // C(17, 5) - 13 (5-of-a-rank cases excluded)
const NOFLUSH_PLO4_LEN: usize = NUM_HOLE_QUINARY * NUM_BOARD_QUINARY; // 11_238_500

fn gen_noflush_plo4(no_flush_5: &[u16], dp: &[Vec<Vec<u32>>]) -> Vec<u16> {
    let mut out = vec![0u16; NOFLUSH_PLO4_LEN];

    // Outer loop: enumerate every 5-card rank multiset (board), with
    // per-rank ≤ 4. For each: enumerate every 4-card rank multiset
    // (hole), with combined ≤ 4 per rank. Compute the 60-combo min
    // and store at NOFLUSH_PLO4[board_hash * 1820 + hole_hash].
    let mut board = [0u8; 13];
    enumerate_quinary_5(&mut board, 0, 5, &mut |board| {
        let mut q_board = [0u8; 13];
        q_board.copy_from_slice(board);
        let board_hash = hash_quinary(&q_board, 5, dp) as usize;

        // Reconstruct 5-card sorted rank list for sub-hand picks.
        let board_ranks = expand_quinary(&q_board);

        let mut hole = [0u8; 13];
        enumerate_quinary_4(&mut hole, 0, 4, &q_board, &mut |hole| {
            let mut q_hole = [0u8; 13];
            q_hole.copy_from_slice(hole);
            let hole_hash = hash_quinary(&q_hole, 4, dp) as usize;

            let hole_ranks = expand_quinary(&q_hole);

            // 60-combo min (lower = stronger).
            let mut best: u16 = u16::MAX;
            for &(i, j) in &HOLE_PAIRS {
                for &(a, b, c) in &BOARD_TRIPLES {
                    let sub = [
                        hole_ranks[i],
                        hole_ranks[j],
                        board_ranks[a],
                        board_ranks[b],
                        board_ranks[c],
                    ];
                    let sub_quinary = quinary_of(&sub);
                    let h = hash_quinary(&sub_quinary, 5, dp) as usize;
                    let r = no_flush_5[h];
                    if r < best {
                        best = r;
                    }
                }
            }

            out[board_hash * NUM_HOLE_QUINARY + hole_hash] = best;
        });
    });

    out
}

/// Convert a quinary multiset histogram into a sorted list of ranks
/// (with multiplicities). E.g., `[2, 0, 1, 0, 0, 0, 1, 0, 1, 0, 0, 0, 0]`
/// → `[0, 0, 2, 6, 8]` (lowest first).
fn expand_quinary(q: &[u8; 13]) -> Vec<u8> {
    let mut out = Vec::with_capacity(13);
    for r in 0..13u8 {
        for _ in 0..q[r as usize] {
            out.push(r);
        }
    }
    out
}

fn quinary_of(ranks: &[u8; 5]) -> [u8; 13] {
    let mut q = [0u8; 13];
    for &r in ranks {
        q[r as usize] += 1;
    }
    q
}

/// Enumerate all 5-card rank multisets over 13 ranks with per-rank
/// count ≤ 4 (i.e., reachable from a real deck). Calls `f` with the
/// quinary histogram for each.
fn enumerate_quinary_5(
    q: &mut [u8; 13],
    rank: usize,
    remaining: u8,
    f: &mut impl FnMut(&[u8; 13]),
) {
    if rank == NUM_RANKS {
        if remaining == 0 {
            f(q);
        }
        return;
    }
    let max = remaining.min(4);
    for c in 0..=max {
        q[rank] = c;
        enumerate_quinary_5(q, rank + 1, remaining - c, f);
    }
    q[rank] = 0;
}

/// Enumerate all 4-card rank multisets given an existing board
/// histogram (combined ≤ 4 per rank).
fn enumerate_quinary_4(
    q: &mut [u8; 13],
    rank: usize,
    remaining: u8,
    board: &[u8; 13],
    f: &mut impl FnMut(&[u8; 13]),
) {
    if rank == NUM_RANKS {
        if remaining == 0 {
            f(q);
        }
        return;
    }
    let max = remaining.min(4 - board[rank]); // ≤4 combined
    for c in 0..=max {
        q[rank] = c;
        enumerate_quinary_4(q, rank + 1, remaining - c, board, f);
    }
    q[rank] = 0;
}

// ------------------------------------------------------------------
// FLUSH_PLO4 generation.
// ------------------------------------------------------------------

const FLUSH_PLO4_BOARD_HASHES: usize = 3003; // C(15, 5)
const FLUSH_PLO4_HOLE_HASHES: usize = 1365; // C(15, 4)
const FLUSH_PLO4_LEN: usize = FLUSH_PLO4_BOARD_HASHES * FLUSH_PLO4_HOLE_HASHES; // 4_099_095

fn gen_flush_plo4(flush_5: &[u16], choose: &[Vec<u32>]) -> Vec<u16> {
    let mut out = vec![0u16; FLUSH_PLO4_LEN];

    // Reachable PLO flush layouts: scb ∈ {3,4,5}, sch ∈ {2,3,4},
    // disjoint card ranks (single-suit subset). The (3, 2) case is
    // handled at runtime via `FLUSH[sb_b | sb_h]` directly, but we
    // still populate the corresponding FLUSH_PLO4 slots so the
    // table is well-formed.
    for scb in 3..=5usize {
        for sch in 2..=4usize {
            // Runtime shortcut: (scb=3, sch=2) is the exactly-5-card
            // case and `evaluator_plo4.c` looks it up directly in
            // `FLUSH[sb_b | sb_h]` instead of going through
            // FLUSH_PLO4. Match HenryRLee's table by leaving these
            // slots at 0.
            if scb == 3 && sch == 2 {
                continue;
            }
            // Iterate every 13-bit board pattern with `popcount = scb`.
            for board_bits in subsets_of_n(13, scb) {
                // Iterate hole patterns with disjoint cards.
                for hole_bits in subsets_of_n(13, sch) {
                    if board_bits & hole_bits != 0 {
                        continue;
                    }

                    // 60-combo enumeration: pick exactly 3 of the
                    // `scb` board flush bits and 2 of the `sch` hole
                    // flush bits. Each sub-hand is a 5-bit single-suit
                    // pattern → eval via `flush_5`.
                    let board_set: Vec<u32> = (0..13)
                        .filter(|i| board_bits & (1 << i) != 0)
                        .map(|i| 1u32 << i)
                        .collect();
                    let hole_set: Vec<u32> = (0..13)
                        .filter(|i| hole_bits & (1 << i) != 0)
                        .map(|i| 1u32 << i)
                        .collect();

                    let mut best: u16 = u16::MAX;
                    for ht in combinations_of(&hole_set, 2) {
                        let hole_picked = ht.iter().fold(0u32, |a, &b| a | b);
                        for bt in combinations_of(&board_set, 3) {
                            let bp = bt.iter().fold(0u32, |a, &b| a | b);
                            let pattern = hole_picked | bp;
                            let r = flush_5[pattern as usize];
                            // flush_5 stores 0 for non-straight-flush patterns
                            // that aren't straights — but every popcount=5
                            // pattern produces a valid flush rank (high card
                            // through royal straight flush), so r > 0.
                            if r > 0 && r < best {
                                best = r;
                            }
                        }
                    }
                    if best == u16::MAX {
                        continue; // shouldn't happen given the loops above
                    }

                    let board_padded = board_bits | PADDING[5 - scb];
                    let hole_padded = hole_bits | PADDING[4 - sch];
                    let board_hash = hash_binary(board_padded, 5, choose) as usize;
                    let hole_hash = hash_binary(hole_padded, 4, choose) as usize;
                    out[board_hash * FLUSH_PLO4_HOLE_HASHES + hole_hash] = best;
                }
            }
        }
    }

    out
}

/// Iterator over all 13-bit values with exactly `k` bits set.
fn subsets_of_n(n: usize, k: usize) -> impl Iterator<Item = u32> {
    let total = 1u32 << n;
    (0..total).filter(move |&v| v.count_ones() as usize == k)
}

/// All `k`-combinations of a slice, as `Vec<&T>`.
fn combinations_of<T: Copy>(xs: &[T], k: usize) -> Vec<Vec<T>> {
    let mut out = Vec::new();
    let mut buf = Vec::with_capacity(k);
    fn rec<T: Copy>(xs: &[T], k: usize, start: usize, buf: &mut Vec<T>, out: &mut Vec<Vec<T>>) {
        if buf.len() == k {
            out.push(buf.clone());
            return;
        }
        for i in start..xs.len() {
            buf.push(xs[i]);
            rec(xs, k, i + 1, buf, out);
            buf.pop();
        }
    }
    rec(xs, k, 0, &mut buf, &mut out);
    out
}
