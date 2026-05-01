use phe_eight_low_assets::constants::*;
use phe_eight_low_assets::lookup::LOOKUP;
use phe_eight_low_assets::offsets::OFFSETS;
use std::ops::{Add, AddAssign};
use std::str::FromStr;

/// Low hand categories, ordered from best (NoPair=0) to worst (FourOfAKind=5).
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum LowHandCategory {
    NoPair = 0,
    OnePair = 1,
    TwoPair = 2,
    ThreeOfAKind = 3,
    FullHouse = 4,
    FourOfAKind = 5,
}

/// Number of distinct rank values per category (5-card low hands).
pub const LOW_CATEGORY_COUNTS: [u16; 6] = [
    1287, // NoPair: C(13,5)
    2860, // OnePair: 13 * C(12,3)
    858,  // TwoPair: C(13,2) * 11
    858,  // ThreeOfAKind: 13 * C(12,2)
    156,  // FullHouse: 13 * 12
    156,  // FourOfAKind: 13 * 12
];

/// Total number of distinct low hand equivalence classes.
pub const TOTAL_LOW_RANKS: u16 = 6175;

/// Threshold rank value for 8-or-better qualification.
/// Hands with rank <= this value qualify (all no-pair hands with cards A-8).
/// C(8,5) = 56 qualifying no-pair hands, so threshold = 55 (0-indexed).
pub const EIGHT_OR_BETTER_THRESHOLD: u16 = 55;

/// Returns the low hand category from hand rank computed by `Hand::evaluate()`.
#[inline]
pub fn get_low_category(hand_rank: u16) -> LowHandCategory {
    if hand_rank < 1287 {
        LowHandCategory::NoPair
    } else if hand_rank < 1287 + 2860 {
        LowHandCategory::OnePair
    } else if hand_rank < 1287 + 2860 + 858 {
        LowHandCategory::TwoPair
    } else if hand_rank < 1287 + 2860 + 858 + 858 {
        LowHandCategory::ThreeOfAKind
    } else if hand_rank < 1287 + 2860 + 858 + 858 + 156 {
        LowHandCategory::FullHouse
    } else {
        LowHandCategory::FourOfAKind
    }
}

/// Returns whether the hand qualifies for 8-or-better.
#[inline]
pub fn qualifies_8_or_better(hand_rank: u16) -> bool {
    hand_rank <= EIGHT_OR_BETTER_THRESHOLD
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Hand {
    key: u64,
    mask: u64,
}

impl Hand {
    /// Creates an empty `Hand` struct.
    #[inline]
    pub fn new() -> Self {
        Self { key: 0, mask: 0 }
    }

    /// Creates a new hand from a slice of card IDs.
    /// Elements must be in the range \[0, 51\].
    /// (0 = Ac, 1 = Ad, 2 = Ah, 3 = As, 4 = 2c, ..., 51 = Ks)
    #[inline]
    pub fn from_slice(cards: &[usize]) -> Self {
        let mut hand = Self::new();
        for card in cards {
            hand = hand.add_card(*card);
        }
        hand
    }

    /// Checks whether the hand is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.mask == 0
    }

    /// Returns current number of cards in `self`.
    #[inline]
    pub fn len(&self) -> usize {
        self.mask.count_ones() as usize
    }

    /// Returns the bit mask of `self`.
    #[inline]
    pub fn get_mask(&self) -> u64 {
        self.mask
    }

    /// Returns whether the `card` is included in `self`.
    ///
    /// # Safety
    /// `card` must be in the range \[0, 51\]. Out-of-range inputs are
    /// undefined behaviour (the hot-path uses `get_unchecked`).
    #[inline]
    pub fn contains(&self, card: usize) -> bool {
        // SAFETY: caller-asserted `card ∈ [0, 51]` indexes CARDS (length 52).
        (self.mask & unsafe { *CARDS.get_unchecked(card) }.1) != 0
    }

    /// Returns a new hand struct where `card` is added to `self`.
    ///
    /// # Safety
    /// `card` must be in the range \[0, 51\] and must not already be
    /// included in `self`. Adding duplicate cards or cards outside this
    /// range is undefined behaviour.
    #[inline]
    pub fn add_card(&self, card: usize) -> Self {
        // SAFETY: caller-asserted `card ∈ [0, 51]` indexes CARDS (length 52).
        let (k, m) = unsafe { *CARDS.get_unchecked(card) };
        Self {
            key: self.key.wrapping_add(k),
            mask: self.mask | m,
        }
    }

    /// Returns a new hand struct where `card` is removed from `self`.
    ///
    /// # Safety
    /// `card` must be in the range \[0, 51\] and must currently be
    /// present in `self`. Out-of-range inputs are undefined behaviour.
    #[inline]
    pub fn remove_card(&self, card: usize) -> Self {
        // SAFETY: caller-asserted `card ∈ [0, 51]` indexes CARDS (length 52).
        let (k, m) = unsafe { *CARDS.get_unchecked(card) };
        Self {
            key: self.key.wrapping_sub(k),
            mask: self.mask & !m,
        }
    }

    /// Returns low hand strength as a 16-bit integer.
    ///
    /// Lower value = stronger hand. 0 = A-2-3-4-5 (the wheel, best possible).
    ///
    /// # Safety
    /// Behaviour is undefined when `self.len() < 5` or `self.len() > 7`.
    /// Only 5, 6, or 7 card hands produce valid results.
    #[inline]
    pub fn evaluate(&self) -> u16 {
        let rank_key = self.key as u32 as usize;
        // SAFETY: 5..=7-card invariant keeps `rank_key >> OFFSET_SHIFT`
        // inside OFFSETS, and the displaced `rank_key + offset` lands
        // inside the dense LOOKUP table (perfect-hash by construction).
        let offset = unsafe { *OFFSETS.get_unchecked(rank_key >> OFFSET_SHIFT) as usize };
        let hash_key = rank_key.wrapping_add(offset);
        unsafe { *LOOKUP.get_unchecked(hash_key) }
    }
}

// `mask | rhs.mask` is the correct semantics for the 52-bit card
// presence set (cards are unique per hand, so OR == add). Clippy's
// `suspicious_arithmetic_impl` and `suspicious_op_assign_impl` only
// know that we used `|` inside an arithmetic trait; they don't know
// the type-level invariant that the masks are disjoint.
#[allow(clippy::suspicious_arithmetic_impl)]
impl Add for Hand {
    type Output = Self;
    fn add(self, rhs: Self) -> Self::Output {
        Self {
            key: self.key.wrapping_add(rhs.key),
            mask: self.mask | rhs.mask,
        }
    }
}

#[allow(clippy::suspicious_op_assign_impl, clippy::assign_op_pattern)]
impl AddAssign for Hand {
    fn add_assign(&mut self, rhs: Self) {
        self.key = self.key.wrapping_add(rhs.key);
        self.mask = self.mask | rhs.mask;
    }
}

impl Default for Hand {
    fn default() -> Self {
        Self::new()
    }
}

impl FromStr for Hand {
    type Err = String;

    fn from_str(hand_str: &str) -> Result<Self, Self::Err> {
        let mut hand = Self::new();
        let mut chars = hand_str.chars();
        loop {
            let rank_char = match chars.next() {
                Some(c) => c,
                None => return Ok(hand),
            };
            let suit_char = chars
                .next()
                .ok_or("parse failed: expected suit character, but got EOF")?;
            let rank_id = match rank_char.to_ascii_uppercase() {
                'A' => Ok(0),
                '2' => Ok(1),
                '3' => Ok(2),
                '4' => Ok(3),
                '5' => Ok(4),
                '6' => Ok(5),
                '7' => Ok(6),
                '8' => Ok(7),
                '9' => Ok(8),
                'T' => Ok(9),
                'J' => Ok(10),
                'Q' => Ok(11),
                'K' => Ok(12),
                ch => Err(format!(
                    "parse failed: expected rank character, but got '{}'",
                    ch
                )),
            }?;
            let suit_id = match suit_char.to_ascii_lowercase() {
                'c' => Ok(0),
                'd' => Ok(1),
                'h' => Ok(2),
                's' => Ok(3),
                ch => Err(format!(
                    "parse failed: expected suit character, but got '{}'",
                    ch
                )),
            }?;
            hand = hand.add_card(rank_id * 4 + suit_id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parser() {
        let cards = [0, 4, 8, 12, 16]; // Ac 2c 3c 4c 5c
        let hand_from_vec = Hand::from_slice(&cards);
        let hand_from_str = "Ac2c3c4c5c".parse::<Hand>();
        assert_eq!(hand_from_str, Ok(hand_from_vec));
        assert_eq!("".parse::<Hand>(), Ok(Hand::new()));
        assert_eq!(
            "A".parse::<Hand>(),
            Err("parse failed: expected suit character, but got EOF".into())
        );
        assert_eq!(
            "Ax".parse::<Hand>(),
            Err("parse failed: expected suit character, but got 'x'".into())
        );
    }

    #[test]
    fn test_hand_len() {
        let hand = "Ac2c3c4c5c".parse::<Hand>().unwrap();
        assert_eq!(hand.len(), 5);
        let hand = "Ac2c3c4c5c6c7c".parse::<Hand>().unwrap();
        assert_eq!(hand.len(), 7);
    }

    #[test]
    fn test_hand_contains() {
        let hand = "Ac2d3h".parse::<Hand>().unwrap();
        assert!(hand.contains(0)); // Ac
        assert!(hand.contains(5)); // 2d
        assert!(hand.contains(10)); // 3h
        assert!(!hand.contains(1)); // Ad
    }

    #[test]
    fn test_hand_addition() {
        let hand1 = "Ac2c".parse::<Hand>().unwrap();
        let hand2 = "3c4c5c".parse::<Hand>().unwrap();
        let combined = hand1 + hand2;
        assert_eq!(combined.len(), 5);
        assert_eq!(combined, "Ac2c3c4c5c".parse::<Hand>().unwrap());
    }
}
