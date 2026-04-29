//! Tests for the path-1 (no-flush) 9-card direct table lookup.
//!
//! Path 1 is hit when no suit has both ≥2 hole and ≥3 board cards
//! (i.e., no 5-card flush combo is reachable). The evaluator
//! collapses the 60-combo enumeration into a single
//! `NOFLUSH_LOOKUP[hole_idx * NUM_BOARD + board_idx]` access. These
//! tests pin equivalence with the naive 60-combo Hold'em-eval
//! reference across:
//!   - structural corner cases (rainbow board, hole all in one suit
//!     with low board, board with multiple pairs, etc.),
//!   - a deterministic 10K random sweep limited to no-flush fixtures.

use phe_core::Hand;
use phe_holdem::HighRule;
use phe_omaha::{flush_possible, OmahaHighRule};

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

fn card(rank: char, suit: char) -> usize {
    let r = match rank {
        '2' => 0, '3' => 1, '4' => 2, '5' => 3, '6' => 4, '7' => 5, '8' => 6,
        '9' => 7, 'T' => 8, 'J' => 9, 'Q' => 10, 'K' => 11, 'A' => 12,
        _ => panic!("bad rank: {rank}"),
    };
    let s = match suit {
        'c' => 0, 'd' => 1, 'h' => 2, 's' => 3, _ => panic!("bad suit: {suit}"),
    };
    r * 4 + s
}

fn hole(s: &str) -> [usize; 4] {
    let mut chars = s.chars();
    let mut out = [0usize; 4];
    for slot in &mut out {
        let r = chars.next().unwrap();
        let su = chars.next().unwrap();
        *slot = card(r, su);
    }
    out
}

fn board(s: &str) -> [usize; 5] {
    let mut chars = s.chars();
    let mut out = [0usize; 5];
    for slot in &mut out {
        let r = chars.next().unwrap();
        let su = chars.next().unwrap();
        *slot = card(r, su);
    }
    out
}

// --- structural corner cases that funnel into path 1 ---------------

#[test]
fn rainbow_board_high_card() {
    // 4 distinct hole suits, rainbow-ish board → no flush possible.
    let h = hole("As2c3d4h");
    let b = board("Kc7d6h5s9c");
    assert!(!flush_possible(&h, &b));
    let opt = OmahaHighRule::evaluate(&h, &b);
    assert_eq!(opt, naive(&h, &b));
}

#[test]
fn hole_pocket_pair_no_board_pair() {
    // Hole AcAd, board distinct ranks no flush.
    let h = hole("AcAd2c3d");
    let b = board("Ks7d8h5s9c");
    assert!(!flush_possible(&h, &b));
    let opt = OmahaHighRule::evaluate(&h, &b);
    assert_eq!(opt, naive(&h, &b));
}

#[test]
fn board_pair_full_house_via_pocket_pair() {
    // Hole AcAd; board has Ks pair so FH (AAA + KK) is possible.
    // Path 1 only fires when flush_suit == None — here Ks/Kh + 0
    // hole same-suit. Hole AcAd has clubs+diamonds; board has 1 spade
    // (Ks), 1 heart (Kh), 1 club (7c?), 1 diamond (5d), 1 heart (9h).
    // Verify no flush before checking.
    let h = hole("AcAd2c3d");
    let b = board("Ks7c5dKh9h");
    assert!(!flush_possible(&h, &b));
    let opt = OmahaHighRule::evaluate(&h, &b);
    assert_eq!(opt, naive(&h, &b));
}

#[test]
fn board_trips_quads_with_hole_match() {
    // Board has trips of 7s; hole has 7d (1 of the same rank) →
    // quads possible (3 board + 1 hole). Confirm no flush, then
    // check eval equality.
    let h = hole("7dAs2c3d");
    let b = board("7c7h7sQdJh");
    assert!(!flush_possible(&h, &b));
    let opt = OmahaHighRule::evaluate(&h, &b);
    assert_eq!(opt, naive(&h, &b));
}

#[test]
fn straight_via_two_hole_three_board() {
    // 9-T-J-Q-K straight using 2 hole + 3 board. Mixed suits, no flush.
    let h = hole("KcQc2d3h");
    let b = board("Js9hTd5c6s");
    assert!(!flush_possible(&h, &b));
    let opt = OmahaHighRule::evaluate(&h, &b);
    assert_eq!(opt, naive(&h, &b));
}

#[test]
fn wheel_straight_no_flush() {
    // A-2-3-4-5 wheel using As (hole) + 2c, 3d, 4h, 5s (board).
    let h = hole("AsKh7c8d");
    let b = board("2c3d4h5sJh");
    assert!(!flush_possible(&h, &b));
    let opt = OmahaHighRule::evaluate(&h, &b);
    assert_eq!(opt, naive(&h, &b));
}

#[test]
fn two_pair_no_flush() {
    // Hole AsKs (two distinct ranks); board has Ad and Kd, no 3-suit.
    let h = hole("AsKs2h3h");
    let b = board("AdKdQc5d7c");
    assert!(!flush_possible(&h, &b));
    let opt = OmahaHighRule::evaluate(&h, &b);
    assert_eq!(opt, naive(&h, &b));
}

#[test]
fn pocket_pair_with_repeated_rank_in_board() {
    // Hole 9d9c, board has another 9 → trips possible. No flush.
    let h = hole("9d9c2s3h");
    let b = board("9hQcKsJd7c");
    assert!(!flush_possible(&h, &b));
    let opt = OmahaHighRule::evaluate(&h, &b);
    assert_eq!(opt, naive(&h, &b));
}

#[test]
fn four_of_one_rank_in_hole_plus_board() {
    // Edge: 4 aces total across hole+board. Hole has 2 aces; need
    // to test that the table doesn't blow up on max-multiplicity.
    let h = hole("AcAd2c3d");
    let b = board("AhAs5d6h7c");
    assert!(!flush_possible(&h, &b));
    let opt = OmahaHighRule::evaluate(&h, &b);
    assert_eq!(opt, naive(&h, &b));
}

// --- random sweep: 10K no-flush fixtures vs naive ------------------

#[test]
fn no_flush_random_sweep_matches_naive() {
    // Same PCG-style RNG used in the bench; different seed.
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

    let mut rng = Rng(0xCAFE_F00D_DEAD_BEEF);
    let mut checked = 0usize;
    let mut tried = 0usize;
    while checked < 10_000 && tried < 200_000 {
        tried += 1;
        let mut deck: [usize; 52] = std::array::from_fn(|i| i);
        for i in 0..9 {
            let j = i + (rng.next_u64() as usize) % (52 - i);
            deck.swap(i, j);
        }
        let h = [deck[0], deck[1], deck[2], deck[3]];
        let b = [deck[4], deck[5], deck[6], deck[7], deck[8]];
        if flush_possible(&h, &b) {
            continue;
        }
        let opt = OmahaHighRule::evaluate(&h, &b);
        let nai = naive(&h, &b);
        assert_eq!(
            opt, nai,
            "mismatch on hole={:?} board={:?}: opt={} naive={}",
            h, b, opt, nai
        );
        checked += 1;
    }
    assert!(
        checked >= 5_000,
        "too few no-flush samples drawn ({checked} of {tried})"
    );
}
