//! Path 1: no-flush 9-card direct table lookup.
//!
//! Invoked when no suit has both ≥2 hole and ≥3 board cards, i.e.,
//! flush is unreachable across all 60 combos. The 5-card answer
//! depends only on the rank multisets of hole and board, so the
//! 60-combo enumeration collapses into a single
//! `phe_omaha_assets::noflush_lookup` access.
//!
//! Indexing: hole and board are encoded into dense multiset indices
//! via the **multiset combinatorial number system**:
//!
//! ```text
//! sort the k ranks ascending: r_0 ≤ r_1 ≤ … ≤ r_{k-1}
//! shift to strict-increasing: y_i = r_i + i
//! index = sum_{i=0..k} C(y_i, i+1)
//! ```
//!
//! With `k = 4` ranks from 13, the hole index lives in
//! `[0, C(16, 4)) = [0, 1820)`; with `k = 5` ranks from 13, the
//! board index lives in `[0, C(17, 5)) = [0, 6188)`. Sorting is
//! done via 5- and 9-comparator networks; the binomial sum uses a
//! pre-computed `BINOM` table with no runtime allocation.

use phe_omaha_assets::{noflush_lookup, NUM_BOARD};

/// Pre-computed binomial coefficients `BINOM[a][b] = C(a, b)`.
/// Covers all values needed by `hole_index` / `board_index`
/// (`a <= 16, b <= 5`).
const BINOM: [[u32; 6]; 17] = [
    [1, 0, 0, 0, 0, 0],
    [1, 1, 0, 0, 0, 0],
    [1, 2, 1, 0, 0, 0],
    [1, 3, 3, 1, 0, 0],
    [1, 4, 6, 4, 1, 0],
    [1, 5, 10, 10, 5, 1],
    [1, 6, 15, 20, 15, 6],
    [1, 7, 21, 35, 35, 21],
    [1, 8, 28, 56, 70, 56],
    [1, 9, 36, 84, 126, 126],
    [1, 10, 45, 120, 210, 252],
    [1, 11, 55, 165, 330, 462],
    [1, 12, 66, 220, 495, 792],
    [1, 13, 78, 286, 715, 1287],
    [1, 14, 91, 364, 1001, 2002],
    [1, 15, 105, 455, 1365, 3003],
    [1, 16, 120, 560, 1820, 4368],
];

/// In-place ascending sort of 4 elements via a 5-comparator
/// Bose-Nelson network. Branchless on x86_64 in release mode (each
/// comparator becomes a `cmov` pair).
#[inline]
fn sort4(a: &mut [u32; 4]) {
    if a[0] > a[1] { a.swap(0, 1); }
    if a[2] > a[3] { a.swap(2, 3); }
    if a[0] > a[2] { a.swap(0, 2); }
    if a[1] > a[3] { a.swap(1, 3); }
    if a[1] > a[2] { a.swap(1, 2); }
}

/// In-place ascending sort of 5 elements via a 9-comparator network.
#[inline]
fn sort5(a: &mut [u32; 5]) {
    if a[0] > a[1] { a.swap(0, 1); }
    if a[3] > a[4] { a.swap(3, 4); }
    if a[2] > a[4] { a.swap(2, 4); }
    if a[2] > a[3] { a.swap(2, 3); }
    if a[1] > a[4] { a.swap(1, 4); }
    if a[0] > a[3] { a.swap(0, 3); }
    if a[0] > a[2] { a.swap(0, 2); }
    if a[1] > a[3] { a.swap(1, 3); }
    if a[1] > a[2] { a.swap(1, 2); }
}

/// Multiset combinatorial-number-system encoder for the hole's 4
/// rank values. Returns dense index in `[0, 1820)`.
#[inline]
fn hole_index(hole: &[usize; 4]) -> usize {
    let mut r: [u32; 4] = [
        (hole[0] / 4) as u32,
        (hole[1] / 4) as u32,
        (hole[2] / 4) as u32,
        (hole[3] / 4) as u32,
    ];
    sort4(&mut r);
    let r = [r[0] as usize, r[1] as usize, r[2] as usize, r[3] as usize];
    (BINOM[r[0]][1]
        + BINOM[r[1] + 1][2]
        + BINOM[r[2] + 2][3]
        + BINOM[r[3] + 3][4]) as usize
}

/// Multiset combinatorial-number-system encoder for the board's 5
/// rank values. Returns dense index in `[0, 6188)`.
#[inline]
fn board_index(board: &[usize; 5]) -> usize {
    let mut r: [u32; 5] = [
        (board[0] / 4) as u32,
        (board[1] / 4) as u32,
        (board[2] / 4) as u32,
        (board[3] / 4) as u32,
        (board[4] / 4) as u32,
    ];
    sort5(&mut r);
    let r = [r[0] as usize, r[1] as usize, r[2] as usize, r[3] as usize, r[4] as usize];
    (BINOM[r[0]][1]
        + BINOM[r[1] + 1][2]
        + BINOM[r[2] + 2][3]
        + BINOM[r[3] + 3][4]
        + BINOM[r[4] + 4][5]) as usize
}

/// Path-1 entry point: one direct table access.
#[inline]
pub(crate) fn evaluate(hole: &[usize; 4], board: &[usize; 5]) -> u16 {
    let key = hole_index(hole) * NUM_BOARD + board_index(board);
    unsafe { *noflush_lookup().get_unchecked(key) }
}
