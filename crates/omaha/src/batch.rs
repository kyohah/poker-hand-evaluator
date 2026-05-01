//! Batch PLO4 evaluation with software prefetch.
//!
//! The single-hand `evaluate_plo4_cards` is bound by NOFLUSH_PLO4
//! lookup latency (table is 22 MB, larger than typical L3 cache, so a
//! cold lookup costs ~25-30 ns of DRAM round-trip). When the caller
//! has many hands to evaluate, a 2-pass design lets us hide that
//! latency:
//!
//! 1. **Pass 1** computes the noflush index for every hand. This is
//!    ~38 ns of pure CPU work per hand and never touches the big
//!    table. Uses early-exit `hash_quinary` (the branchless variant
//!    and a hand-written AVX2 8-wide gather were both tried and lost
//!    to it — see BENCH_NOTES.md negative results).
//! 2. **Pass 2** loops through the precomputed indices and runs the
//!    full eval. Before each lookup we issue a `_mm_prefetch` for
//!    `i + PF_AHEAD` so by the time control reaches that iteration
//!    the table line is already on its way to L1.
//!
//! Empirically `PF_AHEAD = 8` works well on x86_64 with DDR4 — large
//! enough to hide ~80 ns of latency while small enough to fit in the
//! reorder buffer.

use crate::dp::BIT_OF_DIV_4;
use crate::flush_5card::FLUSH;
use crate::hash::{hash_binary, hash_quinary};
use phe_omaha_assets::{FLUSH_PLO4, NOFLUSH_PLO4};

#[cfg(target_arch = "x86_64")]
use core::arch::x86_64::{_mm_prefetch, _MM_HINT_T0};

const PADDING: [i32; 3] = [0x0000, 0x2000, 0x6000];
#[cfg(target_arch = "x86_64")]
const PF_AHEAD: usize = 8;

/// Computes the NOFLUSH_PLO4 index for a single hand. Pure CPU work,
/// no memory accesses to the big table.
#[inline(always)]
fn noflush_index_scalar(
    c1: i32,
    c2: i32,
    c3: i32,
    c4: i32,
    c5: i32,
    h1: i32,
    h2: i32,
    h3: i32,
    h4: i32,
) -> usize {
    let mut quinary_board = [0u8; 13];
    let mut quinary_hole = [0u8; 13];
    unsafe {
        *quinary_board.get_unchecked_mut((c1 >> 2) as usize) += 1;
        *quinary_board.get_unchecked_mut((c2 >> 2) as usize) += 1;
        *quinary_board.get_unchecked_mut((c3 >> 2) as usize) += 1;
        *quinary_board.get_unchecked_mut((c4 >> 2) as usize) += 1;
        *quinary_board.get_unchecked_mut((c5 >> 2) as usize) += 1;
        *quinary_hole.get_unchecked_mut((h1 >> 2) as usize) += 1;
        *quinary_hole.get_unchecked_mut((h2 >> 2) as usize) += 1;
        *quinary_hole.get_unchecked_mut((h3 >> 2) as usize) += 1;
        *quinary_hole.get_unchecked_mut((h4 >> 2) as usize) += 1;
    }
    let board_hash = hash_quinary(&quinary_board, 5);
    let hole_hash = hash_quinary(&quinary_hole, 4);
    (board_hash * 1820 + hole_hash) as usize
}

/// Same flush-and-min logic as `evaluate_plo4_cards`, but takes the
/// pre-computed `noflush_idx` so we can amortise that hash work
/// across the 2-pass batch loop.
#[inline(always)]
fn evaluate_with_noflush_idx(
    c1: i32,
    c2: i32,
    c3: i32,
    c4: i32,
    c5: i32,
    h1: i32,
    h2: i32,
    h3: i32,
    h4: i32,
    noflush_idx: usize,
) -> i32 {
    let mut value_flush: i32 = 10000;

    let mut suit_count_board = [0i32; 4];
    let mut suit_count_hole = [0i32; 4];
    unsafe {
        *suit_count_board.get_unchecked_mut((c1 & 0x3) as usize) += 1;
        *suit_count_board.get_unchecked_mut((c2 & 0x3) as usize) += 1;
        *suit_count_board.get_unchecked_mut((c3 & 0x3) as usize) += 1;
        *suit_count_board.get_unchecked_mut((c4 & 0x3) as usize) += 1;
        *suit_count_board.get_unchecked_mut((c5 & 0x3) as usize) += 1;
        *suit_count_hole.get_unchecked_mut((h1 & 0x3) as usize) += 1;
        *suit_count_hole.get_unchecked_mut((h2 & 0x3) as usize) += 1;
        *suit_count_hole.get_unchecked_mut((h3 & 0x3) as usize) += 1;
        *suit_count_hole.get_unchecked_mut((h4 & 0x3) as usize) += 1;
    }

    for i in 0..4 {
        let scb = unsafe { *suit_count_board.get_unchecked(i) };
        let sch = unsafe { *suit_count_hole.get_unchecked(i) };
        if scb >= 3 && sch >= 2 {
            let mut suit_binary_board = [0i32; 4];
            let mut suit_binary_hole = [0i32; 4];
            unsafe {
                *suit_binary_board.get_unchecked_mut((c1 & 0x3) as usize) |=
                    *BIT_OF_DIV_4.get_unchecked(c1 as usize) as i32;
                *suit_binary_board.get_unchecked_mut((c2 & 0x3) as usize) |=
                    *BIT_OF_DIV_4.get_unchecked(c2 as usize) as i32;
                *suit_binary_board.get_unchecked_mut((c3 & 0x3) as usize) |=
                    *BIT_OF_DIV_4.get_unchecked(c3 as usize) as i32;
                *suit_binary_board.get_unchecked_mut((c4 & 0x3) as usize) |=
                    *BIT_OF_DIV_4.get_unchecked(c4 as usize) as i32;
                *suit_binary_board.get_unchecked_mut((c5 & 0x3) as usize) |=
                    *BIT_OF_DIV_4.get_unchecked(c5 as usize) as i32;
                *suit_binary_hole.get_unchecked_mut((h1 & 0x3) as usize) |=
                    *BIT_OF_DIV_4.get_unchecked(h1 as usize) as i32;
                *suit_binary_hole.get_unchecked_mut((h2 & 0x3) as usize) |=
                    *BIT_OF_DIV_4.get_unchecked(h2 as usize) as i32;
                *suit_binary_hole.get_unchecked_mut((h3 & 0x3) as usize) |=
                    *BIT_OF_DIV_4.get_unchecked(h3 as usize) as i32;
                *suit_binary_hole.get_unchecked_mut((h4 & 0x3) as usize) |=
                    *BIT_OF_DIV_4.get_unchecked(h4 as usize) as i32;
            }
            let sbb = unsafe { *suit_binary_board.get_unchecked(i) };
            let sbh = unsafe { *suit_binary_hole.get_unchecked(i) };
            if scb == 3 && sch == 2 {
                value_flush = unsafe { *FLUSH.get_unchecked((sbb | sbh) as usize) } as i32;
            } else {
                let board_padded = sbb | unsafe { *PADDING.get_unchecked((5 - scb) as usize) };
                let hole_padded = sbh | unsafe { *PADDING.get_unchecked((4 - sch) as usize) };
                let board_hash = hash_binary(board_padded, 5);
                let hole_hash = hash_binary(hole_padded, 4);
                value_flush =
                    unsafe { *FLUSH_PLO4.get_unchecked((board_hash * 1365 + hole_hash) as usize) }
                        as i32;
            }
            break;
        }
    }

    let value_noflush = unsafe { *NOFLUSH_PLO4.get_unchecked(noflush_idx) } as i32;
    value_flush.min(value_noflush)
}

/// Evaluates a batch of PLO4 hands, writing ranks into `out`.
///
/// `hands` and `out` must have the same length. Each entry of `hands`
/// is `(hole, board)` with hole = 4 cards and board = 5 cards.
///
/// Internally:
/// 1. Pass 1 — compute the NOFLUSH_PLO4 index for every hand (no big
///    memory accesses, ~10 ns / hand CPU).
/// 2. Pass 2 — for each hand, prefetch `noflush[i + PF_AHEAD]` and
///    evaluate hand `i`. Memory latency on the large table is hidden
///    behind the in-flight prefetches.
pub fn evaluate_plo4_batch(hands: &[([u8; 4], [u8; 5])], out: &mut [i32]) {
    assert_eq!(hands.len(), out.len(), "hands / out length mismatch");
    let n = hands.len();

    // Pass 1: precompute noflush indices. Pure CPU work, no big-table
    // accesses, ~38 ns/hand. Early-exit `hash_quinary` (the path used
    // by `noflush_index_scalar`) beat both the branchless variant and
    // a hand-written AVX2 8-wide gather (see BENCH_NOTES.md, "Negative
    // results").
    let mut indices: Vec<usize> = Vec::with_capacity(n);
    for (hole, board) in hands {
        indices.push(noflush_index_scalar(
            board[0] as i32,
            board[1] as i32,
            board[2] as i32,
            board[3] as i32,
            board[4] as i32,
            hole[0] as i32,
            hole[1] as i32,
            hole[2] as i32,
            hole[3] as i32,
        ));
    }

    // Warmup prefetch for the first PF_AHEAD entries.
    #[cfg(target_arch = "x86_64")]
    unsafe {
        let table = NOFLUSH_PLO4.as_ptr();
        for &idx in indices.iter().take(PF_AHEAD.min(n)) {
            _mm_prefetch::<_MM_HINT_T0>(table.add(idx) as *const i8);
        }
    }

    // Pass 2: full eval, prefetching i + PF_AHEAD ahead of each i.
    for i in 0..n {
        #[cfg(target_arch = "x86_64")]
        unsafe {
            let j = i + PF_AHEAD;
            if j < n {
                _mm_prefetch::<_MM_HINT_T0>(NOFLUSH_PLO4.as_ptr().add(indices[j]) as *const i8);
            }
        }
        let (hole, board) = &hands[i];
        out[i] = evaluate_with_noflush_idx(
            board[0] as i32,
            board[1] as i32,
            board[2] as i32,
            board[3] as i32,
            board[4] as i32,
            hole[0] as i32,
            hole[1] as i32,
            hole[2] as i32,
            hole[3] as i32,
            indices[i],
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eval::evaluate_plo4_cards;

    fn deal_hand(seed: u64, n: usize) -> Vec<([u8; 4], [u8; 5])> {
        let mut s = seed;
        let mut out = Vec::with_capacity(n);
        for _ in 0..n {
            let mut deck = [0u8; 52];
            for (i, slot) in deck.iter_mut().enumerate() {
                *slot = i as u8;
            }
            for i in 0..9 {
                s ^= s << 13;
                s ^= s >> 7;
                s ^= s << 17;
                let p = i + (s as usize) % (52 - i);
                deck.swap(i, p);
            }
            out.push((
                [deck[0], deck[1], deck[2], deck[3]],
                [deck[4], deck[5], deck[6], deck[7], deck[8]],
            ));
        }
        out
    }

    /// Batch must produce identical output to the single-hand path.
    #[test]
    fn batch_matches_single() {
        let hands = deal_hand(0xCAFEBABE_DEADBEEF, 1000);
        let mut single_out = vec![0i32; hands.len()];
        let mut batch_out = vec![0i32; hands.len()];

        for (i, (hole, board)) in hands.iter().enumerate() {
            single_out[i] = evaluate_plo4_cards(
                board[0] as i32,
                board[1] as i32,
                board[2] as i32,
                board[3] as i32,
                board[4] as i32,
                hole[0] as i32,
                hole[1] as i32,
                hole[2] as i32,
                hole[3] as i32,
            );
        }
        evaluate_plo4_batch(&hands, &mut batch_out);

        let mut mismatches = 0;
        for i in 0..hands.len() {
            if single_out[i] != batch_out[i] {
                mismatches += 1;
                if mismatches < 5 {
                    eprintln!(
                        "mismatch at {i}: single={} batch={} hands={:?}",
                        single_out[i], batch_out[i], hands[i]
                    );
                }
            }
        }
        assert_eq!(mismatches, 0, "{} batch / single mismatches", mismatches);
    }
}
