//! Omaha high evaluator.
//!
//! In Omaha each player has 4 hole cards and 5 community-board cards,
//! and **must** use exactly 2 hole cards and 3 board cards. The best
//! 5-card hand is the maximum-Hold'em-rank choice over
//! `C(4,2) * C(5,3) = 6 * 10 = 60` candidate combinations.
//!
//! ## Optimizations
//!
//! 1. **Board-partial caching.** The 10 board-triple Hands are built
//!    once per call instead of being re-added inside every hole-pair
//!    iteration. Saves ~120 [`Hand::add_card`] calls per evaluation.
//!
//! 2. **Flush-impossible fast path.** Suit counts on hole + board tell
//!    us up front whether any flush can land (need ≥2 hole + ≥3 board
//!    of one suit). When no flush is possible, the inner loop uses a
//!    specialised rank-only lookup that skips the [`phe_core`] flush
//!    dispatch entirely. The structural insight from the user — boards
//!    like `4h3h5h8h9h` with 5 distinct ranks make Quads/FullHouse
//!    mathematically impossible — does not turn into a lookup-side
//!    optimisation, since the [`phe_holdem::HighRule`] LOOKUP returns
//!    the rank in O(1) regardless of which categories are reachable.

use phe_core::{Hand, OFFSETS, OFFSET_SHIFT};
use phe_holdem::assets::LOOKUP;
use phe_holdem::HighRule;

/// Omaha high rule.
///
/// `Strength = u16` (higher = stronger), reusing the Hold'em packing
/// scheme: bits 12-15 hold the [`phe_holdem::HandCategory`], bits 0-11
/// the within-category index.
pub struct OmahaHighRule;

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

/// Returns whether some 5-card combo (2 hole + 3 board) can be a
/// flush. Equivalent to: `∃ suit s. #hole_cards_in_s ≥ 2 ∧
/// #board_cards_in_s ≥ 3`.
#[inline]
pub fn flush_possible(hole: &[usize; 4], board: &[usize; 5]) -> bool {
    let mut hole_s = [0u8; 4];
    let mut board_s = [0u8; 4];
    for &c in hole {
        hole_s[c & 3] += 1;
    }
    for &c in board {
        board_s[c & 3] += 1;
    }
    (0..4).any(|s| hole_s[s] >= 2 && board_s[s] >= 3)
}

/// Builds the 10 partial Hands for each board-triple selection.
#[inline]
fn build_board_partials(board: &[usize; 5]) -> [Hand; 10] {
    std::array::from_fn(|idx| {
        let (a, b, c) = BOARD_TRIPLES[idx];
        Hand::new()
            .add_card(board[a])
            .add_card(board[b])
            .add_card(board[c])
    })
}

/// Rank-only lookup. Skips the flush-mask check and `LOOKUP_FLUSH`
/// access, so callers must guarantee the hand has no 5+ cards of one
/// suit. Behaviour is undefined otherwise.
#[inline]
fn evaluate_rank_only(hand: &Hand) -> u16 {
    let rank_key = hand.get_key() as u32 as usize;
    unsafe {
        let offset = *OFFSETS.get_unchecked(rank_key >> OFFSET_SHIFT) as usize;
        *LOOKUP.get_unchecked(rank_key.wrapping_add(offset))
    }
}

/// Inner loop: for every (hole pair × board triple), evaluate via
/// `eval` and return the maximum. Generic over the per-hand evaluator
/// so the call site can pick the rank-only or full path without
/// duplicating the loop.
#[inline]
fn evaluate_inner<F>(hole: &[usize; 4], board_partials: &[Hand; 10], eval: F) -> u16
where
    F: Fn(&Hand) -> u16,
{
    let mut best: u16 = 0;
    for &(i, j) in &HOLE_PAIRS {
        let hp = Hand::new().add_card(hole[i]).add_card(hole[j]);
        for bp in board_partials {
            let h = hp + *bp;
            let r = eval(&h);
            if r > best {
                best = r;
            }
        }
    }
    best
}

impl OmahaHighRule {
    /// Evaluates an Omaha high hand from `hole_cards` (length 4) and
    /// `board` (length 5).
    ///
    /// # Panics
    /// Panics if `hole_cards.len() != 4` or `board.len() != 5`.
    #[inline]
    pub fn evaluate(hole_cards: &[usize; 4], board: &[usize; 5]) -> u16 {
        let board_partials = build_board_partials(board);
        if flush_possible(hole_cards, board) {
            evaluate_inner(hole_cards, &board_partials, HighRule::evaluate)
        } else {
            evaluate_inner(hole_cards, &board_partials, evaluate_rank_only)
        }
    }
}
