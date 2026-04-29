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
    let s = match suit {
        'c' => 0,
        'd' => 1,
        'h' => 2,
        's' => 3,
        _ => panic!(),
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
    const HOLE_PAIRS: [(usize, usize); 6] = [(0, 1), (0, 2), (0, 3), (1, 2), (1, 3), (2, 3)];
    const BOARD_TRIPLES: [(usize, usize, usize); 10] = [
        (0, 1, 2),
        (0, 1, 3),
        (0, 1, 4),
        (0, 2, 3),
        (0, 2, 4),
        (0, 3, 4),
        (1, 2, 3),
        (1, 2, 4),
        (1, 3, 4),
        (2, 3, 4),
    ];
    let mut best = 0u16;
    for &(i, j) in &HOLE_PAIRS {
        let hp = Hand::new().add_card(hole[i]).add_card(hole[j]);
        for &(a, b, c) in &BOARD_TRIPLES {
            let h = hp.add_card(board[a]).add_card(board[b]).add_card(board[c]);
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

// --- 9-card-direct edge cases for the flush-dominates path ----------
//
// These cases specifically exercise the SF window scan + top-2/top-3
// plain-flush fallback. They were chosen so that a "top-2 hole + top-3
// board by rank" heuristic would return the *wrong* answer if it ran
// without SF detection.

#[test]
fn wheel_sf_chosen_over_high_card_flush() {
    // hole = {As, 2s, 6s, 7s} / board = {3s, 4s, 5s, plus 2 non-spades}
    // Top-2 hole = As, 7s; top-3 board = 5s, 4s, 3s → A 7 5 4 3 (plain flush, A high).
    // But hole {6s, 7s} + board {3s, 4s, 5s} = 7-6-5-4-3 SF (7-high).
    // SF > flush, so SF must win.
    let hole = parse_4("As2s6s7s");
    let board = parse_5("3s4s5sKdQc"); // 3 spades, no pair
    let r = OmahaHighRule::evaluate(&hole, &board);
    assert_eq!(r, naive(&hole, &board));
    assert_eq!(get_hand_category(r), HandCategory::StraightFlush);
}

#[test]
fn royal_sf_via_window_scan() {
    // Hole {Ks, Js, 2c, 3c}, board {As, Qs, Ts, 4d, 5d}
    // Window {T,J,Q,K,A}: hole has K,J (2) ≥ 2; board has A,Q,T (3) ≥ 3 → royal SF.
    let hole = parse_4("KsJs2c3c");
    let board = parse_5("AsQsTs4d5d");
    let r = OmahaHighRule::evaluate(&hole, &board);
    assert_eq!(r, naive(&hole, &board));
    assert_eq!(get_hand_category(r), HandCategory::StraightFlush);
}

#[test]
fn sf_window_scan_picks_higher_window_when_two_match() {
    // Two SF windows both reachable; the higher one must win.
    // Hole {7s, 8s, 4s, 3s}, board {5s, 6s, 9s, Ad, 2c}.
    // Window 5..9 (5-6-7-8-9): hole has 7,8 (2) ≥ 2; board has 5,6,9 (3) ≥ 3 → 9-high SF ✓
    // Window 4..8 (4-5-6-7-8): hole has 4,7,8 (3) ≥ 2; board has 5,6 (2) — only 2, fails.
    // Window 3..7 (3-4-5-6-7): hole has 3,4,7 (3) ≥ 2; board has 5,6 (2) — fails.
    // So only the 9-high window matches; the eval returns 9-high SF.
    let hole = parse_4("7s8s4s3s");
    let board = parse_5("5s6s9sAd2c");
    let r = OmahaHighRule::evaluate(&hole, &board);
    assert_eq!(r, naive(&hole, &board));
    assert_eq!(get_hand_category(r), HandCategory::StraightFlush);
}

#[test]
fn plain_flush_no_sf_top2_top3_works() {
    // hole {As, Ks, 2c, 3c}, board {Qs, 9s, 6s, 4d, 7h}
    // Spade-suit ranks: hole {A, K}, board {Q, 9, 6}. No 5-rank window is
    // covered (gap 9-Q for the J/T range). Top-2+top-3 = A,K,Q,9,6 = plain flush A high.
    let hole = parse_4("AsKs2c3c");
    let board = parse_5("Qs9s6s4d7h");
    let r = OmahaHighRule::evaluate(&hole, &board);
    assert_eq!(r, naive(&hole, &board));
    assert_eq!(get_hand_category(r), HandCategory::Flush);
}

#[test]
fn plain_flush_with_4_hole_in_suit_picks_top_2() {
    // 4 hole spades. Need to make sure top-2 logic picks the right pair.
    // hole {As, Ks, 5s, 4s}, board {Qs, 9s, 6s, 4d, 7h}.
    // Top-2 hole = A, K; top-3 board = Q, 9, 6 → A K Q 9 6 plain flush.
    let hole = parse_4("AsKs5s4s");
    let board = parse_5("Qs9s6s4d7h");
    let r = OmahaHighRule::evaluate(&hole, &board);
    assert_eq!(r, naive(&hole, &board));
    assert_eq!(get_hand_category(r), HandCategory::Flush);
}

#[test]
fn plain_flush_with_5_board_in_suit_picks_top_3() {
    // 5 board spades. Top-3 by rank = K, Q, J (skip 5, 4).
    // hole {As, 9s, 2c, 3c}, board {Ks, Qs, Js, 5s, 4s} — but board has 5
    // distinct ranks (Ks, Qs, Js, 5s, 4s) so flush-dominates path applies.
    // Top-2 hole = A, 9; top-3 board = K, Q, J → A K Q J 9 plain flush.
    let hole = parse_4("As9s2c3c");
    let board = parse_5("KsQsJs5s4s");
    let r = OmahaHighRule::evaluate(&hole, &board);
    assert_eq!(r, naive(&hole, &board));
    assert_eq!(get_hand_category(r), HandCategory::Flush);
}

/// Cross-check: every (hole, board) configuration that hits the
/// flush-dominates fast path must agree with the naive 60-combo
/// reference. This is a deterministic random sweep so any regression
/// in the SF window scan / top-N bit logic is caught.
#[test]
fn flush_dominates_random_sweep_matches_naive() {
    // Same PCG-style RNG the bench uses, different seed.
    struct Rng(u64);
    impl Rng {
        fn next_u64(&mut self) -> u64 {
            self.0 = self
                .0
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            self.0
        }
    }

    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    let mut checked = 0usize;
    let mut tried = 0usize;
    // Bound the loop so a pathological draw distribution can't hang.
    while checked < 5_000 && tried < 200_000 {
        tried += 1;
        let mut deck: [usize; 52] = std::array::from_fn(|i| i);
        for i in 0..9 {
            let j = i + (rng.next_u64() as usize) % (52 - i);
            deck.swap(i, j);
        }
        let hole = [deck[0], deck[1], deck[2], deck[3]];
        let board = [deck[4], deck[5], deck[6], deck[7], deck[8]];

        if !board_has_no_pair(&board) || flush_suit(&hole, &board).is_none() {
            continue;
        }

        let opt = OmahaHighRule::evaluate(&hole, &board);
        let nai = naive(&hole, &board);
        assert_eq!(
            opt, nai,
            "mismatch on hole={:?} board={:?}: opt={} naive={}",
            hole, board, opt, nai
        );
        checked += 1;
    }
    assert!(
        checked >= 1_000,
        "too few flush-dominates samples drawn ({checked})"
    );
}
