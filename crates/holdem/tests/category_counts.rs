//! Exhaustive 5-, 6-, and 7-card category-count validation.
//!
//! These match published combinatorial counts and serve as the
//! end-to-end correctness gate for the lookup tables. Run with
//! `--release` — debug mode is too slow.

use phe_core::{Hand, NUMBER_OF_CARDS};
use phe_holdem::{get_hand_category, HandCategory, HighRule};
use std::collections::HashSet;

#[test]
fn all_5card_combinations() {
    let mut rankset = HashSet::new();
    let mut counter = vec![0; HandCategory::StraightFlush as usize + 1];

    for i in 0..(NUMBER_OF_CARDS - 4) {
        let h = Hand::new().add_card(i);
        for j in (i + 1)..(NUMBER_OF_CARDS - 3) {
            let h = h.add_card(j);
            for k in (j + 1)..(NUMBER_OF_CARDS - 2) {
                let h = h.add_card(k);
                for m in (k + 1)..(NUMBER_OF_CARDS - 1) {
                    let h = h.add_card(m);
                    for n in (m + 1)..NUMBER_OF_CARDS {
                        let h = h.add_card(n);
                        let r = HighRule::evaluate(&h);
                        rankset.insert(r);
                        counter[get_hand_category(r) as usize] += 1;
                    }
                }
            }
        }
    }

    assert_eq!(rankset.len(), 7462);
    assert_eq!(counter[HandCategory::StraightFlush as usize], 40);
    assert_eq!(counter[HandCategory::FourOfAKind as usize], 624);
    assert_eq!(counter[HandCategory::FullHouse as usize], 3744);
    assert_eq!(counter[HandCategory::Flush as usize], 5108);
    assert_eq!(counter[HandCategory::Straight as usize], 10200);
    assert_eq!(counter[HandCategory::ThreeOfAKind as usize], 54912);
    assert_eq!(counter[HandCategory::TwoPair as usize], 123552);
    assert_eq!(counter[HandCategory::OnePair as usize], 1098240);
    assert_eq!(counter[HandCategory::HighCard as usize], 1302540);
}

#[test]
fn all_6card_combinations() {
    let mut rankset = HashSet::new();
    let mut counter = vec![0; HandCategory::StraightFlush as usize + 1];

    for i in 0..(NUMBER_OF_CARDS - 5) {
        let h = Hand::new().add_card(i);
        for j in (i + 1)..(NUMBER_OF_CARDS - 4) {
            let h = h.add_card(j);
            for k in (j + 1)..(NUMBER_OF_CARDS - 3) {
                let h = h.add_card(k);
                for m in (k + 1)..(NUMBER_OF_CARDS - 2) {
                    let h = h.add_card(m);
                    for n in (m + 1)..(NUMBER_OF_CARDS - 1) {
                        let h = h.add_card(n);
                        for p in (n + 1)..NUMBER_OF_CARDS {
                            let h = h.add_card(p);
                            let r = HighRule::evaluate(&h);
                            rankset.insert(r);
                            counter[get_hand_category(r) as usize] += 1;
                        }
                    }
                }
            }
        }
    }

    assert_eq!(rankset.len(), 6075);
    assert_eq!(counter[HandCategory::StraightFlush as usize], 1844);
    assert_eq!(counter[HandCategory::FourOfAKind as usize], 14664);
    assert_eq!(counter[HandCategory::FullHouse as usize], 165984);
    assert_eq!(counter[HandCategory::Flush as usize], 205792);
    assert_eq!(counter[HandCategory::Straight as usize], 361620);
    assert_eq!(counter[HandCategory::ThreeOfAKind as usize], 732160);
    assert_eq!(counter[HandCategory::TwoPair as usize], 2532816);
    assert_eq!(counter[HandCategory::OnePair as usize], 9730740);
    assert_eq!(counter[HandCategory::HighCard as usize], 6612900);
}

#[test]
fn all_7card_combinations() {
    let mut rankset = HashSet::new();
    let mut counter = vec![0u64; HandCategory::StraightFlush as usize + 1];

    for i in 0..(NUMBER_OF_CARDS - 6) {
        let h = Hand::new().add_card(i);
        for j in (i + 1)..(NUMBER_OF_CARDS - 5) {
            let h = h.add_card(j);
            for k in (j + 1)..(NUMBER_OF_CARDS - 4) {
                let h = h.add_card(k);
                for m in (k + 1)..(NUMBER_OF_CARDS - 3) {
                    let h = h.add_card(m);
                    for n in (m + 1)..(NUMBER_OF_CARDS - 2) {
                        let h = h.add_card(n);
                        for p in (n + 1)..(NUMBER_OF_CARDS - 1) {
                            let h = h.add_card(p);
                            for q in (p + 1)..NUMBER_OF_CARDS {
                                let h = h.add_card(q);
                                let r = HighRule::evaluate(&h);
                                rankset.insert(r);
                                counter[get_hand_category(r) as usize] += 1;
                            }
                        }
                    }
                }
            }
        }
    }

    assert_eq!(rankset.len(), 4824);
    assert_eq!(counter[HandCategory::StraightFlush as usize], 41584);
    assert_eq!(counter[HandCategory::FourOfAKind as usize], 224848);
    assert_eq!(counter[HandCategory::FullHouse as usize], 3473184);
    assert_eq!(counter[HandCategory::Flush as usize], 4047644);
    assert_eq!(counter[HandCategory::Straight as usize], 6180020);
    assert_eq!(counter[HandCategory::ThreeOfAKind as usize], 6461620);
    assert_eq!(counter[HandCategory::TwoPair as usize], 31433400);
    assert_eq!(counter[HandCategory::OnePair as usize], 58627800);
    assert_eq!(counter[HandCategory::HighCard as usize], 23294460);
}
