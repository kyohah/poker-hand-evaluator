//! Path 2: flush-dominates 9-card direct evaluator.
//!
//! Invoked when (a) at least one suit has both ≥2 hole and ≥3 board
//! cards (so a 5-card flush combo is reachable), AND (b) the board
//! has 5 distinct ranks (so Full House and Quads are unreachable for
//! every combo). Under these preconditions the best 5-card hand is
//! the strongest reachable Flush or Straight Flush — every other
//! category (HighCard..Straight) loses to any flush.
//!
//! Algorithm (one `LOOKUP_FLUSH` access total):
//!
//!   1. Build per-suit rank bitmasks `hole_mask` / `board_mask`
//!      restricted to the flush suit.
//!   2. Scan the 10 straight-flush 5-rank windows (descending
//!      strength: royal → 6-high → wheel). The first `W` for which
//!      `popcount(hole_mask & W) >= 2` AND
//!      `popcount(board_mask & W) >= 3` is the strongest reachable
//!      SF; return `LOOKUP_FLUSH[W]`.
//!   3. If no SF window matched, the strongest plain flush is
//!      `top_2_bits(hole_mask) | top_3_bits(board_mask)`. Return
//!      `LOOKUP_FLUSH[…]`.
//!
//! Why the SF predicate is exact: in any single suit, hole and board
//! contribute disjoint rank sets. With both popcounts at least 2 and
//! 3 over a 5-bit window, their disjoint union is bounded above by 5
//! → equals 5 → fully covers the window, so an Omaha 2+3 SF combo
//! genuinely exists.

use phe_holdem::assets::LOOKUP_FLUSH;

/// 10 straight-flush 5-rank windows in descending rank order. Index
/// 0 is T-J-Q-K-A (royal SF, highest); index 8 is 2-3-4-5-6 (lowest
/// "standard" SF); index 9 is the wheel A-2-3-4-5 (lowest SF
/// overall, still beats any plain flush per `LOOKUP_FLUSH`).
const SF_WINDOWS_DESC: [u16; 10] = [
    0b1_1111_0000_0000, // T-J-Q-K-A (royal)
    0b0_1111_1000_0000, // 9-T-J-Q-K
    0b0_0111_1100_0000, // 8-9-T-J-Q
    0b0_0011_1110_0000, // 7-8-9-T-J
    0b0_0001_1111_0000, // 6-7-8-9-T
    0b0_0000_1111_1000, // 5-6-7-8-9
    0b0_0000_0111_1100, // 4-5-6-7-8
    0b0_0000_0011_1110, // 3-4-5-6-7
    0b0_0000_0001_1111, // 2-3-4-5-6
    0b1_0000_0000_1111, // A-2-3-4-5 (wheel)
];

/// Returns `mask` with all but its top-2 set bits cleared.
/// Caller must ensure `mask.count_ones() >= 2`.
#[inline]
fn top_2_bits(mask: u16) -> u16 {
    debug_assert!(mask.count_ones() >= 2);
    let h1 = 1u16 << (15 - mask.leading_zeros());
    let m2 = mask ^ h1;
    let h2 = 1u16 << (15 - m2.leading_zeros());
    h1 | h2
}

/// Returns `mask` with all but its top-3 set bits cleared.
/// Caller must ensure `mask.count_ones() >= 3`.
#[inline]
fn top_3_bits(mask: u16) -> u16 {
    debug_assert!(mask.count_ones() >= 3);
    let h1 = 1u16 << (15 - mask.leading_zeros());
    let m2 = mask ^ h1;
    let h2 = 1u16 << (15 - m2.leading_zeros());
    let m3 = m2 ^ h2;
    let h3 = 1u16 << (15 - m3.leading_zeros());
    h1 | h2 | h3
}

/// Path-2 entry point.
#[inline]
pub(crate) fn evaluate(hole: &[usize; 4], board: &[usize; 5], suit: u8) -> u16 {
    let suit_u = suit as usize;

    let mut hole_mask: u16 = 0;
    for &c in hole {
        if c & 3 == suit_u {
            hole_mask |= 1u16 << (c / 4);
        }
    }
    let mut board_mask: u16 = 0;
    for &c in board {
        if c & 3 == suit_u {
            board_mask |= 1u16 << (c / 4);
        }
    }
    debug_assert!(hole_mask.count_ones() >= 2 && board_mask.count_ones() >= 3);

    for &window in &SF_WINDOWS_DESC {
        if (hole_mask & window).count_ones() >= 2
            && (board_mask & window).count_ones() >= 3
        {
            return unsafe { *LOOKUP_FLUSH.get_unchecked(window as usize) };
        }
    }

    let flush_key = top_2_bits(hole_mask) | top_3_bits(board_mask);
    unsafe { *LOOKUP_FLUSH.get_unchecked(flush_key as usize) }
}
