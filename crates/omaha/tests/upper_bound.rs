//! Tests for `upper_bound_category` (per-hole-pair max-category bound).
//!
//! The branch-and-bound prune in `evaluate` relies on a cheap upper
//! bound for each hole pair: if a pair's upper-bound category is
//! strictly less than the running best's category, the pair cannot
//! contribute and is skipped. The bound must be **safe** — it may
//! over-estimate but must never under-estimate the true max.

use phe_omaha::upper_bound_category;

const fn card(rank: u8, suit: u8) -> usize {
    (rank as usize) * 4 + (suit as usize)
}
// Hold'em-style ranks: 0='2', ..., 12='A'. Suits: 0=c, 1=d, 2=h, 3=s.

// --- SF / Flush bound --------------------------------------------------

#[test]
fn suited_pair_with_three_board_in_suit_yields_sf_upper() {
    let suits = [3u8, 0, 0, 0]; // 3 clubs on board
    let h1 = card(12, 0); // Ac
    let h2 = card(11, 0); // Kc
    assert_eq!(upper_bound_category(h1, h2, &suits, false, false), 8);
}

#[test]
fn suited_pair_only_two_in_suit_falls_back_below_flush() {
    let suits = [2u8, 1, 1, 1];
    let h1 = card(12, 0);
    let h2 = card(11, 0); // suited but board only has 2 in this suit
    let ub = upper_bound_category(h1, h2, &suits, false, false);
    assert!(ub < 5, "should not promise Flush+, got {}", ub);
}

// --- Pocket pair (same rank) -------------------------------------------

#[test]
fn pocket_pair_with_board_pair_yields_quads_upper() {
    let suits = [1u8, 1, 1, 2];
    let h1 = card(12, 0); // Ac
    let h2 = card(12, 1); // Ad — pocket aces
    assert_eq!(upper_bound_category(h1, h2, &suits, true, false), 7);
}

#[test]
fn pocket_pair_no_board_pair_yields_trips_upper() {
    let suits = [1u8, 1, 2, 1];
    let h1 = card(12, 0);
    let h2 = card(12, 1); // pocket aces, board no pair
    assert_eq!(upper_bound_category(h1, h2, &suits, false, false), 3);
}

// --- Mixed pair on paired board ----------------------------------------

#[test]
fn mixed_pair_with_board_pair_yields_fh_upper() {
    let suits = [1u8, 1, 2, 1];
    let h1 = card(12, 0); // Ac
    let h2 = card(11, 1); // Kd  (mixed)
    assert_eq!(upper_bound_category(h1, h2, &suits, true, false), 6);
}

// --- Mixed pair on no-pair board ---------------------------------------

#[test]
fn mixed_pair_no_pair_no_straight_yields_two_pair_upper() {
    let suits = [1u8, 1, 2, 1];
    let h1 = card(12, 0);
    let h2 = card(11, 1);
    assert_eq!(upper_bound_category(h1, h2, &suits, false, true), 2);
}

#[test]
fn mixed_pair_no_pair_straight_possible_yields_straight_upper() {
    let suits = [1u8, 1, 2, 1];
    let h1 = card(12, 0);
    let h2 = card(11, 1);
    assert_eq!(upper_bound_category(h1, h2, &suits, false, false), 4);
}

// --- Suited mixed-rank pair without flush eligibility ------------------

#[test]
fn suited_mixed_rank_with_board_pair_but_no_flush_uses_fh_path() {
    // Suited (clubs) but board has only 2 clubs → no flush.
    // Board has a pair → FH possible from mixed-rank perspective.
    let suits = [2u8, 2, 0, 1];
    let h1 = card(12, 0); // Ac
    let h2 = card(11, 0); // Kc — suited but no flush eligibility
    let ub = upper_bound_category(h1, h2, &suits, true, false);
    assert_eq!(ub, 6);
}

// --- Bound safety: must NEVER under-estimate ----------------------------

#[test]
fn bound_is_at_least_eight_when_sf_is_actually_possible() {
    // Constructively pick a configuration where SF is reachable to
    // confirm the bound covers the cat-8 case.
    let suits = [4u8, 0, 0, 0]; // 4 clubs on board
    let h1 = card(8, 0); // Tc
    let h2 = card(9, 0); // Jc — suited high, board has 4 clubs
    assert_eq!(upper_bound_category(h1, h2, &suits, false, false), 8);
}
