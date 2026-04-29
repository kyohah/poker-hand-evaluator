//! Cross-check: optimised eval vs naive 60-combo eval.
//!
//! The optimised `OmahaHighRule::evaluate` dispatches between a full
//! eval path and a rank-only fast path. Since both paths must return
//! the same value, we run a side-by-side comparison on a fixed
//! sample of (hole, board) configurations covering each branch.

use phe_core::Hand;
use phe_holdem::HighRule;
use phe_omaha::OmahaHighRule;

/// Naive reference: enumerate every (2 hole + 3 board) combination
/// using the full Hold'em eval; take the maximum. No suit-aware
/// dispatch.
fn naive(hole: &[usize; 4], board: &[usize; 5]) -> u16 {
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
            let h = hp.add_card(board[a]).add_card(board[b]).add_card(board[c]);
            best = best.max(HighRule::evaluate(&h));
        }
    }
    best
}

fn cards(s: &str) -> Vec<usize> {
    let mut chars = s.chars();
    let mut out = Vec::new();
    while let Some(r) = chars.next() {
        let suit = chars.next().unwrap();
        let rank = match r.to_ascii_uppercase() {
            '2' => 0, '3' => 1, '4' => 2, '5' => 3, '6' => 4, '7' => 5, '8' => 6,
            '9' => 7, 'T' => 8, 'J' => 9, 'Q' => 10, 'K' => 11, 'A' => 12, _ => panic!(),
        };
        let s = match suit.to_ascii_lowercase() {
            'c' => 0, 'd' => 1, 'h' => 2, 's' => 3, _ => panic!(),
        };
        out.push(rank * 4 + s);
    }
    out
}

fn hole(s: &str) -> [usize; 4] { let v = cards(s); [v[0], v[1], v[2], v[3]] }
fn board(s: &str) -> [usize; 5] { let v = cards(s); [v[0], v[1], v[2], v[3], v[4]] }

/// Coverage matrix: (hole, board) pairs that exercise both eval paths.
fn fixtures() -> Vec<([usize; 4], [usize; 5], &'static str)> {
    vec![
        // -- rank-only fast path (flush impossible) --
        (hole("As2c3d4h"), board("Kc7d6h5s9c"), "rainbow board, mixed hole"),
        (hole("KsQs2c3c"), board("9d8d7c6c5h"), "no 3-of-suit on board"),
        (hole("AhKhQhJh"), board("2c3c4d5s6s"), "4 hearts in hole, only 1 on board"),
        (hole("AsAhKsKh"), board("2c2d3c3d4h"), "two pair on board"),

        // -- full path (flush possible) --
        (hole("AhKh2c3c"), board("4h5h6h7d8d"), "3 hearts on board + 2 in hole"),
        (hole("AhKh2c3c"), board("4h3h5h8h9h"), "user-pattern monotone hearts board"),
        (hole("AsAhKsKh"), board("4s5s6s7s8s"), "5 spades board, 2 spades hole"),

        // -- borderline --
        (hole("AsKsQsJs"), board("Tc9c8c7c6h"), "4 clubs on board + 0 in hole (no flush)"),
        (hole("AcKc2d3d"), board("Qc6c5c4d3h"), "exactly 2 clubs hole + 3 clubs board"),
    ]
}

#[test]
fn optimised_matches_naive_across_path_split_fixtures() {
    for (h, b, label) in fixtures() {
        let opt = OmahaHighRule::evaluate(&h, &b);
        let naive = naive(&h, &b);
        assert_eq!(opt, naive, "mismatch on '{}': opt={}, naive={}", label, opt, naive);
    }
}
