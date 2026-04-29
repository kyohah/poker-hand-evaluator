//! Hold'em high evaluator (5-7 cards).
//!
//! Ported from b-inary/holdem-hand-evaluator (MIT). The shared `Hand`
//! mechanics live in [`phe_core`]; this crate ships the lookup tables
//! plus Hold'em-specific category logic, enumeration, and heads-up
//! win-frequency utilities.

mod category;
mod enumerate;
mod eval;
mod heads_up;
mod parse;

pub use category::{get_hand_category, HandCategory};
pub use enumerate::enumerate_hand_category;
pub use eval::HighRule;
pub use heads_up::heads_up_win_frequency;
pub use parse::parse_hand;

/// Raw lookup tables (`LOOKUP`, `LOOKUP_FLUSH`, `HEADS_UP_WIN_FREQUENCY`).
///
/// Re-exported so downstream callers (notably `phe-omaha`'s suit-aware
/// fast paths and any GPU / SIMD specialisations) can build their own
/// access patterns without taking a direct dependency on
/// `phe-holdem-assets`.
pub use phe_holdem_assets as assets;
