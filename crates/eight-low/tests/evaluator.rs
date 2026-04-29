//! Ported from kyohah/8low-evaluator/src/hand_test.rs.
//!
//! Cross-validates the lookup-driven evaluator against a naive
//! 5-from-7 brute-force evaluator and checks 5-card category counts.

use phe_eight_low::{get_low_category, qualifies_8_or_better, Hand, LowHandCategory};
use phe_eight_low_assets::constants::NUMBER_OF_CARDS;
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::sync::OnceLock;

fn category_and_tiebreak(ranks: &[usize; 5]) -> (u8, Vec<usize>) {
    let mut count = [0u8; 13];
    for &r in ranks {
        count[r] += 1;
    }
    let mut pairs = Vec::new();
    let mut trips = Vec::new();
    let mut quads = Vec::new();
    let mut singles = Vec::new();
    for rank in 0..13 {
        match count[rank] {
            1 => singles.push(rank),
            2 => pairs.push(rank),
            3 => trips.push(rank),
            4 => quads.push(rank),
            _ => {}
        }
    }
    if quads.len() == 1 {
        return (5, vec![quads[0], singles[0]]);
    }
    if trips.len() == 1 && pairs.len() == 1 {
        return (4, vec![trips[0], pairs[0]]);
    }
    if trips.len() == 1 {
        singles.sort();
        singles.reverse();
        return (3, vec![trips[0], singles[0], singles[1]]);
    }
    if pairs.len() == 2 {
        pairs.sort();
        return (2, vec![pairs[1], pairs[0], singles[0]]);
    }
    if pairs.len() == 1 {
        singles.sort();
        singles.reverse();
        return (1, vec![pairs[0], singles[0], singles[1], singles[2]]);
    }
    singles.sort();
    singles.reverse();
    (0, singles)
}

struct NaiveTable {
    key_to_rank: BTreeMap<(u8, Vec<usize>), u16>,
}

fn build_naive_table() -> NaiveTable {
    let mut keys = BTreeSet::new();
    for a in 0..13usize {
        for b in a..13 {
            for c in b..13 {
                for d in c..13 {
                    for e in d..13 {
                        let mut count = [0u8; 13];
                        count[a] += 1;
                        count[b] += 1;
                        count[c] += 1;
                        count[d] += 1;
                        count[e] += 1;
                        if count.iter().any(|&c| c > 4) {
                            continue;
                        }
                        keys.insert(category_and_tiebreak(&[a, b, c, d, e]));
                    }
                }
            }
        }
    }
    let mut key_to_rank = BTreeMap::new();
    for (rank, key) in keys.iter().enumerate() {
        key_to_rank.insert(key.clone(), rank as u16);
    }
    NaiveTable { key_to_rank }
}

static NAIVE_TABLE: OnceLock<NaiveTable> = OnceLock::new();

fn naive_eval_5(ranks: &[usize; 5]) -> u16 {
    let table = NAIVE_TABLE.get_or_init(build_naive_table);
    *table.key_to_rank.get(&category_and_tiebreak(ranks)).unwrap()
}

fn naive_eval_7cards(cards: &[usize; 7]) -> u16 {
    let mut best = u16::MAX;
    for i in 0..7 {
        for j in (i + 1)..7 {
            let mut sub = [0usize; 5];
            let mut idx = 0;
            for k in 0..7 {
                if k != i && k != j {
                    sub[idx] = cards[k] / 4;
                    idx += 1;
                }
            }
            best = best.min(naive_eval_5(&sub));
        }
    }
    best
}

#[test]
fn all_5card_combinations() {
    let mut rankset = HashSet::new();
    let mut category_counts = [0u32; 6];

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
                        let r = h.evaluate();
                        rankset.insert(r);
                        category_counts[get_low_category(r) as usize] += 1;
                    }
                }
            }
        }
    }

    assert_eq!(rankset.len(), 6175);
    assert_eq!(category_counts[LowHandCategory::NoPair as usize], 1_317_888);
    assert_eq!(category_counts[LowHandCategory::OnePair as usize], 1_098_240);
    assert_eq!(category_counts[LowHandCategory::TwoPair as usize], 123_552);
    assert_eq!(category_counts[LowHandCategory::ThreeOfAKind as usize], 54_912);
    assert_eq!(category_counts[LowHandCategory::FullHouse as usize], 3_744);
    assert_eq!(category_counts[LowHandCategory::FourOfAKind as usize], 624);
}

#[test]
#[ignore = "C(52,7) = 133,784,560 hand cross-check; run with --release -- --ignored"]
fn all_7card_cross_validation() {
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
                                let fast = h.evaluate();
                                let naive = naive_eval_7cards(&[i, j, k, m, n, p, q]);
                                assert_eq!(
                                    fast, naive,
                                    "Mismatch for cards [{},{},{},{},{},{},{}]",
                                    i, j, k, m, n, p, q
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn edge_cases() {
    fn eval(s: &str) -> u16 {
        s.parse::<Hand>().unwrap().evaluate()
    }

    assert_eq!(eval("Ac2d3h4s5c6d7h"), 0);
    assert_eq!(eval("As2h3c4d5s8cKd"), 0);
    assert_eq!(eval("Ac2c3c4c5c6c7c"), eval("As2s3s4s5s6s7s"));

    assert!(qualifies_8_or_better(eval("Ac2d3h4s8cTdKh")));
    assert!(!qualifies_8_or_better(eval("Ac2d3h4s9cTdKh")));

    let forced_pair = eval("AcAd2c2d3c3d4c");
    assert_eq!(get_low_category(forced_pair), LowHandCategory::OnePair);

    let worst_np = "9cTdJhQsKc".parse::<Hand>().unwrap().evaluate();
    assert!(worst_np < forced_pair);

    assert_eq!(get_low_category(eval("Ac2d3h4s5c6d7h")), LowHandCategory::NoPair);
    assert_eq!(get_low_category(forced_pair), LowHandCategory::OnePair);
}

#[test]
fn remove_card_round_trip() {
    let h = "Ac2d3h4s5c".parse::<Hand>().unwrap();
    assert_eq!(h.len(), 5);

    let removed = h.remove_card(0);
    assert_eq!(removed.len(), 4);
    assert!(!removed.contains(0));
    assert!(removed.contains(5));

    let restored = removed.add_card(0);
    assert_eq!(restored.evaluate(), h.evaluate());
}

#[test]
fn hand_addition() {
    let a = "Ac2c".parse::<Hand>().unwrap();
    let b = "3c4c5c".parse::<Hand>().unwrap();
    let combined = a + b;
    assert_eq!(combined.len(), 5);
    assert_eq!(combined.evaluate(), 0);
}

#[test]
fn hand_add_assign() {
    let mut h = "Ac2c3c".parse::<Hand>().unwrap();
    h += "4c5c".parse::<Hand>().unwrap();
    assert_eq!(h.len(), 5);
    assert_eq!(h.evaluate(), 0);
}
