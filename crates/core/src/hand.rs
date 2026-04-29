use crate::constants::*;
use crate::offsets::OFFSETS;
use std::ops::{Add, AddAssign};

/// A 5-to-7-card poker hand encoded as `(rank_key, mask)`.
///
/// `key` is a perfect-hash sum over the rank/suit base values defined in
/// `constants.rs`; `mask` is a 52-bit set indicating which of the 52 cards
/// are present.
///
/// This struct is variant-agnostic: it does not know whether it is being
/// scored as Hold'em high or 2-7 lowball. The variant comes from the lookup
/// tables passed to [`evaluate_via_lookup`].
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Hand {
    key: u64,
    mask: u64,
}

impl Hand {
    /// Creates an empty `Hand`.
    #[inline]
    pub fn new() -> Self {
        Self {
            key: 0x3333 << SUIT_SHIFT,
            mask: 0,
        }
    }

    /// Builds a hand from a slice of card IDs in the range `[0, 51]`.
    ///
    /// Card encoding: `card = rank * 4 + suit`, with suits ordered
    /// `0=club, 1=diamond, 2=heart, 3=spade`. The interpretation of the
    /// `rank` index (Ace-high vs. Ace-low) is decided by which lookup
    /// table is used at evaluation time.
    #[inline]
    pub fn from_slice(cards: &[usize]) -> Self {
        let mut hand = Self::new();
        for card in cards {
            hand = hand.add_card(*card);
        }
        hand
    }

    /// Returns whether the hand contains no cards.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.mask == 0
    }

    /// Returns the number of cards in the hand.
    #[inline]
    pub fn len(&self) -> usize {
        self.mask.count_ones() as usize
    }

    /// Returns the raw 52-bit card mask.
    #[inline]
    pub fn get_mask(&self) -> u64 {
        self.mask
    }

    /// Returns the perfect-hash key (used by [`evaluate_via_lookup`]).
    #[inline]
    pub fn get_key(&self) -> u64 {
        self.key
    }

    /// Returns whether `card` is contained in the hand.
    ///
    /// `card` must be in the range `[0, 51]`.
    #[inline]
    pub fn contains(&self, card: usize) -> bool {
        (self.mask & unsafe { *CARDS.get_unchecked(card) }.1) != 0
    }

    /// Returns a new hand with `card` added.
    ///
    /// `card` must be in the range `[0, 51]` and must not already be present.
    #[inline]
    pub fn add_card(&self, card: usize) -> Self {
        let (k, m) = unsafe { *CARDS.get_unchecked(card) };
        Self {
            key: self.key.wrapping_add(k),
            mask: self.mask.wrapping_add(m),
        }
    }

    /// Returns a new hand with `card` removed.
    ///
    /// `card` must be in the range `[0, 51]` and must currently be present.
    #[inline]
    pub fn remove_card(&self, card: usize) -> Self {
        let (k, m) = unsafe { *CARDS.get_unchecked(card) };
        Self {
            key: self.key.wrapping_sub(k),
            mask: self.mask.wrapping_sub(m),
        }
    }
}

impl Add for Hand {
    type Output = Self;
    #[inline]
    fn add(self, rhs: Self) -> Self::Output {
        Self {
            key: self
                .key
                .wrapping_add(rhs.key)
                .wrapping_sub(0x3333 << SUIT_SHIFT),
            mask: self.mask.wrapping_add(rhs.mask),
        }
    }
}

impl AddAssign for Hand {
    #[inline]
    fn add_assign(&mut self, rhs: Self) {
        self.key = self.key.wrapping_add(rhs.key);
        self.key = self.key.wrapping_sub(0x3333 << SUIT_SHIFT);
        self.mask = self.mask.wrapping_add(rhs.mask);
    }
}

impl Default for Hand {
    fn default() -> Self {
        Self::new()
    }
}

/// Evaluates a 5-to-7-card hand via the supplied lookup tables.
///
/// `lookup` is the offset-indexed rank table (perfect-hashed over
/// `Hand::get_key()`); `lookup_flush` is indexed by the 13-bit flush rank
/// pattern. Both tables are produced by the `scripts/02-lookup_tables`
/// generator and shipped per variant (`phe-holdem-assets`,
/// `phe-deuce-seven-assets`, ...).
///
/// The interpretation of the returned `u16` (higher = stronger vs. lower =
/// stronger) is variant-defined. The caller is responsible for wrapping
/// with `std::cmp::Reverse` when appropriate.
///
/// # Safety
///
/// Behavior is undefined when `hand.len()` is outside `5..=7`.
#[inline]
pub fn evaluate_via_lookup(hand: &Hand, lookup: &[u16], lookup_flush: &[u16]) -> u16 {
    let is_flush = hand.key & FLUSH_MASK;
    if is_flush > 0 {
        let flush_key = (hand.mask >> (4 * is_flush.leading_zeros())) as u16;
        unsafe { *lookup_flush.get_unchecked(flush_key as usize) }
    } else {
        let rank_key = hand.key as u32 as usize;
        let offset = unsafe { *OFFSETS.get_unchecked(rank_key >> OFFSET_SHIFT) as usize };
        let hash_key = rank_key.wrapping_add(offset);
        unsafe { *lookup.get_unchecked(hash_key) }
    }
}
