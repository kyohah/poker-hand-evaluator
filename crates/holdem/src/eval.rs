use phe_core::{evaluate_via_lookup, Hand};
use phe_holdem_assets::{LOOKUP, LOOKUP_FLUSH};

/// Hold'em high evaluator.
///
/// Higher returned `u16` = stronger hand. Bits 12-15 encode the
/// [`crate::HandCategory`]; bits 0-11 encode the within-category index.
///
/// # Examples
///
/// ```
/// use phe_core::Hand;
/// use phe_holdem::{get_hand_category, HandCategory, HighRule};
///
/// // Build a 7-card hand: A♠ K♠ Q♠ J♠ T♠ 2♣ 2♦ — royal flush + dead pair.
/// let hand = Hand::from_slice(&[51, 47, 43, 39, 35, 0, 1]);
/// let strength = HighRule::evaluate(&hand);
/// assert_eq!(get_hand_category(strength), HandCategory::StraightFlush);
/// ```
#[derive(Default, Clone, Copy, Debug)]
pub struct HighRule;

impl HighRule {
    /// Evaluates a 5-, 6-, or 7-card hand.
    ///
    /// # Safety
    /// Behavior is undefined when `hand.len()` is outside `5..=7`.
    #[inline]
    pub fn evaluate(hand: &Hand) -> u16 {
        evaluate_via_lookup(hand, &LOOKUP, &LOOKUP_FLUSH)
    }
}
