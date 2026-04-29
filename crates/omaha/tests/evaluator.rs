//! Pin-down tests for `OmahaHighRule`.
//!
//! The Omaha "must use exactly 2 hole + 3 board" rule is the trap most
//! novice implementations fall into; these tests focus on the cases
//! where naive Hold'em-style "best 5 of 9" would give a wrong answer.

use phe_holdem::{get_hand_category, parse_hand, HandCategory};
use phe_omaha::OmahaHighRule;

fn cards(s: &str) -> Vec<usize> {
    let h = parse_hand(s).unwrap();
    let mut out = Vec::new();
    for i in 0..52usize {
        if h.contains(i) {
            out.push(i);
        }
    }
    // parse_hand uses bit-mask; preserve the input order by re-walking.
    // Re-implement manually to keep order.
    let mut chars = s.chars();
    let mut ordered = Vec::new();
    while let Some(r) = chars.next() {
        let s = chars.next().unwrap();
        let rank = match r.to_ascii_uppercase() {
            '2' => 0,
            '3' => 1,
            '4' => 2,
            '5' => 3,
            '6' => 4,
            '7' => 5,
            '8' => 6,
            '9' => 7,
            'T' => 8,
            'J' => 9,
            'Q' => 10,
            'K' => 11,
            'A' => 12,
            _ => panic!(),
        };
        let suit = match s.to_ascii_lowercase() {
            'c' => 0,
            'd' => 1,
            'h' => 2,
            's' => 3,
            _ => panic!(),
        };
        ordered.push(rank * 4 + suit);
    }
    let _ = out; // silence unused
    ordered
}

fn hole(s: &str) -> [usize; 4] {
    let v = cards(s);
    assert_eq!(v.len(), 4, "hole must be 4 cards");
    [v[0], v[1], v[2], v[3]]
}

fn board(s: &str) -> [usize; 5] {
    let v = cards(s);
    assert_eq!(v.len(), 5, "board must be 5 cards");
    [v[0], v[1], v[2], v[3], v[4]]
}

#[test]
fn royal_flush_on_board_unplayable_without_matching_hole_cards() {
    // Board is a royal flush in hearts. Hole has no hearts.
    // Naive "best 5 of 9" would say royal flush; Omaha says no.
    let r = OmahaHighRule::evaluate(&hole("As2c3c4c"), &board("AhKhQhJhTh"));
    assert_ne!(get_hand_category(r), HandCategory::StraightFlush);
    assert_ne!(get_hand_category(r), HandCategory::Flush);
}

#[test]
fn royal_flush_playable_with_two_hole_hearts() {
    // Hole has AhKh, board has QhJhTh + two non-hearts.
    let r = OmahaHighRule::evaluate(&hole("AhKh2c3c"), &board("QhJhTh4d5d"));
    assert_eq!(get_hand_category(r), HandCategory::StraightFlush);
}

#[test]
fn pocket_aces_with_aaa_board_makes_quads_not_more() {
    // Hole AsAd + 2 useless. Board AhAcKsQsJs.
    // Best 5 = 2 hole aces + 3 board (Ah, Ac + 1 kicker). 4 aces.
    let r = OmahaHighRule::evaluate(&hole("AsAd2c3c"), &board("AhAcKsQsJs"));
    assert_eq!(get_hand_category(r), HandCategory::FourOfAKind);
}

#[test]
fn straight_requires_two_hole_cards() {
    // Board is a made straight 6-7-8-9-T. Hole has irrelevant cards.
    // Naive "best 5 of 9" would say straight; Omaha says no — must use
    // exactly 2 hole cards.
    let r = OmahaHighRule::evaluate(&hole("2c2d3c3d"), &board("6c7d8h9sTc"));
    assert_ne!(get_hand_category(r), HandCategory::Straight);
}

#[test]
fn straight_playable_with_two_connecting_hole_cards() {
    // Hole 5c6d, board 7h8s9c + two unrelated. Use 5,6 from hole and
    // 7,8,9 from board → 5-6-7-8-9 straight.
    let r = OmahaHighRule::evaluate(&hole("5c6d2s3s"), &board("7h8s9cKdJh"));
    assert_eq!(get_hand_category(r), HandCategory::Straight);
}

#[test]
fn flush_requires_two_hole_cards_of_the_same_suit() {
    // Board has 4 hearts. Hole has only 1 heart.
    // Naive "any 5" would say flush; Omaha says no.
    let r = OmahaHighRule::evaluate(&hole("Ah2c3c4c"), &board("KhQhJhTh9d"));
    // Best legal 5-card: AsKsJsTs + Ah → no, hole gives only Ah heart.
    // Use 2 hole + 3 board: Ah, ?(any non-heart from hole) + 3 board hearts → 4 hearts max.
    // So no flush.
    assert_ne!(get_hand_category(r), HandCategory::Flush);
    assert_ne!(get_hand_category(r), HandCategory::StraightFlush);
}

#[test]
fn flush_playable_with_two_hole_hearts() {
    let r = OmahaHighRule::evaluate(&hole("AhKh2c3c"), &board("QhJh9h2d3d"));
    // 2 hole hearts + 3 board hearts = flush A-high.
    assert_eq!(get_hand_category(r), HandCategory::Flush);
}

#[test]
fn full_house_via_2_hole_pair_3_board_trips() {
    // Hole AA (+ 2 random), board KKK + 2 random. Best: AA + KKK = full house.
    let r = OmahaHighRule::evaluate(&hole("AsAd2c3c"), &board("KsKhKd5d6h"));
    assert_eq!(get_hand_category(r), HandCategory::FullHouse);
}
