use crate::category::{get_hand_category, HandCategory};
use crate::eval::HighRule;
use phe_core::{Hand, CARDS, NUMBER_OF_CARDS};

const NUM_HAND_CATEGORIES: usize = HandCategory::StraightFlush as usize + 1;

/// Enumerates how often each [`HandCategory`] occurs over every possible
/// completion of `hand` to a 7-card hand, given the supplied `dead_cards`.
///
/// `hand.len()` must be in `2..=7`; the returned counts sum to
/// `C(52 - hand.len() - dead_cards.len(), 7 - hand.len())`.
pub fn enumerate_hand_category(hand: &Hand, dead_cards: &Hand) -> [u32; NUM_HAND_CATEGORIES] {
    assert!((2..=7).contains(&hand.len()));
    assert_eq!(hand.get_mask() & dead_cards.get_mask(), 0);
    let alive = compute_alive_cards(hand.get_mask() | dead_cards.get_mask());
    assert!(alive.len() >= 7 - hand.len());
    match hand.len() {
        2 => enumerate_2(hand, &alive),
        3 => enumerate_3(hand, &alive),
        4 => enumerate_4(hand, &alive),
        5 => enumerate_5(hand, &alive),
        6 => enumerate_6(hand, &alive),
        7 => enumerate_7(hand),
        _ => unreachable!(),
    }
}

fn compute_alive_cards(mask: u64) -> Vec<usize> {
    (0..NUMBER_OF_CARDS)
        .filter(|&i| (CARDS[i].1 & mask) == 0)
        .collect()
}

fn bump(counter: &mut [u32; NUM_HAND_CATEGORIES], h: &Hand) {
    counter[get_hand_category(HighRule::evaluate(h)) as usize] += 1;
}

fn enumerate_2(hand: &Hand, alive: &[usize]) -> [u32; NUM_HAND_CATEGORIES] {
    let len = alive.len();
    let mut r = [0; NUM_HAND_CATEGORIES];
    for i in 0..(len - 4) {
        let h = hand.add_card(alive[i]);
        for j in (i + 1)..(len - 3) {
            let h = h.add_card(alive[j]);
            for k in (j + 1)..(len - 2) {
                let h = h.add_card(alive[k]);
                for m in (k + 1)..(len - 1) {
                    let h = h.add_card(alive[m]);
                    for n in (m + 1)..len {
                        let h = h.add_card(alive[n]);
                        bump(&mut r, &h);
                    }
                }
            }
        }
    }
    r
}

fn enumerate_3(hand: &Hand, alive: &[usize]) -> [u32; NUM_HAND_CATEGORIES] {
    let len = alive.len();
    let mut r = [0; NUM_HAND_CATEGORIES];
    for i in 0..(len - 3) {
        let h = hand.add_card(alive[i]);
        for j in (i + 1)..(len - 2) {
            let h = h.add_card(alive[j]);
            for k in (j + 1)..(len - 1) {
                let h = h.add_card(alive[k]);
                for m in (k + 1)..len {
                    let h = h.add_card(alive[m]);
                    bump(&mut r, &h);
                }
            }
        }
    }
    r
}

fn enumerate_4(hand: &Hand, alive: &[usize]) -> [u32; NUM_HAND_CATEGORIES] {
    let len = alive.len();
    let mut r = [0; NUM_HAND_CATEGORIES];
    for i in 0..(len - 2) {
        let h = hand.add_card(alive[i]);
        for j in (i + 1)..(len - 1) {
            let h = h.add_card(alive[j]);
            for k in (j + 1)..len {
                let h = h.add_card(alive[k]);
                bump(&mut r, &h);
            }
        }
    }
    r
}

fn enumerate_5(hand: &Hand, alive: &[usize]) -> [u32; NUM_HAND_CATEGORIES] {
    let len = alive.len();
    let mut r = [0; NUM_HAND_CATEGORIES];
    for i in 0..(len - 1) {
        let h = hand.add_card(alive[i]);
        for j in (i + 1)..len {
            let h = h.add_card(alive[j]);
            bump(&mut r, &h);
        }
    }
    r
}

fn enumerate_6(hand: &Hand, alive: &[usize]) -> [u32; NUM_HAND_CATEGORIES] {
    let mut r = [0; NUM_HAND_CATEGORIES];
    for &c in alive {
        let h = hand.add_card(c);
        bump(&mut r, &h);
    }
    r
}

fn enumerate_7(hand: &Hand) -> [u32; NUM_HAND_CATEGORIES] {
    let mut r = [0; NUM_HAND_CATEGORIES];
    bump(&mut r, hand);
    r
}
