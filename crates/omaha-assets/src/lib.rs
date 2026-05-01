//! Omaha high lookup tables.
//!
//! `NOFLUSH_LOOKUP` is the path-1 (no-flush) direct table: indexed by
//! a flat `hole_index * NUM_BOARD + board_index` key over rank
//! multisets, it returns the best 60-combo non-flush rank in a single
//! `u16` access.
//!
//! Generated at build time by `build.rs` (the algorithm mirrors
//! `scripts/gen-omaha-noflush-lookup`, which is kept around as a
//! standalone tool for one-shot debugging). Output written to
//! `OUT_DIR/noflush_lookup.bin` and embedded via `include_bytes!`
//! through a `repr(C)` aligned wrapper so it can be soundly
//! reinterpreted as `&[u16; N]`.

/// Number of distinct 4-card rank multisets over 13 ranks =
/// `C(13 + 4 - 1, 4) = C(16, 4) = 1820`. Hole histogram index range.
pub const NUM_HOLE: usize = 1820;

/// Number of distinct 5-card rank multisets over 13 ranks =
/// `C(13 + 5 - 1, 5) = C(17, 5) = 6188`. Board histogram index range.
/// (13 of these — the "5 of one rank" multisets — are unreachable
/// because a real deck has only 4 cards per rank, so they're filled
/// with 0 in the table.)
pub const NUM_BOARD: usize = 6188;

/// Total table length = `NUM_HOLE * NUM_BOARD`.
pub const NOFLUSH_LOOKUP_LEN: usize = NUM_HOLE * NUM_BOARD;

const BLOB_SIZE: usize = NOFLUSH_LOOKUP_LEN * 2;

/// `repr(C)` wrapper that forces 2-byte alignment on the included
/// blob, so we can soundly reinterpret it as `&[u16]`.
#[repr(C)]
struct Aligned<T: ?Sized> {
    _align: [u16; 0],
    bytes: T,
}

static ALIGNED: &Aligned<[u8; BLOB_SIZE]> = &Aligned {
    _align: [],
    bytes: *include_bytes!(concat!(env!("OUT_DIR"), "/noflush_lookup.bin")),
};

/// Path-1 no-flush direct lookup. Key = `hole_idx * NUM_BOARD + board_idx`.
///
/// `hole_idx` and `board_idx` are multiset combinatorial-number-system
/// indices computed from the sorted rank lists; see `phe-omaha` for
/// the encoder. Per-rank deck constraint (≤4 cards per rank across
/// hole + board) means some `(hole_idx, board_idx)` pairs are
/// unreachable; those slots are zero in the binary blob.
pub fn noflush_lookup() -> &'static [u16; NOFLUSH_LOOKUP_LEN] {
    // SAFETY: ALIGNED is 2-byte aligned via the [u16; 0] field; the
    // blob is exactly NOFLUSH_LOOKUP_LEN * 2 bytes by construction
    // (compile-time array size match); the generator writes
    // little-endian u16, which matches native ordering on every host
    // we target.
    unsafe { &*(ALIGNED.bytes.as_ptr() as *const [u16; NOFLUSH_LOOKUP_LEN]) }
}
