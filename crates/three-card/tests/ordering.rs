//! Ordering spot-checks: the contract is "higher = stronger", so this
//! file proves that each of the documented category and tiebreak
//! boundaries comes out in the right direction.

use phe_three_card::ThreeCardRule;

fn c(rank: u8, suit: u8) -> u8 {
    rank * 4 + suit
}

#[test]
fn any_pair_beats_any_high_card() {
    // A K Q offsuit vs. 2 2 3 — the highest possible high card vs the
    // lowest possible pair.
    let high = ThreeCardRule::evaluate([c(12, 3), c(11, 1), c(10, 0)]);
    let pair = ThreeCardRule::evaluate([c(0, 0), c(0, 1), c(1, 2)]);
    assert!(high < pair);
}

#[test]
fn any_trips_beats_any_pair() {
    // A A K vs 2 2 2.
    let pair = ThreeCardRule::evaluate([c(12, 3), c(12, 1), c(11, 0)]);
    let trips = ThreeCardRule::evaluate([c(0, 0), c(0, 1), c(0, 2)]);
    assert!(pair < trips);
}

#[test]
fn trip_rank_tiebreak() {
    let aaa = ThreeCardRule::evaluate([c(12, 0), c(12, 1), c(12, 2)]);
    let kkk = ThreeCardRule::evaluate([c(11, 0), c(11, 1), c(11, 2)]);
    assert!(aaa > kkk);
}

#[test]
fn pair_kicker_tiebreak() {
    let aaq = ThreeCardRule::evaluate([c(12, 0), c(12, 1), c(10, 0)]);
    let aaj = ThreeCardRule::evaluate([c(12, 2), c(12, 3), c(9, 0)]);
    assert!(aaq > aaj);
}

#[test]
fn high_card_low_kicker_tiebreak() {
    let akq = ThreeCardRule::evaluate([c(12, 3), c(11, 1), c(10, 0)]);
    let akj = ThreeCardRule::evaluate([c(12, 3), c(11, 1), c(9, 0)]);
    assert!(akq > akj);
}

#[test]
fn high_card_middle_kicker_tiebreak() {
    // Top and bottom equal, middle differs.
    let aqt = ThreeCardRule::evaluate([c(12, 3), c(10, 1), c(8, 0)]);
    let ajt = ThreeCardRule::evaluate([c(12, 3), c(9, 1), c(8, 0)]);
    assert!(aqt > ajt);
}

#[test]
fn pair_rank_dominates_kicker() {
    // 33-A < 44-2 — higher pair, lower kicker still wins.
    let threes_with_ace = ThreeCardRule::evaluate([c(1, 0), c(1, 1), c(12, 0)]);
    let fours_with_two = ThreeCardRule::evaluate([c(2, 0), c(2, 1), c(0, 0)]);
    assert!(threes_with_ace < fours_with_two);
}

#[test]
fn category_boundaries_are_global() {
    // The minimum pair (22-3) is still ≥ the absolute maximum high card
    // (AKQ). The minimum trips (222) is still ≥ the absolute maximum
    // pair (AAK).
    let max_high = ThreeCardRule::evaluate([c(12, 0), c(11, 1), c(10, 2)]);
    let min_pair = ThreeCardRule::evaluate([c(0, 0), c(0, 1), c(1, 2)]);
    let max_pair = ThreeCardRule::evaluate([c(12, 0), c(12, 1), c(11, 0)]);
    let min_trips = ThreeCardRule::evaluate([c(0, 0), c(0, 1), c(0, 2)]);
    assert!(max_high < min_pair);
    assert!(max_pair < min_trips);
}
