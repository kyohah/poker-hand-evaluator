//! Tests for the rule wrappers `EightLowQualifiedRule` and `AceFiveLowRule`.
//!
//! Pinning the strength contract: with `Strength = Option<Reverse<u16>>`,
//! qualifying hands must compare strictly greater than non-qualifying
//! hands, and within qualifying hands smaller raw rank is stronger.

use phe_eight_low::{AceFiveLowRule, EightLowQualifiedRule, Hand};
use std::cmp::Reverse;

fn h(s: &str) -> Hand {
    s.parse::<Hand>().unwrap()
}

#[test]
fn qualified_returns_none_for_nine_low() {
    assert_eq!(EightLowQualifiedRule::evaluate(&h("Ac2d3h4s9cTdKh")), None);
}

#[test]
fn qualified_returns_some_for_eight_low() {
    let s = EightLowQualifiedRule::evaluate(&h("Ac2d3h4s8cTdKh"));
    assert!(s.is_some());
}

#[test]
fn qualified_wheel_strictly_better_than_eight_low() {
    let wheel = EightLowQualifiedRule::evaluate(&h("Ac2d3h4s5cTdKh")).unwrap();
    let eight = EightLowQualifiedRule::evaluate(&h("Ac2d3h4s8cTdKh")).unwrap();
    assert!(wheel > eight);
}

#[test]
fn qualified_any_qualifier_beats_any_non_qualifier() {
    let q = EightLowQualifiedRule::evaluate(&h("Ac2d3h4s8cTdKh"));
    let nq = EightLowQualifiedRule::evaluate(&h("Ac2d3h4s9cTdKh"));
    assert!(q > nq, "qualifier {:?} should beat non-qualifier {:?}", q, nq);
}

#[test]
fn ace_five_returns_rank_unconditionally() {
    // 9-low no-pair vs. four-of-a-kind: with seven cards [A,A,A,A,K,Q,J]
    // every 5-card selection must include at least three aces, so the
    // hand can never escape FourOfAKind — strictly worse than any no-pair.
    let nine_low = AceFiveLowRule::evaluate(&h("Ac2d3h4s9cTdKh"));
    let quads = AceFiveLowRule::evaluate(&h("AcAdAhAsKcQdJh"));
    assert!(nine_low > quads, "nine-low {:?} should beat quads {:?}", nine_low, quads);
}

#[test]
fn ace_five_wheel_is_nuts() {
    let wheel = AceFiveLowRule::evaluate(&h("Ac2d3h4s5cTdKh"));
    assert_eq!(wheel, Reverse(0));
}
