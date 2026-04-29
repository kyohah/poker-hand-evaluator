//! 2-7 lowball evaluator (single-/triple-draw; 5 cards only).
//!
//! Built on `phe-core::Hand` (Hold'em-shape encoding) plus a 2-7-specific
//! lookup table with `A-2-3-4-5` reclassified as no-pair instead of a
//! straight. The strength wrapper applies `Reverse` so the trait
//! contract — higher = stronger — holds while internally we use the
//! same packed-rank scheme as `phe-holdem` (lower packed = stronger as
//! 2-7 hand).
//!
//! ## Hand-size limit
//!
//! The lookup table covers **only 5-card hands**. A 6/7-card 2-7
//! evaluation would need to consider non-flush sub-hands when 5+ cards
//! share a suit (avoiding flush), which the holdem-shape lookup cannot
//! disambiguate. 2-7 is realistically played as a draw game where each
//! player's hand is exactly 5 cards, so this is not a practical
//! limitation. See `phe-scripts gen-deuce-seven-lookup` for details.

use phe_core::{evaluate_via_lookup, Hand};
use phe_deuce_seven_assets::{LOOKUP, LOOKUP_FLUSH};
use std::cmp::Reverse;

pub use phe_holdem::{get_hand_category, parse_hand, HandCategory};

/// 2-7 lowball rule.
///
/// `Strength = Reverse<u16>`: smaller raw 16-bit rank = weaker as a
/// Hold'em high hand = stronger as 2-7. Wrapping with `Reverse` makes
/// the trait contract (higher = stronger) hold.
pub struct DeuceSevenLowRule;

impl DeuceSevenLowRule {
    /// Evaluates a 5-card 2-7 lowball hand.
    ///
    /// # Panics
    /// Panics if `hand.len() != 5`. The lookup table covers only the
    /// 5-card case; see crate docs.
    #[inline]
    pub fn evaluate(hand: &Hand) -> Reverse<u16> {
        assert_eq!(
            hand.len(),
            5,
            "DeuceSevenLowRule supports 5-card hands only"
        );
        Reverse(evaluate_via_lookup(hand, &LOOKUP, &LOOKUP_FLUSH))
    }
}
