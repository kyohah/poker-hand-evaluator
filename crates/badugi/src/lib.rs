//! Badugi 4-card lowball evaluator.
//!
//! Badugi is a draw game where each player has 4 cards and the goal is
//! the lowest 4-card hand with all distinct ranks **and** all distinct
//! suits. If the dealt hand has any rank or suit collision, the player
//! is forced to drop cards until both constraints hold; the resulting
//! "Badugi" can be 4, 3, 2, or 1 card. More cards is always stronger;
//! within the same count, the smaller largest-card wins (with Ace
//! treated as low).
//!
//! ## Strength encoding
//!
//! [`BadugiStrength`] is `(count, Reverse(packed_ranks))`. Derived
//! `Ord` compares `count` first (4 > 3 > 2 > 1) and then the
//! Reverse-wrapped tiebreak (smaller raw packed ranks → stronger).
//!
//! ## Card encoding (input)
//!
//! Hold'em-style: `card = rank * 4 + suit`, `rank 0='2', 12='A'`,
//! `suit 0=c, 1=d, 2=h, 3=s`. Internally the rank is translated to
//! Badugi semantics where Ace is the smallest rank.

#![forbid(unsafe_code)]

use std::cmp::Reverse;

/// Mapping from Hold'em-encoded rank index to Badugi rank index
/// (Ace-low). Holdem rank 12 (Ace) → Badugi rank 0; everything else
/// shifts up by 1.
const RANK_HOLDEM_TO_BADUGI: [u8; 13] = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 0];

#[inline]
fn badugi_rank(card: u8) -> u8 {
    RANK_HOLDEM_TO_BADUGI[(card / 4) as usize]
}

#[inline]
fn suit(card: u8) -> u8 {
    card % 4
}

/// A Badugi hand's strength: the count of cards in the best subset
/// plus a tiebreak over those cards' ranks.
///
/// `Ord` is "higher is stronger" — a 4-badugi beats any 3-badugi, and
/// within the same count smaller top-card wins (then second-largest,
/// etc.). The wheel `A-2-3-4` (ranks 0,1,2,3) is the maximum.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct BadugiStrength {
    count: u8,
    /// Ranks of the chosen subset, sorted descending and packed as
    /// nibbles, then `Reverse`d so smaller raw value = stronger hand.
    tiebreak: Reverse<u32>,
}

impl BadugiStrength {
    /// Number of cards in the best Badugi subset (1..=4).
    #[inline]
    pub fn count(&self) -> u8 {
        self.count
    }
}

/// Badugi rule.
pub struct BadugiRule;

impl BadugiRule {
    /// Evaluates a Badugi hand of exactly 4 cards.
    pub fn evaluate(cards: [u8; 4]) -> BadugiStrength {
        let mut ranks = [0u8; 4];
        let mut suits = [0u8; 4];
        for i in 0..4 {
            ranks[i] = badugi_rank(cards[i]);
            suits[i] = suit(cards[i]);
        }

        let mut best_count: u8 = 0;
        let mut best_tiebreak: u32 = u32::MAX;

        // Enumerate every non-empty subset of the 4 cards. Bit `i`
        // selects card `i`. Mask 0 is skipped (empty hand never wins
        // — and any singleton trivially satisfies both constraints).
        for mask in 1u8..16 {
            let count = mask.count_ones() as u8;
            // Once we've seen a subset with greater count, smaller
            // subsets cannot improve the answer.
            if count < best_count {
                continue;
            }

            let mut sub_ranks = [0u8; 4];
            let mut sub_suits = [0u8; 4];
            let mut idx = 0;
            for i in 0..4 {
                if mask & (1 << i) != 0 {
                    sub_ranks[idx] = ranks[i];
                    sub_suits[idx] = suits[i];
                    idx += 1;
                }
            }

            if !all_distinct(&sub_ranks[..idx]) || !all_distinct(&sub_suits[..idx]) {
                continue;
            }

            // Sort ranks descending, pack into nibbles.
            let mut sorted = sub_ranks;
            sorted[..idx].sort_unstable_by(|a, b| b.cmp(a));
            let mut tb: u32 = 0;
            for i in 0..idx {
                tb = (tb << 4) | (sorted[i] as u32);
            }

            if count > best_count || tb < best_tiebreak {
                best_count = count;
                best_tiebreak = tb;
            }
        }

        BadugiStrength {
            count: best_count,
            tiebreak: Reverse(best_tiebreak),
        }
    }
}

#[inline]
fn all_distinct(xs: &[u8]) -> bool {
    for i in 0..xs.len() {
        for j in (i + 1)..xs.len() {
            if xs[i] == xs[j] {
                return false;
            }
        }
    }
    true
}
