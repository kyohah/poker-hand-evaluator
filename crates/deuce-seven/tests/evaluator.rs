//! 2-7 lowball evaluator tests.
//!
//! Coverage:
//!   - Concrete edge cases for wheel-as-no-pair, broadway-still-straight,
//!     and the canonical 7-5-4-3-2 nuts.
//!   - Strength ordering (smaller raw = stronger 2-7, Reverse-wrapped).
//!   - Exhaustive 5-card cross-validation vs. the naive reference used
//!     to generate the lookup table.

use phe_core::{Hand, NUMBER_OF_CARDS};
use phe_deuce_seven::{parse_hand, DeuceSevenLowRule, HandCategory};
use phe_holdem::get_hand_category as holdem_category;
use phe_scripts::naive_high::{eval_5cards, WheelMode};
use std::cmp::Reverse;

fn h(s: &str) -> Hand {
    parse_hand(s).unwrap()
}

#[test]
fn nuts_is_seven_five_four_three_two() {
    let nuts = DeuceSevenLowRule::evaluate(&h("7s5h4d3c2s"));
    // Nothing should beat this.
    let other_strong = DeuceSevenLowRule::evaluate(&h("8s5h4d3c2s"));
    assert!(
        nuts > other_strong,
        "nuts {:?} should beat 8-low {:?}",
        nuts,
        other_strong
    );
}

#[test]
fn wheel_is_not_a_straight() {
    // A-2-3-4-5 mixed suits — must land in HighCard, not Straight.
    let r = DeuceSevenLowRule::evaluate(&h("As2d3h4c5s"));
    let raw = r.0;
    let cat = (raw >> 12) as u8;
    assert_eq!(
        cat,
        HandCategory::HighCard as u8,
        "wheel must be HighCard in 2-7, got category {} (raw {})",
        cat,
        raw
    );
}

#[test]
fn broadway_remains_a_straight() {
    let r = DeuceSevenLowRule::evaluate(&h("AsKsQhJdTc"));
    let cat = (r.0 >> 12) as u8;
    assert_eq!(cat, HandCategory::Straight as u8);
}

#[test]
fn ace_high_no_pair_beats_pair() {
    // In 2-7, any no-pair beats any pair (pair is "worse Hold'em" but
    // "category Pair > category HighCard" → pair is stronger as Hold'em
    // → weaker as 2-7 → loses).
    let a_high_no_pair = DeuceSevenLowRule::evaluate(&h("As2d3h4c5s"));
    let pair = DeuceSevenLowRule::evaluate(&h("2s2d3h4c5s"));
    assert!(a_high_no_pair > pair);
}

#[test]
fn flush_is_bad() {
    // 7s5s4s3s2s = 5-card flush (best Hold'em "flush 7-high" but worst
    // as a 2-7 hand at this rank set).
    let flush = DeuceSevenLowRule::evaluate(&h("7s5s4s3s2s"));
    let no_flush = DeuceSevenLowRule::evaluate(&h("7s5h4d3c2s"));
    assert!(
        no_flush > flush,
        "no-flush should win, got {:?} vs {:?}",
        no_flush,
        flush
    );
}

#[test]
fn six_or_seven_card_eval_panics() {
    let h6 = h("As2d3h4c5s6d");
    let r = std::panic::catch_unwind(|| DeuceSevenLowRule::evaluate(&h6));
    assert!(
        r.is_err(),
        "DeuceSevenLowRule::evaluate must panic on 6-card hands"
    );
}

/// Exhaustive C(52,5) cross-validation against the naive evaluator that
/// generated the lookup table. Confirms every 5-card hand evaluates to
/// the strength implied by `WheelMode::NoPair`.
#[test]
fn all_5card_cross_validation() {
    use std::collections::BTreeMap;

    // Build the same compactification map the generator uses, in-process.
    // (Cheap: 2.6M iterations.)
    let mut by_category: BTreeMap<u8, std::collections::BTreeSet<u32>> = BTreeMap::new();
    for i in 0..(NUMBER_OF_CARDS - 4) {
        for j in (i + 1)..(NUMBER_OF_CARDS - 3) {
            for k in (j + 1)..(NUMBER_OF_CARDS - 2) {
                for m in (k + 1)..(NUMBER_OF_CARDS - 1) {
                    for n in (m + 1)..NUMBER_OF_CARDS {
                        let v = eval_5cards(&[i, j, k, m, n], WheelMode::NoPair);
                        by_category.entry((v >> 26) as u8).or_default().insert(v);
                    }
                }
            }
        }
    }
    let mut compact = std::collections::HashMap::new();
    for (cat, ranks) in &by_category {
        for (idx, r) in ranks.iter().enumerate() {
            compact.insert(*r, ((*cat as u16) << 12) | (idx as u16));
        }
    }

    // Now sweep every 5-card hand and verify the fast eval matches.
    for i in 0..(NUMBER_OF_CARDS - 4) {
        let h0 = Hand::new().add_card(i);
        for j in (i + 1)..(NUMBER_OF_CARDS - 3) {
            let h1 = h0.add_card(j);
            for k in (j + 1)..(NUMBER_OF_CARDS - 2) {
                let h2 = h1.add_card(k);
                for m in (k + 1)..(NUMBER_OF_CARDS - 1) {
                    let h3 = h2.add_card(m);
                    for n in (m + 1)..NUMBER_OF_CARDS {
                        let h4 = h3.add_card(n);
                        let raw = eval_5cards(&[i, j, k, m, n], WheelMode::NoPair);
                        let expected = Reverse(compact[&raw]);
                        let got = DeuceSevenLowRule::evaluate(&h4);
                        assert_eq!(got, expected, "mismatch on cards [{i},{j},{k},{m},{n}]");
                    }
                }
            }
        }
    }
}

/// Counts of distinct ranks per Hold'em category in the 2-7 lookup —
/// the wheel/wheel-SF should have shifted from Straight/SF into
/// HighCard/Flush, leaving 9 straights and 9 SFs (vs. 10/10 in Hold'em).
#[test]
fn category_distinct_rank_counts() {
    use std::collections::HashSet;

    let mut by_cat: [HashSet<u16>; 9] = Default::default();

    for i in 0..(NUMBER_OF_CARDS - 4) {
        let h0 = Hand::new().add_card(i);
        for j in (i + 1)..(NUMBER_OF_CARDS - 3) {
            let h1 = h0.add_card(j);
            for k in (j + 1)..(NUMBER_OF_CARDS - 2) {
                let h2 = h1.add_card(k);
                for m in (k + 1)..(NUMBER_OF_CARDS - 1) {
                    let h3 = h2.add_card(m);
                    for n in (m + 1)..NUMBER_OF_CARDS {
                        let h4 = h3.add_card(n);
                        let r = DeuceSevenLowRule::evaluate(&h4).0;
                        let cat = holdem_category(r) as usize;
                        by_cat[cat].insert(r);
                    }
                }
            }
        }
    }

    assert_eq!(by_cat[HandCategory::HighCard as usize].len(), 1278);
    assert_eq!(by_cat[HandCategory::OnePair as usize].len(), 2860);
    assert_eq!(by_cat[HandCategory::TwoPair as usize].len(), 858);
    assert_eq!(by_cat[HandCategory::ThreeOfAKind as usize].len(), 858);
    assert_eq!(by_cat[HandCategory::Straight as usize].len(), 9);
    assert_eq!(by_cat[HandCategory::Flush as usize].len(), 1278);
    assert_eq!(by_cat[HandCategory::FullHouse as usize].len(), 156);
    assert_eq!(by_cat[HandCategory::FourOfAKind as usize].len(), 156);
    assert_eq!(by_cat[HandCategory::StraightFlush as usize].len(), 9);
}
