//! Omaha high evaluator.
//!
//! In Omaha each player has 4 hole cards and 5 community-board cards,
//! and **must** use exactly 2 hole cards and 3 board cards. The best
//! 5-card hand is the maximum-Hold'em-rank choice over
//! `C(4,2) * C(5,3) = 6 * 10 = 60` candidate combinations.
//!
//! This crate is purely a wrapper around `phe-holdem::HighRule` — it
//! enumerates the 60 combinations and takes the max.

use phe_core::Hand;
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

impl OmahaHighRule {
    /// Evaluates an Omaha high hand from `hole_cards` (length 4) and
    /// `board` (length 5).
    ///
    /// # Panics
    /// Panics if `hole_cards.len() != 4` or `board.len() != 5`.
    pub fn evaluate(hole_cards: &[usize; 4], board: &[usize; 5]) -> u16 {
        let mut best: u16 = 0;
        for &(i, j) in &HOLE_PAIRS {
            let hole_pair = Hand::new()
                .add_card(hole_cards[i])
                .add_card(hole_cards[j]);
            for &(a, b, c) in &BOARD_TRIPLES {
                let h = hole_pair
                    .add_card(board[a])
                    .add_card(board[b])
                    .add_card(board[c]);
                let r = HighRule::evaluate(&h);
                if r > best {
                    best = r;
                }
            }
        }
        best
    }
}
