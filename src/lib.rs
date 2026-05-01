//! Unified facade for the poker-hand-evaluator workspace.
//!
//! Exposes the [`HandRule`] trait, a feature-gated re-export of every
//! variant rule, and the [`HiLoRule`] composite for high-low split
//! games (8-or-better, Stud Hi-Lo, etc.).
//!
//! ## Card encoding (trait surface)
//!
//! `cards: &[u8]` uses the **Hold'em-style** encoding — `card = rank *
//! 4 + suit`, with rank `0='2', ..., 12='A'` (Ace high) and suit
//! `0=club, 1=diamond, 2=heart, 3=spade`. Variant crates whose internal
//! encoding differs (specifically `phe-eight-low`, where Ace is rank 0)
//! translate at the trait boundary so downstream code only ever needs
//! to think about one encoding.
//!
//! ## Strength contract
//!
//! `Strength: Ord + Copy + Send + Sync`, with **higher = stronger**.
//! Low-hand rules wrap their raw rank in `std::cmp::Reverse` so the
//! contract holds (smaller raw rank = stronger low hand).

#![warn(missing_docs)]
#![cfg_attr(docsrs, feature(doc_cfg))]

// Sub-crate re-exports. These keep raw access to the underlying types
// (Hand, lookup tables, encoding constants) available to downstream
// callers — most notably GPU code that builds device-side copies of
// LOOKUP / OFFSETS / CARDS — without forcing them to depend on each
// internal crate by hand.
//
// **CUDA backend.** Enable the facade-level `cuda` feature to activate
// the NVRTC kernel on every variant that ships one. Reach the per-rule
// context as `poker_hand_evaluator::holdem::cuda::HoldemEvalContext`
// (and similarly `omaha::cuda` once that's wired). The kernel and
// device-resident lookup tables sit behind `phe-holdem/cuda` /
// `phe-omaha/cuda` per-variant features; the facade just propagates
// them via the weak-feature `?/cuda` syntax so disabling a variant
// silently skips its CUDA backend.
#[cfg(feature = "badugi")]
pub use phe_badugi as badugi;
pub use phe_core as core;
#[cfg(feature = "deuce-seven")]
pub use phe_deuce_seven as deuce_seven;
#[cfg(feature = "eight-low")]
pub use phe_eight_low as eight_low;
#[cfg(feature = "holdem")]
pub use phe_holdem as holdem;
#[cfg(feature = "omaha")]
pub use phe_omaha as omaha;

/// A rule that can score a poker hand.
///
/// Implementations are typically zero-sized unit structs (`HighRule`,
/// `DeuceSevenLowRule`, ...) so values are essentially free; pass them
/// by value into composites like [`HiLoRule`].
///
/// # Examples
///
/// ```
/// use poker_hand_evaluator::{HandRule, HighRule};
/// // 7-card eval — royal flush in spades + two off-suit junkers.
/// let cards = [
///     12 * 4 + 3, // A♠
///     11 * 4 + 3, // K♠
///     10 * 4 + 3, // Q♠
///     9 * 4 + 3,  // J♠
///     8 * 4 + 3,  // T♠
///     0 * 4 + 0,  // 2♣
///     0 * 4 + 1,  // 2♦
/// ];
/// assert!(HighRule.evaluate(&cards) > 0);
/// ```
pub trait HandRule: Send + Sync {
    /// Strength type returned by [`evaluate`](Self::evaluate). Must
    /// be totally ordered (higher = stronger) so callers can compare,
    /// sort, or `max` strengths without further conversion. `Reverse`
    /// is the standard wrapper for low-rule variants where the
    /// underlying lookup returns "lower = stronger".
    type Strength: Ord + Copy + Send + Sync;

    /// Returns the strength of the hand formed from `cards`.
    ///
    /// The expected length and content of `cards` is variant-specific
    /// (e.g. 5..=7 for Hold'em high, exactly 9 for Omaha as
    /// `[hole_0..hole_3, board_0..board_4]`). Out-of-contract input
    /// is undefined behaviour; the implementer documents the exact
    /// contract.
    fn evaluate(&self, cards: &[u8]) -> Self::Strength;
}

#[cfg(feature = "holdem")]
pub use phe_holdem::HighRule;

#[cfg(feature = "holdem")]
impl HandRule for HighRule {
    type Strength = u16;

    fn evaluate(&self, cards: &[u8]) -> u16 {
        let mut h = phe_core::Hand::new();
        for &c in cards {
            h = h.add_card(c as usize);
        }
        phe_holdem::HighRule::evaluate(&h)
    }
}

#[cfg(feature = "eight-low")]
pub use phe_eight_low::{AceFiveLowRule, EightLowQualifiedRule};

/// Translates Hold'em-style card id (rank 0=2, 12=A) into the
/// phe-eight-low internal encoding (rank 0=A, 12=K).
#[cfg(feature = "eight-low")]
#[inline]
fn holdem_to_eight_low(c: u8) -> u8 {
    if c / 4 == 12 {
        c % 4 // Ace -> rank 0
    } else {
        c + 4 // shift everything else up by one rank
    }
}

#[cfg(feature = "eight-low")]
fn build_eight_low_hand(cards: &[u8]) -> phe_eight_low::Hand {
    let mut h = phe_eight_low::Hand::new();
    for &c in cards {
        h = h.add_card(holdem_to_eight_low(c) as usize);
    }
    h
}

#[cfg(feature = "eight-low")]
impl HandRule for EightLowQualifiedRule {
    type Strength = Option<std::cmp::Reverse<u16>>;

    fn evaluate(&self, cards: &[u8]) -> Self::Strength {
        let h = build_eight_low_hand(cards);
        phe_eight_low::EightLowQualifiedRule::evaluate(&h)
    }
}

#[cfg(feature = "eight-low")]
impl HandRule for AceFiveLowRule {
    type Strength = std::cmp::Reverse<u16>;

    fn evaluate(&self, cards: &[u8]) -> Self::Strength {
        let h = build_eight_low_hand(cards);
        phe_eight_low::AceFiveLowRule::evaluate(&h)
    }
}

#[cfg(feature = "deuce-seven")]
pub use phe_deuce_seven::DeuceSevenLowRule;

#[cfg(feature = "deuce-seven")]
impl HandRule for DeuceSevenLowRule {
    type Strength = std::cmp::Reverse<u16>;

    fn evaluate(&self, cards: &[u8]) -> Self::Strength {
        let mut h = phe_core::Hand::new();
        for &c in cards {
            h = h.add_card(c as usize);
        }
        phe_deuce_seven::DeuceSevenLowRule::evaluate(&h)
    }
}

#[cfg(feature = "omaha")]
pub use phe_omaha::OmahaHighRule;

#[cfg(feature = "omaha")]
impl HandRule for OmahaHighRule {
    type Strength = u16;

    /// `cards` must be exactly 9 entries: the first 4 are hole cards,
    /// the last 5 are board cards.
    fn evaluate(&self, cards: &[u8]) -> u16 {
        assert_eq!(
            cards.len(),
            9,
            "OmahaHighRule expects 4 hole + 5 board = 9 cards, got {}",
            cards.len()
        );
        let hole = [
            cards[0] as usize,
            cards[1] as usize,
            cards[2] as usize,
            cards[3] as usize,
        ];
        let board = [
            cards[4] as usize,
            cards[5] as usize,
            cards[6] as usize,
            cards[7] as usize,
            cards[8] as usize,
        ];
        phe_omaha::OmahaHighRule::evaluate(&hole, &board)
    }
}

#[cfg(feature = "badugi")]
pub use phe_badugi::{BadugiRule, BadugiStrength};

#[cfg(feature = "badugi")]
impl HandRule for BadugiRule {
    type Strength = BadugiStrength;

    fn evaluate(&self, cards: &[u8]) -> BadugiStrength {
        assert_eq!(
            cards.len(),
            4,
            "BadugiRule expects 4 cards, got {}",
            cards.len()
        );
        phe_badugi::BadugiRule::evaluate([cards[0], cards[1], cards[2], cards[3]])
    }
}

/// Composite rule for high-low split games.
///
/// `Strength = (H::Strength, L::Strength)`. Tuple comparison is
/// lexicographic, which by itself is **not** the right ordering for
/// split-pot games; this is just a convenience for callers that want
/// both scores in one shot. Callers are responsible for split-pot
/// awarding.
#[derive(Default, Clone, Copy, Debug)]
pub struct HiLoRule<H: HandRule, L: HandRule> {
    /// High-side rule (e.g. [`HighRule`] for Hold'em hi/lo).
    pub hi: H,
    /// Low-side rule (e.g. [`EightLowQualifiedRule`] for Omaha 8-or-better).
    pub lo: L,
}

impl<H: HandRule, L: HandRule> HandRule for HiLoRule<H, L> {
    type Strength = (H::Strength, L::Strength);

    fn evaluate(&self, cards: &[u8]) -> Self::Strength {
        (self.hi.evaluate(cards), self.lo.evaluate(cards))
    }
}
