//! `BadugiStrength` ordering invariants.
//!
//! `Strength` ordering: count first (4 > 3 > 2 > 1), then within the
//! same count, smaller largest-card wins, then smaller next, etc.

use phe_badugi::{BadugiRule, BadugiStrength};

/// Cards are Hold'em-encoded: `card = rank * 4 + suit`,
/// rank `0='2', ..., 12='A'`, suit `0=c, 1=d, 2=h, 3=s`.
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
        _ => panic!("bad rank {}", rank),
    };
    let s = match suit {
        'c' => 0,
        'd' => 1,
        'h' => 2,
        's' => 3,
        _ => panic!("bad suit {}", suit),
    };
    r * 4 + s
}

fn parse_4(text: &str) -> [u8; 4] {
    let bytes = text.as_bytes();
    assert_eq!(bytes.len(), 8, "expected 4 cards (8 chars), got {:?}", text);
    [
        card(bytes[0] as char, bytes[1] as char),
        card(bytes[2] as char, bytes[3] as char),
        card(bytes[4] as char, bytes[5] as char),
        card(bytes[6] as char, bytes[7] as char),
    ]
}

fn eval(s: &str) -> BadugiStrength {
    BadugiRule::evaluate(parse_4(s))
}

#[test]
fn wheel_is_a_four_card_badugi() {
    let wheel = eval("Ac2d3h4s");
    assert_eq!(wheel.count(), 4);
}

#[test]
fn wheel_is_the_nuts() {
    let wheel = eval("Ac2d3h4s");
    let next_best_4 = eval("Ac2d3h5s");
    assert!(wheel > next_best_4);
}

#[test]
fn any_four_card_badugi_beats_any_three_card_badugi() {
    let four_card_worst = eval("AcKdQhJs"); // 4 different ranks, 4 different suits
                                            // 3-card best: A♣ 2♦ 3♥ (with one suit-duplicate forcing a drop).
                                            // Construct: A♣ 2♦ 3♥ + (something that pairs the top with one of these
                                            // by either rank or suit). 3♣ pairs suit with A♣ → forced 3-badugi.
    let three_card_best = eval("Ac2d3h3c");
    assert!(four_card_worst > three_card_best);
}

#[test]
fn three_card_beats_two_card() {
    let three = eval("Ac2d3h3c"); // 3-badugi
    let two = eval("Ac2c3d3h"); // 2-badugi (forced: A♣2♣ same suit, 3♦3♥ same rank)
    assert!(three > two);
}

#[test]
fn within_same_count_lower_top_card_wins() {
    let a234 = eval("Ac2d3h4s");
    let a235 = eval("Ac2d3h5s");
    let a236 = eval("Ac2d3h6s");
    assert!(a234 > a235);
    assert!(a235 > a236);
}

#[test]
fn within_same_count_low_kicker_breaks_tie() {
    // Same top (4), but A-2-3-4 vs 2-3-4 + something with top 4.
    // Hmm, both must be 4-badugi. Let me do A-2-3-4 vs A-3-4 + one with top 4.
    // Actually simpler: 4-badugi A-2-3-4 vs 4-badugi A-2-3-4 itself trivially equal.
    // Tie-break case: A-2-4-K vs A-3-4-K — both 4-badugi with top K.
    // descending sorted ranks (Badugi rank, A=0): A-2-4-K = [12, 3, 1, 0]; tiebreak = 0xC310
    //                                                A-3-4-K = [12, 3, 2, 0]; tiebreak = 0xC320
    // A-2-4-K should beat A-3-4-K (smaller second-largest wins).
    let a24k = eval("Ac2d4hKs");
    let a34k = eval("Ac3d4hKs");
    assert!(a24k > a34k);
}

#[test]
fn quad_aces_collapses_to_one_card() {
    let aaaa = eval("AcAdAhAs");
    assert_eq!(aaaa.count(), 1);
}

#[test]
fn single_suit_collapses_to_one_card() {
    let one_suit = eval("AcKcQcJc");
    assert_eq!(one_suit.count(), 1);
}

#[test]
fn one_card_ace_is_better_than_one_card_king() {
    let aaaa = eval("AcAdAhAs"); // 1-badugi at A
    let kkkk = eval("KcKdKhKs"); // 1-badugi at K
    assert!(aaaa > kkkk);
}

#[test]
fn ord_is_total_and_strength_is_copy() {
    fn requires_copy<T: Copy>() {}
    fn requires_ord<T: Ord>() {}
    requires_copy::<BadugiStrength>();
    requires_ord::<BadugiStrength>();
}
