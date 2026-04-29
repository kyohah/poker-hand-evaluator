//! Tests for the flush-dominates fast path.
//!
//! When the board has 5 distinct ranks (no rank repeats) and at least
//! one suit has both ≥3 board cards and ≥2 hole cards, full house and
//! quads are mathematically unreachable for *any* (hole_pair,
//! board_triple) combo, so the best 5-card hand is the best flush.
//! In that case the optimised eval enumerates only the suit-restricted
//! combos (`C(hole_s, 2) × C(board_s, 3)`, often as few as 1) instead
//! of all 60.
//!
//! These tests pin (a) the structural classifiers and (b) the
//! end-to-end correctness vs the naive enumerator.

use phe_holdem::{get_hand_category, HandCategory};
use phe_omaha::{board_has_no_pair, flush_suit, OmahaHighRule};

fn card(rank: char, suit: char) -> usize {
    let r = match rank {
        '2' => 0, '3' => 1, '4' => 2, '5' => 3, '6' => 4, '7' => 5, '8' => 6,
        '9' => 7, 'T' => 8, 'J' => 9, 'Q' => 10, 'K' => 11, 'A' => 12,
        _ => panic!(),
    };
    let s = match suit {
        'c' => 0, 'd' => 1, 'h' => 2, 's' => 3, _ => panic!(),
    };
    r * 4 + s
}

fn parse_4(s: &str) -> [usize; 4] {
    let mut chars = s.chars();
    let mut out = [0usize; 4];
    for slot in &mut out {
        let r = chars.next().unwrap();
        let su = chars.next().unwrap();
        *slot = card(r, su);
    }
    out
}

fn parse_5(s: &str) -> [usize; 5] {
    let mut chars = s.chars();
    let mut out = [0usize; 5];
    for slot in &mut out {
        let r = chars.next().unwrap();
        let su = chars.next().unwrap();
        *slot = card(r, su);
    }
    out
}

// --- board_has_no_pair classifier ---------------------------------------

#[test]
fn five_distinct_ranks_yields_no_pair() {
    assert!(board_has_no_pair(&parse_5("4h3h5h8h9h")));
    assert!(board_has_no_pair(&parse_5("AsKhQdJc2c")));
    assert!(board_has_no_pair(&parse_5("2c3d4h5s7c")));
}

#[test]
fn one_pair_on_board_detected() {
    assert!(!board_has_no_pair(&parse_5("4h4d5s8c9c"))); // 4-pair
    assert!(!board_has_no_pair(&parse_5("AsAhKdQcJc"))); // A-pair
}

#[test]
fn two_pair_on_board_detected() {
    assert!(!board_has_no_pair(&parse_5("4h4d5s5c9c")));
}

#[test]
fn trips_on_board_detected() {
    assert!(!board_has_no_pair(&parse_5("4h4d4s5c9c")));
}

// --- flush_suit identifier ----------------------------------------------

#[test]
fn flush_suit_returns_suit_index_when_eligible() {
    // Hearts: hole has Ah, Kh (2); board has 3 hearts → flush_suit = 2
    assert_eq!(
        flush_suit(&parse_4("AhKh2c3c"), &parse_5("4h5h6h7d8d")),
        Some(2)
    );
}

#[test]
fn flush_suit_none_for_rainbow() {
    // No suit has both ≥2 hole and ≥3 board.
    assert_eq!(
        flush_suit(&parse_4("AsKhQcJd"), &parse_5("2c3d4h5s7c")),
        None
    );
}

#[test]
fn flush_suit_none_when_only_one_hole_card_matches_3_suited_board() {
    // Hole has only 1 heart; need 2.
    assert_eq!(
        flush_suit(&parse_4("Ah2c3c4c"), &parse_5("KhQhJh4d5d")),
        None
    );
}

// --- end-to-end correctness on fast-path inputs --------------------------

/// Naive 60-combo enumerator for ground truth.
fn naive(hole: &[usize; 4], board: &[usize; 5]) -> u16 {
    use phe_core::Hand;
    use phe_holdem::HighRule;
    const HOLE_PAIRS: [(usize, usize); 6] =
        [(0, 1), (0, 2), (0, 3), (1, 2), (1, 3), (2, 3)];
    const BOARD_TRIPLES: [(usize, usize, usize); 10] = [
        (0, 1, 2), (0, 1, 3), (0, 1, 4), (0, 2, 3), (0, 2, 4),
        (0, 3, 4), (1, 2, 3), (1, 2, 4), (1, 3, 4), (2, 3, 4),
    ];
    let mut best = 0u16;
    for &(i, j) in &HOLE_PAIRS {
        let hp = Hand::new().add_card(hole[i]).add_card(hole[j]);
        for &(a, b, c) in &BOARD_TRIPLES {
            let h = hp
                .add_card(board[a])
                .add_card(board[b])
                .add_card(board[c]);
            best = best.max(HighRule::evaluate(&h));
        }
    }
    best
}

#[test]
fn user_pattern_monotone_board_two_hearts_in_hole_yields_flush_a_high() {
    // The user's running example.
    let hole = parse_4("AhKh2c3c");
    let board = parse_5("4h3h5h8h9h"); // 5 hearts, 5 distinct ranks
    let r = OmahaHighRule::evaluate(&hole, &board);
    assert_eq!(r, naive(&hole, &board));
    assert_eq!(get_hand_category(r), HandCategory::Flush);
}

#[test]
fn straight_flush_chosen_over_top_card_flush() {
    // Hole 6h5h, board 4h3h2h8h7h (monotone, 5 distinct).
    // Top-card heuristic would pick 8h,7h,5h+6h,5h... wait hole has 5h&6h.
    // Let's design: hole 7h6h, board 5h4h3h Kh Qh.
    // Top 3 board hearts by rank: K, Q, 5 → flush K-high (no SF).
    // Picking 5h,4h,3h: combined with 7h,6h gives 7-6-5-4-3 = SF.
    // The optimised eval must return the SF rank, not the higher-card flush.
    let hole = parse_4("7h6h2c3c");
    let board = parse_5("5h4h3hKhQh");
    let r = OmahaHighRule::evaluate(&hole, &board);
    assert_eq!(r, naive(&hole, &board));
    assert_eq!(get_hand_category(r), HandCategory::StraightFlush);
}

#[test]
fn no_flush_path_when_only_one_hole_heart() {
    // Hole has 1 heart, monotone-hearts board → no flush combo.
    // Best is some pair / high card. Optimised must agree with naive.
    let hole = parse_4("Ah2c3d4s");
    let board = parse_5("KhQhJhTh9h");
    let r = OmahaHighRule::evaluate(&hole, &board);
    assert_eq!(r, naive(&hole, &board));
}

#[test]
fn flush_dominates_with_three_hole_cards_in_suit() {
    // Hole has 3 hearts; eval must pick the right 2 of the 3.
    let hole = parse_4("AhKh5h2c");
    let board = parse_5("4h3h6h8d9c"); // 3 hearts, no pair
    let r = OmahaHighRule::evaluate(&hole, &board);
    assert_eq!(r, naive(&hole, &board));
    // Best flush from 5 hearts: A,K,5,4,3 (or A,K,6,4,3 — wait only 3 board hearts: 4h, 3h, 6h)
    // hole hearts choose 2 of {Ah, Kh, 5h}, board hearts must be all 3 of {4h, 3h, 6h}.
    // So flush combos: {A,K,4,3,6}, {A,5,4,3,6}, {K,5,4,3,6}. Best = A,K,6,4,3 (flush A-high).
    assert_eq!(get_hand_category(r), HandCategory::Flush);
}

#[test]
fn flush_eligible_but_board_paired_uses_full_eval() {
    // Board has a pair → can't use flush-dominates; FH possible.
    let hole = parse_4("AhKhQhJh"); // 4 hearts
    let board = parse_5("AsAdAcKsKc"); // pair / trips on board
    // Quads of aces from board (3) + hole (1) is impossible (2 hole used for FH).
    // Best: hole pair (Ah, Kh) + board (As, Kc, ...) → AAKKK = FH if combinable.
    // Actually hole (Ah, Kh) + board (As, Ad, Ks) = AAAKK = FH. Good.
    let r = OmahaHighRule::evaluate(&hole, &board);
    assert_eq!(r, naive(&hole, &board));
}
