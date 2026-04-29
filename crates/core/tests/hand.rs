//! Behavioral tests for `phe_core::Hand`.
//!
//! These tests pin down the contract before the implementation lands. They
//! cover only the lookup-table-independent surface — actual hand-rank
//! correctness is exercised by the variant crates that ship lookup tables
//! (phe-holdem, phe-deuce-seven, phe-eight-low).

use phe_core::{Hand, NUMBER_OF_CARDS};

#[test]
fn new_hand_is_empty() {
    let h = Hand::new();
    assert!(h.is_empty());
    assert_eq!(h.len(), 0);
    assert_eq!(h.get_mask(), 0);
}

#[test]
fn default_matches_new() {
    assert_eq!(Hand::default(), Hand::new());
}

#[test]
fn add_card_increases_length_and_sets_mask_bit() {
    let h = Hand::new().add_card(0);
    assert_eq!(h.len(), 1);
    assert!(!h.is_empty());
    assert!(h.contains(0));
    assert!(!h.contains(1));
    // exactly one bit set in the mask
    assert_eq!(h.get_mask().count_ones(), 1);
}

#[test]
fn add_then_remove_yields_empty() {
    let h = Hand::new()
        .add_card(7)
        .add_card(13)
        .remove_card(13)
        .remove_card(7);
    assert!(h.is_empty());
    assert_eq!(h, Hand::new());
}

#[test]
fn from_slice_collects_all_cards() {
    let cards = [0usize, 5, 17, 31, 49];
    let h = Hand::from_slice(&cards);
    assert_eq!(h.len(), cards.len());
    for c in cards {
        assert!(h.contains(c));
    }
}

#[test]
fn add_operator_combines_disjoint_hands() {
    let a = Hand::from_slice(&[0, 1, 2]);
    let b = Hand::from_slice(&[10, 20, 30]);
    let c = a + b;
    assert_eq!(c.len(), 6);
    for x in [0, 1, 2, 10, 20, 30] {
        assert!(c.contains(x));
    }
    // associativity: a + b == b + a (commutative on disjoint sets)
    assert_eq!(a + b, b + a);
}

#[test]
fn add_assign_matches_add() {
    let a = Hand::from_slice(&[3, 4]);
    let b = Hand::from_slice(&[40, 41]);
    let mut acc = a;
    acc += b;
    assert_eq!(acc, a + b);
}

#[test]
fn add_then_subtract_via_remove_round_trips() {
    let board = Hand::from_slice(&[10, 11, 12, 13, 14]);
    let hole = Hand::from_slice(&[20, 21]);
    let combined = board + hole;
    assert_eq!(combined.len(), 7);
    let stripped = combined.remove_card(20).remove_card(21);
    assert_eq!(stripped, board);
}

#[test]
fn number_of_cards_constant_is_52() {
    assert_eq!(NUMBER_OF_CARDS, 52);
}
