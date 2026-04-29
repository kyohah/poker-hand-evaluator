//! Parameterized naive high-hand evaluator.
//!
//! Adapted from b-inary/holdem-hand-evaluator's hand_test.rs with one
//! addition: a [`WheelMode`] flag controlling whether `A-2-3-4-5` is
//! recognized as a straight (Hold'em) or as A-high no-pair (2-7 lowball).
//!
//! Output convention: a 32-bit packed rank where bits 26-29 hold the
//! Hold'em-style category (0=HighCard, ..., 8=StraightFlush) and the
//! lower bits encode within-category tiebreaks. Higher = stronger as a
//! Hold'em high hand.

#![allow(clippy::needless_range_loop)]

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WheelMode {
    /// `A-2-3-4-5` is a straight, and `A-2-3-4-5` of one suit is a
    /// straight flush. Hold'em / Omaha / 8-or-better-low (where suits
    /// don't matter) all fall here.
    StraightAndFlush,
    /// `A-2-3-4-5` is A-high no-pair, and the same in one suit is just a
    /// flush. Used for 2-7 lowball.
    NoPair,
}

#[inline]
fn msb(x: u32) -> u32 {
    1 << (x.leading_zeros() ^ 31)
}

#[inline]
fn keep_n_msb(x: u32, n: usize) -> u32 {
    let mut x = x;
    let mut result = 0;
    for _ in 0..n {
        let m = msb(x);
        x ^= m;
        result |= m;
    }
    result
}

#[inline]
fn find_straight(rankset: u32, mode: WheelMode) -> Option<u32> {
    let wheel = 0b1_0000_0000_1111;
    match rankset & (rankset << 1) & (rankset << 2) & (rankset << 3) & (rankset << 4) {
        0 => match mode {
            WheelMode::StraightAndFlush if (rankset & wheel) == wheel => Some(1 << 3),
            _ => None,
        },
        x => Some(keep_n_msb(x, 1)),
    }
}

/// Computes the packed Hold'em-style rank for a 5-card hand under the
/// requested wheel mode.
pub fn eval_5cards(hand: &[usize], mode: WheelMode) -> u32 {
    debug_assert_eq!(hand.len(), 5);
    let mut rankset: u32 = 0;
    let mut rankset_suit: [u32; 4] = [0; 4];
    let mut rankset_of_count: [u32; 5] = [0; 5];
    let mut count: [usize; 13] = [0; 13];

    for card in hand {
        let suit = *card % 4;
        let rank = *card / 4;
        rankset |= 1 << rank;
        rankset_suit[suit] |= 1 << rank;
        count[rank] += 1;
    }

    for rank in 0..13 {
        rankset_of_count[count[rank]] |= 1 << rank;
    }

    let mut is_flush = -1i32;
    for i in 0..4 {
        if rankset_suit[i].count_ones() >= 5 {
            is_flush = i as i32;
        }
    }

    if is_flush >= 0 {
        match find_straight(rankset_suit[is_flush as usize], mode) {
            Some(x) => (8 << 26) | x,
            None => (5 << 26) | keep_n_msb(rankset_suit[is_flush as usize], 5),
        }
    } else if rankset_of_count[4] > 0 {
        let remaining = keep_n_msb(rankset ^ rankset_of_count[4], 1);
        (7 << 26) | (rankset_of_count[4] << 13) | remaining
    } else if rankset_of_count[3].count_ones() == 2 {
        let trips = keep_n_msb(rankset_of_count[3], 1);
        let pair = rankset_of_count[3] ^ trips;
        (6 << 26) | (trips << 13) | pair
    } else if rankset_of_count[3] > 0 && rankset_of_count[2] > 0 {
        let pair = keep_n_msb(rankset_of_count[2], 1);
        (6 << 26) | (rankset_of_count[3] << 13) | pair
    } else if let Some(x) = find_straight(rankset, mode) {
        (4 << 26) | x
    } else if rankset_of_count[3] > 0 {
        let remaining = keep_n_msb(rankset_of_count[1], 2);
        (3 << 26) | (rankset_of_count[3] << 13) | remaining
    } else if rankset_of_count[2].count_ones() >= 2 {
        let pairs = keep_n_msb(rankset_of_count[2], 2);
        let remaining = keep_n_msb(rankset ^ pairs, 1);
        (2 << 26) | (pairs << 13) | remaining
    } else if rankset_of_count[2] > 0 {
        let remaining = keep_n_msb(rankset_of_count[1], 3);
        (1 << 26) | (rankset_of_count[2] << 13) | remaining
    } else {
        keep_n_msb(rankset, 5)
    }
}

/// Picks the best 5-card sub-hand from `hand` under the `compare`
/// function: pass `u32::max` for Hold'em-style "highest wins", or
/// `u32::min` for 2-7 lowball "lowest wins".
fn best_subset(hand: &[usize], mode: WheelMode, compare: fn(u32, u32) -> u32) -> u32 {
    debug_assert!(hand.len() == 6 || hand.len() == 7);
    let n = hand.len();
    let mut acc: Option<u32> = None;
    let mut sub = [0usize; 5];
    let combos: &[[usize; 5]] = if n == 6 { &SUBSETS_6 } else { &SUBSETS_7 };
    for idxs in combos {
        for (s, &i) in sub.iter_mut().zip(idxs.iter()) {
            *s = hand[i];
        }
        let v = eval_5cards(&sub, mode);
        acc = Some(match acc {
            None => v,
            Some(prev) => compare(prev, v),
        });
    }
    acc.unwrap()
}

/// Hold'em-shape: best 5-card sub of 6 or 7 (highest packed rank wins).
pub fn eval_6_or_7_high(hand: &[usize], mode: WheelMode) -> u32 {
    best_subset(hand, mode, u32::max)
}

/// 2-7 lowball: best 5-card sub of 6 or 7 (lowest packed rank = best 2-7 hand).
pub fn eval_6_or_7_low_2_7(hand: &[usize], mode: WheelMode) -> u32 {
    best_subset(hand, mode, u32::min)
}

const SUBSETS_6: [[usize; 5]; 6] = [
    [0, 1, 2, 3, 4],
    [0, 1, 2, 3, 5],
    [0, 1, 2, 4, 5],
    [0, 1, 3, 4, 5],
    [0, 2, 3, 4, 5],
    [1, 2, 3, 4, 5],
];

const SUBSETS_7: [[usize; 5]; 21] = [
    [0, 1, 2, 3, 4],
    [0, 1, 2, 3, 5],
    [0, 1, 2, 3, 6],
    [0, 1, 2, 4, 5],
    [0, 1, 2, 4, 6],
    [0, 1, 2, 5, 6],
    [0, 1, 3, 4, 5],
    [0, 1, 3, 4, 6],
    [0, 1, 3, 5, 6],
    [0, 1, 4, 5, 6],
    [0, 2, 3, 4, 5],
    [0, 2, 3, 4, 6],
    [0, 2, 3, 5, 6],
    [0, 2, 4, 5, 6],
    [0, 3, 4, 5, 6],
    [1, 2, 3, 4, 5],
    [1, 2, 3, 4, 6],
    [1, 2, 3, 5, 6],
    [1, 2, 4, 5, 6],
    [1, 3, 4, 5, 6],
    [2, 3, 4, 5, 6],
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wheel_is_straight_in_holdem_mode() {
        // A-2-3-4-5 mixed suits: 2c=0, 3d=5, 4h=10, 5s=15, Ac=48
        let cards = [0, 5, 10, 15, 48];
        let r = eval_5cards(&cards, WheelMode::StraightAndFlush);
        assert_eq!(r >> 26, 4, "expected Straight (cat 4), got {}", r >> 26);
    }

    #[test]
    fn wheel_is_high_card_in_no_pair_mode() {
        let cards = [0, 5, 10, 15, 48]; // 2c 3d 4h 5s Ac
        let r = eval_5cards(&cards, WheelMode::NoPair);
        assert_eq!(r >> 26, 0, "expected HighCard (cat 0), got {}", r >> 26);
    }

    #[test]
    fn broadway_remains_a_straight_in_no_pair_mode() {
        // T-J-Q-K-A mixed suits
        let cards = [32, 37, 42, 47, 48]; // Tc Jd Qh Ks Ac
        let r = eval_5cards(&cards, WheelMode::NoPair);
        assert_eq!(r >> 26, 4, "broadway must still be a straight");
    }
}
