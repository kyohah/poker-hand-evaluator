//! PLO4 perfect-hash lookup tables.
//!
//! Generated at build time by `build.rs` from
//! HenryRLee/PokerHandEvaluator algorithmic primitives:
//!
//! * `NO_FLUSH_5` (6 175 × u16 Cactus-Kev rank, 5-card non-flush
//!   evaluator) — shipped as the `no_flush_5.bin` source blob.
//! * `FLUSH_5`    (8 192 × u16 Cactus-Kev rank, 5-card flush
//!   evaluator) — shipped as the `flush_5.bin` source blob.
//!
//! From these `build.rs` enumerates every reachable PLO sub-hand
//! pattern, runs the 60-combo "best of 2 hole + 3 board" selection,
//! and produces:
//!
//! * `FLUSH_PLO4`    ( 4 099 095 × u16, ~8.2 MB) → `OUT_DIR/flush_plo4.bin`
//! * `NOFLUSH_PLO4`  (11 238 500 × u16, ~22.5 MB) → `OUT_DIR/noflush_plo4.bin`
//!
//! Both store Cactus-Kev hand ranks in `[1, 7462]` (lower =
//! stronger). The runtime consumer (`phe-omaha`) does the
//! u16 higher-better conversion at the `HandRule` trait boundary.
//!
//! Switching from committed textual `pub const` arrays (~90 MB
//! source-tree footprint, no longer in repo) to build-time
//! generation cuts the repo and keeps a single algorithmic source
//! of truth. Verified byte-for-byte against the previous textual
//! data before the textual files were removed.

/// Forces the `include_bytes!` blob to be 2-byte aligned so it can
/// be safely reinterpreted as `&[u16; N]`.
#[repr(C)]
struct Aligned<T: ?Sized> {
    _align: [u16; 0],
    bytes: T,
}

// ------------------------------------------------------------------
// Big PLO4 tables — generated at build time, included from OUT_DIR.
// ------------------------------------------------------------------

pub const FLUSH_PLO4_LEN: usize = 4_099_095;
pub const NOFLUSH_PLO4_LEN: usize = 11_238_500;

const FLUSH_PLO4_BYTES: usize = FLUSH_PLO4_LEN * 2;
const NOFLUSH_PLO4_BYTES: usize = NOFLUSH_PLO4_LEN * 2;

static ALIGNED_FLUSH_PLO4: &Aligned<[u8; FLUSH_PLO4_BYTES]> = &Aligned {
    _align: [],
    bytes: *include_bytes!(concat!(env!("OUT_DIR"), "/flush_plo4.bin")),
};

static ALIGNED_NOFLUSH_PLO4: &Aligned<[u8; NOFLUSH_PLO4_BYTES]> = &Aligned {
    _align: [],
    bytes: *include_bytes!(concat!(env!("OUT_DIR"), "/noflush_plo4.bin")),
};

/// Best 5-card flush rank for a (board, hole) PLO sub-hand pattern,
/// indexed by `board_hash * 1365 + hole_hash` (`hash_binary` of the
/// rank-bitmap padded to 5 / 4 bits via the runtime PADDING constants).
///
/// Cactus-Kev rank in `[1, 7462]`, lower = stronger.
//
// SAFETY: `Aligned<[u8; N]>` is 2-byte aligned via the `[u16; 0]`
// field; build.rs writes exactly `N * 2` bytes of little-endian
// u16, matching native byte order on every host we target.
pub static FLUSH_PLO4: &[u16; FLUSH_PLO4_LEN] =
    unsafe { &*(ALIGNED_FLUSH_PLO4.bytes.as_ptr() as *const [u16; FLUSH_PLO4_LEN]) };

/// Best 5-card non-flush rank for a (board, hole) PLO multiset
/// pattern, indexed by `board_hash * 1820 + hole_hash`
/// (`hash_quinary` of the rank-count histogram).
///
/// Cactus-Kev rank in `[1, 7462]`, lower = stronger.
pub static NOFLUSH_PLO4: &[u16; NOFLUSH_PLO4_LEN] =
    unsafe { &*(ALIGNED_NOFLUSH_PLO4.bytes.as_ptr() as *const [u16; NOFLUSH_PLO4_LEN]) };

// ------------------------------------------------------------------
// Small 5-card eval tables — also exposed in case downstream
// crates want them (e.g., the `phe-omaha` flush_5card module).
// ------------------------------------------------------------------

pub const NO_FLUSH_5_LEN: usize = 6175;
pub const FLUSH_5_LEN: usize = 8192;

static ALIGNED_NO_FLUSH_5: &Aligned<[u8; NO_FLUSH_5_LEN * 2]> = &Aligned {
    _align: [],
    bytes: *include_bytes!("no_flush_5.bin"),
};
static ALIGNED_FLUSH_5: &Aligned<[u8; FLUSH_5_LEN * 2]> = &Aligned {
    _align: [],
    bytes: *include_bytes!("flush_5.bin"),
};

/// 5-card non-flush rank table, indexed by `hash_quinary(q, 5)`.
pub static NO_FLUSH_5: &[u16; NO_FLUSH_5_LEN] =
    unsafe { &*(ALIGNED_NO_FLUSH_5.bytes.as_ptr() as *const [u16; NO_FLUSH_5_LEN]) };

/// 5-card flush rank table, indexed by 13-bit rank pattern.
pub static FLUSH_5: &[u16; FLUSH_5_LEN] =
    unsafe { &*(ALIGNED_FLUSH_5.bytes.as_ptr() as *const [u16; FLUSH_5_LEN]) };
