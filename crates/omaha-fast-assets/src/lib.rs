//! PLO4 perfect-hash lookup tables, ported from
//! HenryRLee/PokerHandEvaluator `cpp/src/tables_plo4.c`.
//!
//! Two tables (~30 MB linked total):
//! - `FLUSH_PLO4`:    4 099 095 × u16 (~8.2 MB)
//! - `NOFLUSH_PLO4`: 11 238 500 × u16 (~22.5 MB)
//!
//! Both store Cactus-Kev hand ranks in `[1, 7462]` where lower =
//! stronger. The caller (`phe-omaha-fast`) does the higher-better u16
//! conversion at the trait boundary.

pub mod flush_plo4;
pub mod noflush_plo4;

pub use flush_plo4::FLUSH_PLO4;
pub use noflush_plo4::NOFLUSH_PLO4;
