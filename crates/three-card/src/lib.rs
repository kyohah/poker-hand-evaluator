//! 3-card high evaluator for the Open-Face Chinese poker top row.
//!
//! OFC has a 3-card "top" row that is scored independently from the
//! 5-card middle/bottom rows. A 3-card hand can be one of three
//! categories — High Card, Pair, or Three of a Kind — because 3 cards
//! can't form a straight or flush.
//!
//! The downstream consumer (an OFC solver) hits this evaluator on every
//! leaf of a branch-and-bound search over board partitions, so the
//! implementation is allocation-free, branch-on-rank-multiset, and
//! `#[inline]` at the entry point.
//!
//! ## Card encoding (input)
//!
//! Hold'em-style: `card = rank * 4 + suit`, with rank `0='2', ..., 12='A'`
//! (Ace high) and suit `0=c, 1=d, 2=h, 3=s`. Suit is read but ignored
//! (a 3-card flush is not a recognised category).
//!
//! ## Strength encoding
//!
//! Returns `u16`, **higher = stronger**. Layout (MSB → LSB):
//!
//! ```text
//!   bits 15..12 : category (0 = HighCard, 1 = Pair, 2 = Trips)
//!   bits 11..0  : within-category index
//! ```
//!
//! Within-category packing:
//!
//! | Category | Packing                                              |
//! |----------|------------------------------------------------------|
//! | HighCard | `(top << 8) \| (mid << 4) \| low`, ranks descending  |
//! | Pair     | `(pair_rank << 4) \| kicker`                         |
//! | Trips    | `trip_rank` (4 bits)                                 |
//!
//! Plain `u16` comparison gives the correct order: trips beat any pair,
//! any pair beats any high card, and within a category the higher
//! rank-tuple wins.
//!
//! ## Joker / wildcards
//!
//! Not supported. OFC variants with Jokers handle substitution in the
//! solver (argmax over rank-suit substitutions) on top of this
//! joker-free evaluator. Inputs must be in `0..52`.

#![warn(missing_docs)]
#![forbid(unsafe_code)]

/// Category bits packed into the top nibble of the strength.
const CAT_HIGH_CARD: u16 = 0;
const CAT_PAIR: u16 = 1;
const CAT_TRIPS: u16 = 2;

/// Tag-only rule so callers can write `ThreeCardRule.evaluate(&cards)`.
///
/// Zero-sized — pass by value. The facade crate provides the
/// `HandRule` impl that adapts this to the `&[u8]` trait surface.
#[derive(Default, Clone, Copy, Debug)]
pub struct ThreeCardRule;

impl ThreeCardRule {
    /// Evaluates a 3-card hand and returns its packed `u16` strength.
    ///
    /// `cards` are Hold'em-encoded (`rank * 4 + suit`). Suit is ignored
    /// — no 3-card flush category exists.
    #[inline]
    pub fn evaluate(cards: [u8; 3]) -> u16 {
        // Sort ranks descending with a 3-comparison network. Keeping
        // it in registers matters for the OFC hot path; a generic
        // sort would spill.
        let mut r = [cards[0] / 4, cards[1] / 4, cards[2] / 4];
        if r[0] < r[1] {
            r.swap(0, 1);
        }
        if r[1] < r[2] {
            r.swap(1, 2);
        }
        if r[0] < r[1] {
            r.swap(0, 1);
        }
        let (top, mid, low) = (r[0] as u16, r[1] as u16, r[2] as u16);

        match (top == mid, mid == low) {
            (true, true) => (CAT_TRIPS << 12) | top,
            (true, false) => (CAT_PAIR << 12) | (top << 4) | low,
            (false, true) => (CAT_PAIR << 12) | (mid << 4) | top,
            (false, false) => (CAT_HIGH_CARD << 12) | (top << 8) | (mid << 4) | low,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(rank: u8, suit: u8) -> u8 {
        rank * 4 + suit
    }

    #[test]
    fn high_card_aceking_queen_offsuit() {
        let s = ThreeCardRule::evaluate([c(12, 3), c(11, 1), c(10, 0)]);
        assert_eq!(s >> 12, 0);
        assert_eq!(s & 0xfff, (12 << 8) | (11 << 4) | 10);
    }

    #[test]
    fn pair_of_aces_with_king_kicker() {
        let s = ThreeCardRule::evaluate([c(12, 3), c(12, 1), c(11, 0)]);
        assert_eq!(s >> 12, 1);
        assert_eq!(s & 0xfff, (12 << 4) | 11);
    }

    #[test]
    fn pair_when_low_two_match() {
        // 7 ♣ 4 ♦ 4 ♠ → pair of 4s with 7 kicker.
        let s = ThreeCardRule::evaluate([c(5, 0), c(2, 1), c(2, 3)]);
        assert_eq!(s >> 12, 1);
        assert_eq!(s & 0xfff, (2 << 4) | 5);
    }

    #[test]
    fn trips_aces() {
        let s = ThreeCardRule::evaluate([c(12, 3), c(12, 1), c(12, 0)]);
        assert_eq!(s >> 12, 2);
        assert_eq!(s & 0xfff, 12);
    }

    #[test]
    fn category_boundaries_are_monotonic() {
        let high = ThreeCardRule::evaluate([c(12, 3), c(11, 1), c(10, 0)]); // AKQ
        let pair = ThreeCardRule::evaluate([c(0, 0), c(0, 1), c(1, 0)]); // 22-3
        let trips = ThreeCardRule::evaluate([c(0, 0), c(0, 1), c(0, 2)]); // 222
        assert!(high < pair);
        assert!(pair < trips);
    }
}
