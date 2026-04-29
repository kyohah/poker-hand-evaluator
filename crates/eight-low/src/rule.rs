use crate::hand::{qualifies_8_or_better, Hand};
use std::cmp::Reverse;

/// 8-or-better qualifying low rule.
///
/// `Strength` is `Option<Reverse<u16>>`: `None` if the hand fails to
/// qualify, `Some(Reverse(rank))` otherwise. With `Option`'s `Ord`
/// (`None` < `Some`), `Reverse` (smaller rank = better) yields the
/// expected ordering: any qualifying hand beats any non-qualifying
/// hand, and within qualifying hands the smaller rank wins.
pub struct EightLowQualifiedRule;

impl EightLowQualifiedRule {
    /// Returns the strength of `hand`, or `None` if it does not qualify
    /// for 8-or-better.
    #[inline]
    pub fn evaluate(hand: &Hand) -> Option<Reverse<u16>> {
        let r = hand.evaluate();
        qualifies_8_or_better(r).then_some(Reverse(r))
    }
}

/// A-5 lowball rule (Razz; no qualifier).
///
/// Same lookup as [`EightLowQualifiedRule`], but every hand returns a
/// rank — straights and flushes do not exist in A-5 lowball; pairs
/// matter and the wheel `A-2-3-4-5` is the nuts.
pub struct AceFiveLowRule;

impl AceFiveLowRule {
    #[inline]
    pub fn evaluate(hand: &Hand) -> Reverse<u16> {
        Reverse(hand.evaluate())
    }
}
