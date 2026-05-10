//! Fixture-driven category and packing checks, plus the exhaustive
//! permutation-invariance property test.

use phe_three_card::ThreeCardRule;

fn c(rank: u8, suit: u8) -> u8 {
    rank * 4 + suit
}

fn category(s: u16) -> u16 {
    s >> 12
}

fn within(s: u16) -> u16 {
    s & 0xfff
}

#[test]
fn high_card_category_bits() {
    // Q♣ 9♦ 3♠ — all distinct ranks.
    let s = ThreeCardRule::evaluate([c(10, 0), c(7, 1), c(1, 3)]);
    assert_eq!(category(s), 0);
}

#[test]
fn pair_category_bits_low_pair() {
    // 2♣ 2♦ K♣ — pair of deuces.
    let s = ThreeCardRule::evaluate([c(0, 0), c(0, 1), c(11, 0)]);
    assert_eq!(category(s), 1);
}

#[test]
fn trips_category_bits() {
    // 7♣ 7♦ 7♥
    let s = ThreeCardRule::evaluate([c(5, 0), c(5, 1), c(5, 2)]);
    assert_eq!(category(s), 2);
}

#[test]
fn high_card_packing_descending() {
    // c(rank, ..) uses Hold'em-encoded rank, so c(4,..) is a 6.
    // 6♣ A♠ 9♦ → top=A=12, mid=9=7, low=6=4
    let s = ThreeCardRule::evaluate([c(4, 0), c(12, 3), c(7, 1)]);
    assert_eq!(within(s), (12 << 8) | (7 << 4) | 4);
}

#[test]
fn pair_packing_high_pair_with_kicker() {
    // K♠ K♦ 5♣ → pair_rank=11, kicker=3
    let s = ThreeCardRule::evaluate([c(11, 3), c(11, 1), c(3, 0)]);
    assert_eq!(within(s), (11 << 4) | 3);
}

#[test]
fn trips_packing_uses_only_low_nibble() {
    let s = ThreeCardRule::evaluate([c(8, 0), c(8, 1), c(8, 2)]);
    assert_eq!(within(s), 8);
}

#[test]
fn suit_is_ignored() {
    // Same ranks, different suit assignment → identical strength.
    let a = ThreeCardRule::evaluate([c(12, 0), c(11, 1), c(10, 2)]);
    let b = ThreeCardRule::evaluate([c(12, 3), c(11, 2), c(10, 1)]);
    assert_eq!(a, b);
}

#[test]
fn exhaustive_permutation_invariance_distinct_cards() {
    // For every unordered triple of distinct cards in the 52-card deck,
    // all 6 orderings must yield the same strength. 22 100 triples →
    // 132 600 evaluations; well under a second in debug builds.
    let perms: [[usize; 3]; 6] = [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ];

    for a in 0..52u8 {
        for b in (a + 1)..52u8 {
            for d in (b + 1)..52u8 {
                let cards = [a, b, d];
                let expected = ThreeCardRule::evaluate(cards);
                for p in &perms {
                    let permuted = [cards[p[0]], cards[p[1]], cards[p[2]]];
                    let got = ThreeCardRule::evaluate(permuted);
                    assert_eq!(
                        got, expected,
                        "permutation {:?} of {:?} gave {} != {}",
                        p, cards, got, expected
                    );
                }
            }
        }
    }
}
