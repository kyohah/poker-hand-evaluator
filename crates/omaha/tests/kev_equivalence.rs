//! Cross-check `evaluate_kev` against the production
//! `OmahaHighRule::evaluate` on random fixtures + structural corner
//! cases. Both evaluators must produce identical packed u16 for every
//! input.

use phe_omaha::{evaluate_kev, OmahaHighRule};

/// Linear-congruential PRNG seeded for reproducibility.
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
fn matches_production_on_100k_random_hands() {
    let mut rng = Rng::new(0x0123_4567_89AB_CDEF);
    for _ in 0..100_000 {
        let (hole, board) = random_hand(&mut rng);
        let prod = OmahaHighRule::evaluate(&hole, &board);
        let kev = evaluate_kev(&hole, &board);
        assert_eq!(
            kev, prod,
            "mismatch for hole={:?} board={:?}: kev={:#x} prod={:#x}",
            hole, board, kev, prod
        );
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
fn matches_production_on_structural_corner_cases() {
    let cases = [
        // (label, hole, board) — sweep the existing fast-path scenarios.
        ("rainbow board", hole("As2c3d4h"), board("Kc7d6h5s9c")),
        (
            "3-suit board, 2-suit hole",
            hole("AhKh2c3c"),
            board("4h5h6h7d8d"),
        ),
        (
            "monotone hearts board",
            hole("AhKh2c3c"),
            board("4h3h5h8h9h"),
        ),
        (
            "monotone hearts, no hearts in hole",
            hole("AsKs2c3c"),
            board("4h3h5h8h9h"),
        ),
        (
            "4-of-a-kind on board",
            hole("AsAd2c3c"),
            board("AhAcKsQsJs"),
        ),
        (
            "paired board, suited hole",
            hole("AhKhQhJh"),
            board("AsAdAcKsKc"),
        ),
        ("straight playable", hole("5c6d2s3s"), board("7h8s9cKdJh")),
        (
            "royal flush on board, no hearts in hole",
            hole("As2c3c4c"),
            board("AhKhQhJhTh"),
        ),
        (
            "royal flush playable",
            hole("AhKh2c3c"),
            board("QhJhTh4d5d"),
        ),
        (
            "user pattern monotone",
            hole("AsKs2c3c"),
            board("4h3h5h8h9h"),
        ),
        (
            "widely-separated no-straight board",
            hole("As4d6c8s"),
            board("2c3d7h8sQc"),
        ),
    ];

    for (label, h, b) in cases {
        let prod = OmahaHighRule::evaluate(&h, &b);
        let kev = evaluate_kev(&h, &b);
        assert_eq!(
            kev, prod,
            "mismatch on '{}': hole={:?} board={:?} kev={:#x} prod={:#x}",
            label, h, b, kev, prod
        );
    }
}
