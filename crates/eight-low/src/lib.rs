//! 8-or-better low and A-5 lowball (Razz) hand evaluators.
//!
//! Absorbed from `kyohah/8low-evaluator`. The encoding differs from the
//! Hold'em-shape `phe-core::Hand` (Ace is rank 0, suits are not part of
//! the perfect-hash key), so this crate keeps its own [`Hand`] struct.

#![warn(missing_docs)]

mod hand;
mod rule;

pub use hand::{
    get_low_category, qualifies_8_or_better, Hand, LowHandCategory, EIGHT_OR_BETTER_THRESHOLD,
    LOW_CATEGORY_COUNTS, TOTAL_LOW_RANKS,
};
pub use rule::{AceFiveLowRule, EightLowQualifiedRule};
