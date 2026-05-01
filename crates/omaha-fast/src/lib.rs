//! Omaha PLO4 high evaluator — port of HenryRLee/PokerHandEvaluator.
//!
//! Phase 0a: bit-exact port. No Rust-side optimisations yet.

pub mod batch;
pub mod dp;
pub mod flush_5card;
pub mod hash;
pub mod eval;

pub use dp::DP;

pub use batch::evaluate_plo4_batch;
pub use eval::evaluate_plo4_cards;

/// Omaha 4-hole high rule.
///
/// Wraps [`evaluate_plo4_cards`] with the workspace-standard
/// `(hole, board)` signature. Returns Cactus-Kev rank in `[1, 7462]`,
/// **lower = stronger**. The facade flips this to higher-better u16
/// when implementing `HandRule`.
pub struct OmahaHighFastRule;

impl OmahaHighFastRule {
    /// `hole` must be 4 distinct card ids in `[0, 51]`, `board` must
    /// be 5. No validation in release builds.
    #[inline]
    pub fn evaluate(hole: &[usize; 4], board: &[usize; 5]) -> i32 {
        evaluate_plo4_cards(
            board[0] as i32, board[1] as i32, board[2] as i32,
            board[3] as i32, board[4] as i32,
            hole[0] as i32, hole[1] as i32, hole[2] as i32, hole[3] as i32,
        )
    }
}
