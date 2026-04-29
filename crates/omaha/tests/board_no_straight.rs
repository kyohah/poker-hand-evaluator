//! Tests for `board_no_straight`.
//!
//! Returns `true` iff no 5-card straight can land in *any*
//! (hole_pair, board_triple) combo, no matter what hole the player
//! holds. Equivalent to: every 5-rank sequential window (including
//! the wheel A-2-3-4-5) contains at most 2 board ranks.

use phe_omaha::board_no_straight;

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

fn parse_5(s: &str) -> [usize; 5] {
    let mut chars = s.chars();
    let mut out = [0usize; 5];
    for slot in &mut out {
        *slot = card(chars.next().unwrap(), chars.next().unwrap());
    }
    out
}

#[test]
fn five_sequential_ranks_definitely_have_straight() {
    assert!(!board_no_straight(&parse_5("2c3d4h5s6c")));
    assert!(!board_no_straight(&parse_5("9c8d7h6s5d")));
    assert!(!board_no_straight(&parse_5("TcJdQhKsAd")));
}

#[test]
fn three_close_ranks_in_a_window_allow_straight() {
    // {4, 5, 6} all in window {2..6}, gives 3 ranks -> straight possible
    // (with hole 2,3 or 7,8 or similar).
    assert!(!board_no_straight(&parse_5("4c5d6hQsKc")));
}

#[test]
fn wheel_pattern_with_three_low_ranks_allows_straight() {
    // {A, 2, 3} all in wheel window {12, 0, 1, 2, 3} -> straight possible
    // (with hole 4, 5).
    assert!(!board_no_straight(&parse_5("Ac2d3h9sKc")));
}

#[test]
fn widely_separated_ranks_have_no_straight() {
    // {2, 3, 7, 8, Q} = ranks {0, 1, 5, 6, 10}. Every window has ≤ 2:
    //   {0..4}:{0,1}=2, {1..5}:{1,5}=2, {2..6}:{5,6}=2, ..., wheel {0,1,12}:{0,1}=2.
    assert!(board_no_straight(&parse_5("2c3d7h8sQc")));
}

#[test]
fn another_widely_separated_set_has_no_straight() {
    // {2, 7, J, K, A} = ranks {0, 5, 9, 11, 12}.
    // Wheel {0,1,2,3,12}: contains 0, 12 = 2. OK.
    // {1..5}: contains nothing (1 not in board ranks). OK.
    // {5..9}: contains 5 = 1.
    // {9..13}: contains 9, 11, 12 = 3. NOT OK — straight possible.
    // So this is NOT no_straight.
    assert!(!board_no_straight(&parse_5("2c7dJhKsAc")));
}

#[test]
fn small_gap_at_low_end_allows_straight() {
    // {2, 3, 7, J, A} = ranks {0, 1, 5, 9, 12}.
    // Wheel {0,1,2,3,12}: contains 0, 1, 12 = 3. Straight possible.
    assert!(!board_no_straight(&parse_5("2c3d7hJsAc")));
}

#[test]
fn another_no_straight_example() {
    // {2, 8, T, Q, A} = ranks {0, 6, 8, 10, 12}. Check:
    // {0..4}: {0} = 1.
    // {1..5}: {} = 0.
    // {2..6}: {6} = 1.  (wait, window {2,3,4,5,6} -> rank 6 in ranks? ranks contains 6=Yes from 8). Let me recount: ranks = {0, 6, 8, 10, 12}. window {2-6} = {2,3,4,5,6}. board ranks in {2,3,4,5,6} = {6}. = 1.
    // {3..7}: {3,4,5,6,7} ∩ board = {6}. = 1.
    // {4..8}: {4,5,6,7,8} ∩ board = {6, 8}. = 2.
    // {5..9}: {5,6,7,8,9} ∩ board = {6, 8}. = 2.
    // {6..10}: {6,7,8,9,10} ∩ board = {6, 8, 10}. = 3. STRAIGHT POSSIBLE.
    assert!(!board_no_straight(&parse_5("2c8dThQsAc")));
}

#[test]
fn another_no_straight_board() {
    // {2, 3, 8, 9, K} = ranks {0, 1, 6, 7, 11}. All windows ≤ 2:
    //   wheel {0,1,12}: {0,1} = 2.
    //   {0..4}: {0,1} = 2. {3..7}: {6,7} = 2. {6..10}: {6,7} = 2.
    //   {7..11}: {7, 11} = 2. {8..12}: {11} = 1.
    assert!(board_no_straight(&parse_5("2c3d8h9sKc")));
}

#[test]
fn maximum_3_to_3_distance_breaks_no_straight() {
    // For sorted ranks r1 < r2 < r3, if r3 - r1 < 5 then they all
    // fit in some 5-window. We use this to verify that very bunched
    // ranks always produce a straight.
    // {2, 3, 4, K, A} = ranks {0, 1, 2, 11, 12}. r3 - r1 = 2 < 5
    // → window {0..4} has 3 ranks. Straight possible.
    assert!(!board_no_straight(&parse_5("2c3d4hKsAc")));
}
