//! Variant-agnostic core for the poker hand evaluator.
//!
//! Holds the perfect-hash constants, the 52-card encoding, the universal
//! offset table, the [`Hand`] struct, and the lookup-driven evaluation
//! primitive [`evaluate_via_lookup`]. Variant crates (`phe-holdem`,
//! `phe-deuce-seven`, ...) ship the rule-specific lookup tables and a
//! thin wrapper around the shared core.

pub mod constants;
mod hand;
pub mod offsets;

pub use constants::{
    CARDS, FLUSH_MASK, MAX_RANK_KEY, NUMBER_OF_CARDS, NUMBER_OF_RANKS, OFFSET_SHIFT, RANK_BASES,
    SUIT_BASES, SUIT_SHIFT,
};
pub use hand::{evaluate_via_lookup, Hand};
pub use offsets::OFFSETS;
