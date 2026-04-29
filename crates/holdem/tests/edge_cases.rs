//! Concrete-value edge cases ported verbatim from b-inary/holdem-hand-evaluator.
//!
//! These pin every hand-category boundary to a specific 7-card hand and
//! its expected 16-bit rank, so any regression in the lookup tables or
//! the dispatch path shows up here immediately.

use phe_holdem::{parse_hand, HighRule};

fn rank(s: &str) -> u16 {
    let h = parse_hand(s).unwrap();
    assert_eq!(h.len(), 7);
    HighRule::evaluate(&h)
}

#[test]
fn straight_flushes() {
    assert_eq!(rank("AsKsQsJsTs7d5s"), (8 << 12) + 9);
    assert_eq!(rank("Ac7c6c5c4c3c2c"), (8 << 12) + 2);
    assert_eq!(rank("AdQsJc5d4d3d2d"), (8 << 12) + 0);
}

#[test]
fn four_of_a_kinds() {
    assert_eq!(rank("AsAcAhAdKsQcTh"), (7 << 12) + 155);
    assert_eq!(rank("3d3h3s2c2d2h2s"), (7 << 12) + 0);
}

#[test]
fn full_houses() {
    assert_eq!(rank("AsAdAhKcKdKh2d"), (6 << 12) + 155);
    assert_eq!(rank("4h4c3s3c2d2c2h"), (6 << 12) + 1);
    assert_eq!(rank("5h4c3s3c2d2c2h"), (6 << 12) + 0);
}

#[test]
fn flushes() {
    assert_eq!(rank("AhKhQhJh9h9c9s"), (5 << 12) + 1276);
    assert_eq!(rank("Js7c6d5c4c3c2c"), (5 << 12) + 0);
}

#[test]
fn straights() {
    assert_eq!(rank("AhKcKdKhQcJdTs"), (4 << 12) + 9);
    assert_eq!(rank("Ac8c7c5d4d3d2d"), (4 << 12) + 0);
}

#[test]
fn three_of_a_kinds() {
    assert_eq!(rank("AsAcAhKhQd5c3s"), (3 << 12) + 857);
    assert_eq!(rank("7d5c4c3c2d2s2h"), (3 << 12) + 8);
}

#[test]
fn two_pairs() {
    assert_eq!(rank("AsAhKsKhQsQhJs"), (2 << 12) + 857);
    assert_eq!(rank("7c6d5h3s3c2d2h"), (2 << 12) + 3);
}

#[test]
fn one_pairs() {
    assert_eq!(rank("AdAsKhQdJs3s2c"), (1 << 12) + 2859);
    assert_eq!(rank("8s7s5h4c3c2d2c"), (1 << 12) + 18);
}

#[test]
fn high_cards() {
    assert_eq!(rank("AdKdQdJd9s3h2c"), (0 << 12) + 1276);
    assert_eq!(rank("9h8s7d5d4d3c2d"), (0 << 12) + 48);
}

#[test]
fn hand_addition_then_evaluate() {
    let hand1 = parse_hand("4h4c").unwrap();
    let hand2 = parse_hand("5h4s").unwrap();
    let board = parse_hand("3s3c2d2c2h").unwrap();
    assert_eq!(HighRule::evaluate(&(hand1 + board)), (6 << 12) + 1);
    assert_eq!(HighRule::evaluate(&(hand2 + board)), (6 << 12) + 0);
}
