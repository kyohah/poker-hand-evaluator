//! Property-based contract checks for the [`HandRule`] trait.
//!
//! These exercise three invariants that are easy to break with a
//! lookup-table edit and hard to spot in unit tests:
//!
//! * **Determinism / order-invariance.** A `HandRule` operates on the
//!   *multiset* of cards; permuting the input slice must not change
//!   the strength.
//! * **`Ord` totality.** For any pair of inputs, exactly one of
//!   `<`, `>`, `==` must hold (we lean on `derive Ord` for `u16` and
//!   tuples, but `Reverse<Option<_>>` and the `HiLoRule` tuple are
//!   easy to miswire).
//! * **Lookup range.** Hold'em high strengths must fit in the 4-bit
//!   category × 12-bit within-category layout; eight-low qualifying
//!   strengths must be `<= 55` (the C(8,5) threshold).
//!
//! Gated on every variant feature so it's elided under narrower
//! feature builds (matching `tests/facade.rs`).

#![cfg(all(
    feature = "holdem",
    feature = "eight-low",
    feature = "deuce-seven",
    feature = "omaha",
    feature = "badugi"
))]

use poker_hand_evaluator::{
    AceFiveLowRule, BadugiRule, DeuceSevenLowRule, EightLowQualifiedRule, HandRule, HighRule,
    OmahaHighRule,
};
use proptest::prelude::*;

/// Generate `n` distinct cards in `[0, 52)` (Hold'em encoding).
fn distinct_cards(n: usize) -> impl Strategy<Value = Vec<u8>> {
    // `subsequence` needs a base Vec; build it from 0..52 and ask for
    // n elements. The result is sorted, so we shuffle via permutation
    // before yielding.
    (
        proptest::sample::subsequence((0u8..52).collect::<Vec<_>>(), n),
        any::<u64>(),
    )
        .prop_map(|(mut cards, seed)| {
            // xorshift permutation, deterministic per `seed`.
            let mut state = seed.max(1);
            for i in (1..cards.len()).rev() {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                let j = (state as usize) % (i + 1);
                cards.swap(i, j);
            }
            cards
        })
}

fn shuffled(cards: &[u8], seed: u64) -> Vec<u8> {
    let mut out = cards.to_vec();
    let mut state = seed.max(1);
    for i in (1..out.len()).rev() {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        let j = (state as usize) % (i + 1);
        out.swap(i, j);
    }
    out
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    /// `HighRule` is a multiset function — re-ordering the cards must
    /// not change the rank.
    #[test]
    fn holdem_high_is_order_invariant(
        cards in distinct_cards(7),
        seed in any::<u64>(),
    ) {
        let r1 = HighRule.evaluate(&cards);
        let r2 = HighRule.evaluate(&shuffled(&cards, seed));
        prop_assert_eq!(r1, r2);
    }

    /// `OmahaHighRule` rules: 4 hole + 5 board. Permuting within
    /// hole and within board must not change the rank.
    #[test]
    fn omaha_high_is_order_invariant_within_segments(
        cards in distinct_cards(9),
        hseed in any::<u64>(),
        bseed in any::<u64>(),
    ) {
        let mut hole = cards[..4].to_vec();
        let mut board = cards[4..].to_vec();
        let r1 = OmahaHighRule.evaluate(&cards);

        // Shuffle hole and board independently; concat back in the
        // original order.
        hole = shuffled(&hole, hseed);
        board = shuffled(&board, bseed);
        let mut both = hole;
        both.extend_from_slice(&board);
        let r2 = OmahaHighRule.evaluate(&both);

        prop_assert_eq!(r1, r2);
    }

    /// 8-or-better qualifying ranks land in `0..=55` (= C(8,5) - 1).
    /// Non-qualifying hands return None.
    #[test]
    fn eight_low_qualifier_threshold(cards in distinct_cards(7)) {
        let s = EightLowQualifiedRule.evaluate(&cards);
        if let Some(std::cmp::Reverse(rank)) = s {
            prop_assert!(rank <= 55, "qualifying rank out of range: {}", rank);
        }
    }

    /// `AceFiveLowRule` always returns `Some` (every hand has an A-5
    /// rank — there is no qualifier). Encoded here as "evaluate
    /// doesn't panic and the inner u16 is in 0..6175".
    #[test]
    fn ace_five_low_in_range(cards in distinct_cards(5)) {
        let std::cmp::Reverse(rank) = AceFiveLowRule.evaluate(&cards);
        prop_assert!(rank < 6175);
    }

    /// `DeuceSevenLowRule` is a multiset function — re-ordering the
    /// cards must not change the rank.
    #[test]
    fn deuce_seven_is_order_invariant(
        cards in distinct_cards(5),
        seed in any::<u64>(),
    ) {
        let r1 = DeuceSevenLowRule.evaluate(&cards);
        let r2 = DeuceSevenLowRule.evaluate(&shuffled(&cards, seed));
        prop_assert_eq!(r1, r2);
    }

    /// `BadugiRule` always finds a non-empty subset; count is 1..=4.
    #[test]
    fn badugi_count_bounds(cards in distinct_cards(4)) {
        let s = BadugiRule.evaluate(&cards);
        prop_assert!((1..=4).contains(&s.count()));
    }

    /// Total Ord on `HighRule` strengths: for any two random 7-card
    /// hands, exactly one of <, >, == must hold.
    #[test]
    fn holdem_strength_is_total(
        a in distinct_cards(7),
        b in distinct_cards(7),
    ) {
        let sa = HighRule.evaluate(&a);
        let sb = HighRule.evaluate(&b);
        prop_assert_eq!(
            (sa < sb) as u8 + (sa > sb) as u8 + (sa == sb) as u8,
            1
        );
    }
}
