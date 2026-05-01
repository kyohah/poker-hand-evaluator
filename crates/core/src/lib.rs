//! Variant-agnostic core for the poker hand evaluator.
//!
//! Holds the perfect-hash constants, the 52-card encoding, the universal
//! offset table, the [`Hand`] struct, and the lookup-driven evaluation
//! primitive [`evaluate_via_lookup`]. Variant crates (`phe-holdem`,
//! `phe-deuce-seven`, ...) ship the rule-specific lookup tables and a
//! thin wrapper around the shared core.

#![warn(missing_docs)]

/// Card-encoding constants and per-card perfect-hash bases.
pub mod constants;
mod hand;
/// First-fit-decreasing displacement table that flattens the
/// perfect-hash key space into a dense `LOOKUP[..]` index.
pub mod offsets;

pub use constants::{
    CARDS, FLUSH_MASK, MAX_RANK_KEY, NUMBER_OF_CARDS, NUMBER_OF_RANKS, OFFSET_SHIFT, RANK_BASES,
    SUIT_BASES, SUIT_SHIFT,
};
pub use hand::{evaluate_via_lookup, Hand};
pub use offsets::OFFSETS;
