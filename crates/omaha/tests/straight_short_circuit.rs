//! Cross-check `evaluate_straight_short_circuit` against
//! `OmahaHighRule::evaluate` on 100K random hands plus a sweep of
//! "straight is the max" hand-shapes. The short-circuit returns the
//! Straight packed rank for hands that hit the fast path and falls
//! back to the production eval otherwise; both paths must agree.

use phe_omaha::{evaluate_straight_short_circuit, OmahaHighRule};

struct Rng(u64);
impl Rng {
    fn new(seed: u64) -> Self {
        Self(seed)
    }
    fn next_u64(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0
    }
}

fn random_hand(rng: &mut Rng) -> ([usize; 4], [usize; 5]) {
    let mut deck: [usize; 52] = std::array::from_fn(|i| i);
    for i in 0..9 {
        let j = i + (rng.next_u64() as usize) % (52 - i);
        deck.swap(i, j);
    }
    (
        [deck[0], deck[1], deck[2], deck[3]],
        [deck[4], deck[5], deck[6], deck[7], deck[8]],
    )
}

#[test]
fn matches_production_on_100k_random() {
    let mut rng = Rng::new(0x90AB_CDEF_1234_5678);
    for _ in 0..100_000 {
        let (h, b) = random_hand(&mut rng);
        let prod = OmahaHighRule::evaluate(&h, &b);
        let ssc = evaluate_straight_short_circuit(&h, &b);
        assert_eq!(prod, ssc, "mismatch at hole={:?} board={:?}", h, b);
    }
}

fn cards(s: &str) -> Vec<usize> {
    let mut chars = s.chars();
    let mut out = Vec::new();
    while let Some(r) = chars.next() {
        let suit = chars.next().unwrap();
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
        let s = match suit.to_ascii_lowercase() {
            'c' => 0,
            'd' => 1,
            'h' => 2,
            's' => 3,
            _ => panic!(),
        };
        out.push(rank * 4 + s);
    }
    out
}
fn hole(s: &str) -> [usize; 4] {
    let v = cards(s);
    [v[0], v[1], v[2], v[3]]
}
fn board(s: &str) -> [usize; 5] {
    let v = cards(s);
    [v[0], v[1], v[2], v[3], v[4]]
}

#[test]
fn straight_is_returned_when_fast_path_fires() {
    // Board has 4-5-6 (and two unrelated high cards), no pair, no flush
    // eligibility. Hole 7-8 completes 4-5-6-7-8 (8-high straight).
    let h = hole("7c8d2c3c"); // 7c, 8d (mixed), filler 2c, 3c
    let b = board("4h5d6cKsAd"); // mixed suits, no pair, has 4-5-6
    let prod = OmahaHighRule::evaluate(&h, &b);
    let ssc = evaluate_straight_short_circuit(&h, &b);
    assert_eq!(prod, ssc);
    // Top of straight is 8 (rank 6); packed = (4 << 12) | (6 - 3) = 0x4003
    let cat = (ssc >> 12) as u8;
    assert_eq!(cat, 4, "expected Straight category, got {}", cat);
}

#[test]
fn wheel_straight_via_short_circuit() {
    // Board 3-4-5 + filler. Hole A-2 (with no overlap). Wheel 5-high.
    let h = hole("Ac2d8c9c");
    let b = board("3h4d5sKcQd"); // no pair, no flush, has 3-4-5
    let prod = OmahaHighRule::evaluate(&h, &b);
    let ssc = evaluate_straight_short_circuit(&h, &b);
    assert_eq!(prod, ssc);
}

#[test]
fn flush_eligible_falls_through_to_production() {
    // Hole 2 hearts + board 3 hearts → flush eligible. Short circuit
    // mustn't fire; result must come from OmahaHighRule.
    let h = hole("AhKh2c3c");
    let b = board("4h3h5h8h9h");
    let prod = OmahaHighRule::evaluate(&h, &b);
    let ssc = evaluate_straight_short_circuit(&h, &b);
    assert_eq!(prod, ssc);
}

#[test]
fn paired_board_falls_through() {
    // Board with pair → FH/Quads possible, can't short-circuit on
    // straight even if straight present.
    let h = hole("7c8d2c3c");
    let b = board("4h4d5d6cAd"); // 4-pair on board
    let prod = OmahaHighRule::evaluate(&h, &b);
    let ssc = evaluate_straight_short_circuit(&h, &b);
    assert_eq!(prod, ssc);
}

#[test]
fn no_straight_falls_through() {
    // Board with widely-separated ranks; no straight reachable.
    let h = hole("Ac4d6c8s");
    let b = board("2c3d7h8sQc");
    let prod = OmahaHighRule::evaluate(&h, &b);
    let ssc = evaluate_straight_short_circuit(&h, &b);
    assert_eq!(prod, ssc);
}
