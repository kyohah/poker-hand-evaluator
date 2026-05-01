//! Omaha PLO4 high evaluator — port of HenryRLee/PokerHandEvaluator.
//!
//! Phase 0a: bit-exact port. No Rust-side optimisations yet.

#![warn(missing_docs)]
// The 9-card and 10-card argument lists in `evaluate_plo4_cards` /
// `noflush_index_scalar` / `evaluate_with_noflush_idx` mirror
// HenryRLee's C signatures exactly so the algorithm is easy to
// diff against the reference. Bundling into a struct hides that
// correspondence.
#![allow(clippy::too_many_arguments)]

pub mod batch;
pub mod dp;
pub mod eval;
pub mod flush_5card;
pub mod hash;

#[cfg(feature = "cuda")]
pub mod cuda;

pub use dp::DP;

pub use batch::{evaluate_plo4_batch, evaluate_plo4_batch_into};
pub use eval::{evaluate_plo4_cards, evaluate_plo4_cards_arr};

/// Omaha 4-hole high rule.
///
/// Wraps [`evaluate_plo4_cards`] with the workspace-standard
/// `(hole, board)` signature, converting the Cactus-Kev rank
/// (`1..=7462`, lower = stronger) to the workspace's flat
/// `u16` strength (`1..=7462`, **higher = stronger**) so it
/// composes cleanly with the `HandRule` trait.
///
/// Note: this loses the bit-packed `(category, within-category-index)`
/// layout that other workspace evaluators use. Callers that need to
/// know the hand category (Pair / Two Pair / Straight / …) can
/// recover it from the Cactus-Kev rank ranges; for ordinary
/// strength comparisons (which is what solvers / showdowns need)
/// the flat value works directly.
#[derive(Default, Clone, Copy, Debug)]
pub struct OmahaHighRule;

impl OmahaHighRule {
    /// `hole` must be 4 distinct card ids in `[0, 51]`, `board` must
    /// be 5. No validation in release builds.
    #[inline]
    pub fn evaluate(hole: &[usize; 4], board: &[usize; 5]) -> u16 {
        let cactus_rank = evaluate_plo4_cards(
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
        // Invert to "higher = stronger" while staying in [1, 7462].
        (7463 - cactus_rank) as u16
    }
}
