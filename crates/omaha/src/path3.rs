//! Path 3: flush eligible AND board has a pair.
//!
//! Both flush combos and non-flush combos may produce the maximum
//! (a flush, but also Full House or Four-of-a-Kind that the board
//! pair makes reachable). Each of the 60 (hole_pair, board_triple)
//! combos is dispatched per-combo — flush iff all 5 cards share the
//! flush suit, otherwise rank-only. Hole and board partials are
//! pre-summed once so the inner loop is one branch + one of two
//! lookups per combo.

use phe_core::{OFFSETS, OFFSET_SHIFT, RANK_BASES};
use phe_holdem::assets::{LOOKUP, LOOKUP_FLUSH};

use crate::{BOARD_TRIPLES, HOLE_PAIRS};

/// Rank-only lookup driven by a pre-summed `rank_key` (the lower 32
/// bits of `Hand::get_key()`). Skips Hand construction entirely.
/// Caller must guarantee `rank_key = sum of RANK_BASES[rank] for the
/// 5 cards`, none of those cards form a flush, and the sum fits in
/// `u32` (always true for ≤7 cards).
#[inline]
fn evaluate_rank_only_from_key(rank_key: u32) -> u16 {
    let rk = rank_key as usize;
    unsafe {
        let offset = *OFFSETS.get_unchecked(rk >> OFFSET_SHIFT) as usize;
        *LOOKUP.get_unchecked(rk.wrapping_add(offset))
    }
}

/// Path-3 entry point.
#[inline]
pub(crate) fn evaluate(hole: &[usize; 4], board: &[usize; 5], suit: u8) -> u16 {
    let suit_u = suit as usize;

    // Per-hole-card precomputations.
    let mut hole_rk = [0u32; 4];
    let mut hole_inc = [0u8; 4]; // 1 if in flush suit, else 0
    let mut hole_fb = [0u16; 4]; // (1 << rank) if in suit, else 0
    for i in 0..4 {
        let c = hole[i];
        let rank = c / 4;
        hole_rk[i] = RANK_BASES[rank] as u32;
        let in_s = (c & 3) == suit_u;
        hole_inc[i] = in_s as u8;
        hole_fb[i] = if in_s { 1u16 << rank } else { 0 };
    }
    let mut pair_rk = [0u32; 6];
    let mut pair_inc = [0u8; 6];
    let mut pair_fb = [0u16; 6];
    for (idx, &(i, j)) in HOLE_PAIRS.iter().enumerate() {
        pair_rk[idx] = hole_rk[i].wrapping_add(hole_rk[j]);
        pair_inc[idx] = hole_inc[i] + hole_inc[j];
        pair_fb[idx] = hole_fb[i] | hole_fb[j];
    }

    // Per-board-card precomputations.
    let mut board_rk = [0u32; 5];
    let mut board_inc = [0u8; 5];
    let mut board_fb = [0u16; 5];
    for i in 0..5 {
        let c = board[i];
        let rank = c / 4;
        board_rk[i] = RANK_BASES[rank] as u32;
        let in_s = (c & 3) == suit_u;
        board_inc[i] = in_s as u8;
        board_fb[i] = if in_s { 1u16 << rank } else { 0 };
    }
    let mut triple_rk = [0u32; 10];
    let mut triple_inc = [0u8; 10];
    let mut triple_fb = [0u16; 10];
    for (idx, &(a, b, c)) in BOARD_TRIPLES.iter().enumerate() {
        triple_rk[idx] = board_rk[a]
            .wrapping_add(board_rk[b])
            .wrapping_add(board_rk[c]);
        triple_inc[idx] = board_inc[a] + board_inc[b] + board_inc[c];
        triple_fb[idx] = board_fb[a] | board_fb[b] | board_fb[c];
    }

    let mut best: u16 = 0;
    for pi in 0..6usize {
        for ti in 0..10usize {
            // Combo is a 5-card flush iff all 5 cards share `suit`.
            let r = if pair_inc[pi] + triple_inc[ti] == 5 {
                let flush_key = pair_fb[pi] | triple_fb[ti];
                unsafe { *LOOKUP_FLUSH.get_unchecked(flush_key as usize) }
            } else {
                evaluate_rank_only_from_key(pair_rk[pi].wrapping_add(triple_rk[ti]))
            };
            if r > best {
                best = r;
            }
        }
    }
    best
}
