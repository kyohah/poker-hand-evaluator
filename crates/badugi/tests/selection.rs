//! Best-subset selection tests.
//!
//! When ranks or suits collide, Badugi forces dropping the higher card
//! (so the remaining subset has all-distinct ranks AND all-distinct
//! suits, with the smallest possible top card). These tests pin down
//! the selection logic for non-trivial collision cases.

use phe_badugi::BadugiRule;

fn card(rank: char, suit: char) -> u8 {
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

fn parse_4(text: &str) -> [u8; 4] {
    let b = text.as_bytes();
    [
        card(b[0] as char, b[1] as char),
        card(b[2] as char, b[3] as char),
        card(b[4] as char, b[5] as char),
        card(b[6] as char, b[7] as char),
    ]
}

#[test]
fn two_suit_pairs_force_two_card_subset() {
    // A♠ 2♠ 3♥ 4♥: every 3-card subset has two of one suit. Forced 2-badugi.
    // Best 2-badugi: choose A♠ + 3♥ (lowest available with distinct suit).
    let s = BadugiRule::evaluate(parse_4("As2s3h4h"));
    assert_eq!(s.count(), 2);
    // The best 2-badugi from this hand is A and 3 (Badugi ranks 0 and 2).
    // Beating any worse 2-badugi like 2-3 (badugi ranks 1, 2):
    let worse = BadugiRule::evaluate(parse_4("2s5s3h6h"));
    // From 2♠5♠3♥6♥, best 2-badugi is 2♠ + 3♥ (or 2♠ + 6♥, etc.).
    // Best 2-badugi: 2-3 (ranks 1,2). Should be worse than A-3 (ranks 0,2).
    assert!(s > worse);
}

#[test]
fn one_suit_collision_drops_to_three_badugi() {
    // A♣ 2♦ 3♥ 3♣: 3♣ collides with A♣ on suit. Drop 3♣ (since it's
    // the higher card on the offending suit), keep A♣ 2♦ 3♥ → 3-badugi.
    let s = BadugiRule::evaluate(parse_4("Ac2d3h3c"));
    assert_eq!(s.count(), 3);
}

#[test]
fn rank_pair_drops_one_to_three_badugi() {
    // A♣ A♦ 2♥ 3♠: rank duplicates A♣/A♦. Drop one ace.
    // Result: 3-badugi A-2-3 (any single ace + 2♥ + 3♠).
    let s = BadugiRule::evaluate(parse_4("AcAd2h3s"));
    assert_eq!(s.count(), 3);
}

#[test]
fn must_drop_higher_when_choice_exists() {
    // A♣ 2♣ 3♥ K♠: A and 2 share suit clubs.
    // 3-badugi options: drop 2 → A-3-K (ranks 0,2,12), or drop A → 2-3-K (1,2,12).
    // Lower top-3 wins: both have top=K. Next: 3 vs 3 (same). Next: 0 vs 1.
    // So A-3-K wins (drops 2♣, keeps A♣).
    let cards = parse_4("Ac2c3hKs");
    let s = BadugiRule::evaluate(cards);
    assert_eq!(s.count(), 3);

    // Cross-check via comparison: 2-3-K from the SAME flush-suit setup
    // — different hand: A♦ 2♣ 3♥ K♠ doesn't have a club collision,
    // so use B♦ ... let's compare against an actual 2-3-K 3-badugi:
    // 2♣ 3♥ K♠ + a card that forces dropping. e.g. 2♣ 4♣ 3♥ K♠:
    //   collision: 2♣/4♣ share suit. 3-badugi options: drop 4 -> 2-3-K, drop 2 -> 4-3-K.
    //   2-3-K wins (lower).
    let other = BadugiRule::evaluate(parse_4("2c4c3hKs"));
    // Ours: A-3-K. Other: 2-3-K. A-3-K should beat 2-3-K (lower top-tied,
    // next tied 3, then 0 < 1).
    assert!(s > other);
}

#[test]
fn already_a_badugi_returns_four_count() {
    let s = BadugiRule::evaluate(parse_4("Ac2d3h4s"));
    assert_eq!(s.count(), 4);
}
