//! Hand-string parser tests (Hold'em high encoding: rank 0='2', 12='A').

use phe_core::Hand;
use phe_holdem::parse_hand;

#[test]
fn parses_seven_card_hand() {
    let cards = [2usize, 3, 5, 7, 11, 13, 17];
    let from_vec = Hand::from_slice(&cards);
    let from_str = parse_hand("2h2s3d3s4s5d6d").unwrap();
    assert_eq!(from_str, from_vec);
}

#[test]
fn empty_string_yields_empty_hand() {
    assert_eq!(parse_hand("").unwrap(), Hand::new());
}

#[test]
fn missing_suit_errors() {
    let err = parse_hand("A").unwrap_err();
    assert!(err.contains("expected suit character"), "got: {err}");
}

#[test]
fn invalid_suit_errors() {
    let err = parse_hand("Ax").unwrap_err();
    assert!(err.contains("expected suit character"), "got: {err}");
}

#[test]
fn ten_must_be_T_not_10() {
    let err = parse_hand("10s").unwrap_err();
    assert!(err.contains("expected rank character"), "got: {err}");
}
