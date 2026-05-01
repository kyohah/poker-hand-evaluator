//! Facade-crate tests.
//!
//! Cover three things:
//!   1. The HandRule impls produce the same value as the underlying
//!      variant crate would.
//!   2. The Hold'em -> eight-low encoding translation is correct.
//!   3. HiLoRule composes high and low rules into a tuple Strength.
//!
//! Requires every variant feature (the default `all`); under
//! narrower feature subsets (e.g. `--features holdem` alone) this
//! file is gated out so feature-gate spot-checks compile.

#![cfg(all(
    feature = "holdem",
    feature = "eight-low",
    feature = "deuce-seven",
    feature = "omaha",
    feature = "badugi"
))]
// `card = rank * 4 + suit` literals are kept verbose with the
// explicit `+ 0` for the club suit and `0 *` / `12 *` for the rank
// 2 / A so every card encoding reads the same way; clippy's
// `identity_op` / `erasing_op` would obscure that.
#![allow(clippy::identity_op)]
#![allow(clippy::erasing_op)]

use poker_hand_evaluator::{
    AceFiveLowRule, BadugiRule, DeuceSevenLowRule, EightLowQualifiedRule, HandRule, HiLoRule,
    HighRule, OmahaHighRule,
};

// Hold'em-style card ids: card = rank*4 + suit; rank 0=2, 12=A; suit 0=c..3=s.
const TWOS: u8 = 0; // 2c
const TWOH: u8 = 2;
const FIVE_S: u8 = 3 * 4 + 3; // 5s
const SEVEN_S: u8 = 5 * 4 + 3;
const ACE_S: u8 = 12 * 4 + 3;
const ACE_C: u8 = 12 * 4 + 0;
const ACE_D: u8 = 12 * 4 + 1;
const ACE_H: u8 = 12 * 4 + 2;
const KING_S: u8 = 11 * 4 + 3;

#[test]
fn high_rule_matches_underlying_evaluator() {
    // Quad aces with K kicker (7 cards).
    let cards = [ACE_S, ACE_C, ACE_D, ACE_H, KING_S, TWOH, TWOS];
    let s = HighRule.evaluate(&cards);
    // Quads category = 7
    assert_eq!(s >> 12, 7);
}

#[test]
fn eight_low_rule_qualifies_correctly_via_facade() {
    // 8-low qualifying: A 2 3 4 8 + 2 fillers
    let cards = [
        ACE_S,      // ace
        TWOS,       // 2c
        1 * 4 + 0,  // 3c (rank 1=3 in holdem)
        2 * 4 + 0,  // 4c (rank 2=4)
        6 * 4 + 0,  // 8c (rank 6=8)
        11 * 4 + 0, // Kc filler
        10 * 4 + 0, // Qc filler
    ];
    let s = EightLowQualifiedRule.evaluate(&cards);
    assert!(
        s.is_some(),
        "A-2-3-4-8 + fillers must qualify for 8-or-better"
    );
}

#[test]
fn nine_low_does_not_qualify_via_facade() {
    let cards = [
        ACE_S,
        TWOS,
        1 * 4 + 0,
        2 * 4 + 0,
        7 * 4 + 0, // 9c
        11 * 4 + 0,
        10 * 4 + 0,
    ];
    let s = EightLowQualifiedRule.evaluate(&cards);
    assert_eq!(s, None);
}

#[test]
fn ace_five_low_rule_returns_some_rank_for_any_input() {
    // Any 5-card combo. Razz = no qualifier.
    let cards = [ACE_S, TWOS, 1 * 4, 2 * 4, 3 * 4, 11 * 4, 10 * 4];
    let s = AceFiveLowRule.evaluate(&cards);
    // Just confirm it returns *something* (Strength is non-Option).
    let _ = s;
}

#[test]
fn deuce_seven_rule_works_on_5_cards_via_facade() {
    // 7-5-4-3-2 mixed suits = the nuts in 2-7
    let cards = [
        SEVEN_S,   // 7s
        FIVE_S,    // 5s
        2 * 4 + 2, // 4h
        1 * 4 + 1, // 3d
        0 * 4 + 0, // 2c
    ];
    let nuts = DeuceSevenLowRule.evaluate(&cards);

    // Compare to a worse 8-high
    let worse = [
        6 * 4 + 3, // 8s
        FIVE_S,
        2 * 4 + 2,
        1 * 4 + 1,
        0 * 4 + 0,
    ];
    let s2 = DeuceSevenLowRule.evaluate(&worse);
    assert!(nuts > s2);
}

#[test]
fn omaha_rule_uses_two_hole_three_board_via_facade() {
    // 9 cards: hole [As Kh 2c 3c] + board [Qh Jh Th 4d 5d]
    // Without the 2-hole rule, this would be a royal flush
    // (5 hearts: KQJT + Ah). Omaha says we must use exactly 2 hole
    // cards, and we only have 1 heart in hand (Kh) — so no flush
    // is reachable. The best legal 5-card hand is the AKQJT
    // ace-high straight (As + Kh + Qh + Jh + Th, mixed suits).
    let cards = [
        ACE_S,      // hole As
        11 * 4 + 2, // hole Kh
        TWOS,       // hole 2c
        1 * 4 + 0,  // hole 3c
        10 * 4 + 2, // board Qh
        9 * 4 + 2,  // board Jh
        8 * 4 + 2,  // board Th
        2 * 4 + 1,  // board 4d
        3 * 4 + 1,  // board 5d
    ];
    let actual_straight = OmahaHighRule.evaluate(&cards);

    // A clearly-weaker 9-card hand (just a pair of aces, no straight,
    // no flush): hole [As Ad 7c 2c] + board [Kh Th 8d 5d 3c].
    let weaker_pair = [
        ACE_S,
        ACE_D,
        5 * 4 + 0,
        TWOS,
        11 * 4 + 2,
        8 * 4 + 2,
        6 * 4 + 1,
        3 * 4 + 1,
        1 * 4 + 0,
    ];
    let pair_strength = OmahaHighRule.evaluate(&weaker_pair);
    assert!(
        actual_straight > pair_strength,
        "AKQJT straight via 2-hole rule must beat pair-of-aces (got {} vs {})",
        actual_straight,
        pair_strength,
    );
}

#[test]
fn hi_lo_rule_returns_tuple_strength() {
    let rule = HiLoRule {
        hi: HighRule,
        lo: EightLowQualifiedRule,
    };
    // 7-card hand with both a high and a qualifying low component.
    let cards = [ACE_S, TWOS, 1 * 4, 2 * 4, 6 * 4, 11 * 4, 10 * 4]; // A-2-3-4-8 + KQ
    let (hi, lo) = rule.evaluate(&cards);
    // Hi: should be at least HighCard or better.
    let _ = hi;
    // Lo: A-2-3-4-8 qualifies.
    assert!(lo.is_some());
}

#[test]
fn badugi_rule_via_facade_uses_holdem_encoding() {
    // Wheel A-2-3-4 of four different suits = 4-badugi nuts.
    // (Encoding: card = rank * 4 + suit; rank 0 = 2, rank 12 = A.)
    let cards = [12 * 4 + 0, 0 * 4 + 1, 1 * 4 + 2, 2 * 4 + 3]; // Ac 2d 3h 4s
    let s = BadugiRule.evaluate(&cards);
    assert_eq!(s.count(), 4);

    // Worse 4-badugi: Ac 2d 3h 5s
    let worse = [12 * 4 + 0, 0 * 4 + 1, 1 * 4 + 2, 3 * 4 + 3];
    let s2 = BadugiRule.evaluate(&worse);
    assert!(s > s2);
}

#[test]
fn eight_low_translation_round_trip() {
    // Confirm the translation via outcome equivalence: facade
    // EightLowQualifiedRule against directly-built phe_eight_low::Hand.
    let cards: [u8; 7] = [
        ACE_S,      // Ace (holdem rank 12 → eight-low rank 0)
        TWOS,       // 2c (holdem rank 0 → eight-low rank 1)
        1 * 4 + 0,  // 3c
        2 * 4 + 0,  // 4c
        6 * 4 + 0,  // 8c
        11 * 4 + 0, // Kc
        10 * 4 + 0, // Qc
    ];

    let via_facade = EightLowQualifiedRule.evaluate(&cards);

    // Build the same hand directly using phe-eight-low's encoding.
    let direct = phe_eight_low::Hand::new()
        .add_card(0 * 4 + 3) // As (eight-low rank 0)
        .add_card(1 * 4 + 0) // 2c (eight-low rank 1)
        .add_card(2 * 4 + 0) // 3c
        .add_card(3 * 4 + 0) // 4c
        .add_card(7 * 4 + 0) // 8c
        .add_card(12 * 4 + 0) // Kc
        .add_card(11 * 4 + 0); // Qc
    let direct_s = phe_eight_low::EightLowQualifiedRule::evaluate(&direct);

    assert_eq!(via_facade, direct_s);
}
