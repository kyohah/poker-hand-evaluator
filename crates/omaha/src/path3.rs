//! Path 3: flush eligible AND board has a pair.
//!
//! Invariants on entry:
//!   - `flush_suit = Some(suit)` → at least one 5-card flush combo is
//!     reachable, so the answer is **at least** a Flush.
//!   - Board has at least one rank pair → Full House and Quads are
//!     reachable (Path 2 ruled out the no-board-pair case).
//!
//! Under these invariants, only four categories can possibly win:
//!
//!   - Straight Flush (cat 8)
//!   - Four of a Kind  (cat 7)
//!   - Full House      (cat 6)
//!   - Flush           (cat 5) — guaranteed reachable
//!
//! Straight (cat 4) and lower are dominated by the guaranteed Flush
//! and never need to be evaluated.
//!
//! Strategy: compute each candidate independently with category-direct
//! logic, take the `u16` max. The 9-card direct path-2 evaluator
//! returns `max(SF, plain Flush)` in one call; on top of that we add
//! O(13) Quads scan + O(13×13) FH scan over rank counts. **No
//! 60-combo enumeration.**

use phe_core::RANK_BASES;
use phe_holdem::assets::LOOKUP_FLUSH;

use crate::lookup_5card::{LOOKUP_5C, OFFSETS_5C, OFFSET_SHIFT_5C};

/// 10 straight-flush 5-rank windows, descending strength
/// (royal → wheel). Same constant as `path2::SF_WINDOWS_DESC`,
/// re-declared here so path 3's hot path doesn't depend on path 2's
/// module boundary (and any inlining decisions the compiler makes
/// at it).
const SF_WINDOWS_DESC: [u16; 10] = [
    0b1_1111_0000_0000, // T-J-Q-K-A (royal)
    0b0_1111_1000_0000, // 9-T-J-Q-K
    0b0_0111_1100_0000, // 8-9-T-J-Q
    0b0_0011_1110_0000, // 7-8-9-T-J
    0b0_0001_1111_0000, // 6-7-8-9-T
    0b0_0000_1111_1000, // 5-6-7-8-9
    0b0_0000_0111_1100, // 4-5-6-7-8
    0b0_0000_0011_1110, // 3-4-5-6-7
    0b0_0000_0001_1111, // 2-3-4-5-6
    0b1_0000_0000_1111, // A-2-3-4-5 (wheel)
];

/// Returns max(SF, plain Flush) for the given suit-restricted hole
/// and board rank bitmasks. Equivalent to `path2::evaluate` but
/// inlined here so the per-suit pass over hole/board can be folded
/// into path 3's per-rank pass (avoiding a redundant scan of all
/// 9 cards).
#[inline]
fn sf_or_flush(hole_mask: u16, board_mask: u16) -> u16 {
    debug_assert!(hole_mask.count_ones() >= 2 && board_mask.count_ones() >= 3);
    for &window in &SF_WINDOWS_DESC {
        if (hole_mask & window).count_ones() >= 2 && (board_mask & window).count_ones() >= 3 {
            return unsafe { *LOOKUP_FLUSH.get_unchecked(window as usize) };
        }
    }
    let h1 = 1u16 << (15 - hole_mask.leading_zeros());
    let m2 = hole_mask ^ h1;
    let h2 = 1u16 << (15 - m2.leading_zeros());
    let b1 = 1u16 << (15 - board_mask.leading_zeros());
    let bm2 = board_mask ^ b1;
    let b2 = 1u16 << (15 - bm2.leading_zeros());
    let bm3 = bm2 ^ b2;
    let b3 = 1u16 << (15 - bm3.leading_zeros());
    let flush_key = h1 | h2 | b1 | b2 | b3;
    unsafe { *LOOKUP_FLUSH.get_unchecked(flush_key as usize) }
}

/// Rank-only lookup driven by a pre-summed `rank_key` (the lower 32
/// bits of `Hand::get_key()`). Caller must ensure `rank_key` is the
/// sum of `RANK_BASES[rank]` over **5** cards forming a non-flush
/// 5-card hand.
///
/// Uses the compact 5-card-only perfect-hash tables in
/// `lookup_5card` (~32 KB total → fits Alder Lake P-core L1d), as
/// opposed to the production 5/6/7-card `phe-core::OFFSETS` +
/// `phe-holdem-assets::LOOKUP` (~192 KB).
#[inline]
fn evaluate_rank_only_from_key(rank_key: u32) -> u16 {
    let rk = rank_key as usize;
    unsafe {
        let offset = *OFFSETS_5C.get_unchecked(rk >> OFFSET_SHIFT_5C) as usize;
        *LOOKUP_5C.get_unchecked(rk.wrapping_add(offset))
    }
}

/// Returns the highest set bit position (`0..15`) of a non-zero
/// `u16`, or `None` for `0`. Used to pick the highest rank in a
/// rank bitmask.
#[inline]
fn highest_bit(mask: u16) -> Option<usize> {
    if mask == 0 {
        None
    } else {
        Some(15 - mask.leading_zeros() as usize)
    }
}

/// Per-side rank bitmasks at threshold counts: bit `r` is set iff the
/// rank has at least the indicated number of cards on that side.
#[derive(Clone, Copy)]
struct RankMasks {
    h_ge_1: u16,
    h_ge_2: u16,
    b_ge_1: u16,
    b_ge_2: u16,
    b_ge_3: u16,
}

#[inline]
fn build_masks(h: &[u8; 13], b: &[u8; 13]) -> RankMasks {
    let mut m = RankMasks {
        h_ge_1: 0,
        h_ge_2: 0,
        b_ge_1: 0,
        b_ge_2: 0,
        b_ge_3: 0,
    };
    for r in 0..13 {
        let hr = h[r];
        let br = b[r];
        if hr >= 1 {
            m.h_ge_1 |= 1u16 << r;
        }
        if hr >= 2 {
            m.h_ge_2 |= 1u16 << r;
        }
        if br >= 1 {
            m.b_ge_1 |= 1u16 << r;
        }
        if br >= 2 {
            m.b_ge_2 |= 1u16 << r;
        }
        if br >= 3 {
            m.b_ge_3 |= 1u16 << r;
        }
    }
    m
}

/// Best Quads (cat 7) reachable under Omaha's 2+3 rule, or `None`.
///
/// 4 cards of rank `r` come from `hole_r_used + board_r_used = 4`
/// with `hole_r_used ≤ 2`, `board_r_used ≤ 3`. Two reachable cases:
///
///   Case A: `h[r] ≥ 1 ∧ b[r] ≥ 3` — board trips + 1 hole; kicker
///           is the highest non-`r` hole rank.
///   Case B: `h[r] ≥ 2 ∧ b[r] ≥ 2` — hole pocket pair + board pair;
///           kicker is the highest non-`r` board rank.
///
/// Implemented with bit-mask arithmetic — no per-rank scan loop.
/// `cand_a | cand_b` identifies all `r` where Quads is reachable;
/// the highest bit picks the best `r`. Within that `r`, both cases'
/// kicker candidates are unioned and the highest bit picks the best
/// kicker (since LOOKUP ranks Quads by `(rank_R, rank_kicker)`
/// lexicographically).
#[inline]
fn check_quads(m: RankMasks) -> Option<u16> {
    let cand_a = m.h_ge_1 & m.b_ge_3;
    let cand_b = m.h_ge_2 & m.b_ge_2;
    let cand = cand_a | cand_b;
    let r = highest_bit(cand)?;
    let r_bit = 1u16 << r;

    // Union of kicker candidates from each applicable case.
    let mut kicker_mask: u16 = 0;
    if cand_a & r_bit != 0 {
        kicker_mask |= m.h_ge_1 & !r_bit;
    }
    if cand_b & r_bit != 0 {
        kicker_mask |= m.b_ge_1 & !r_bit;
    }
    let kicker =
        highest_bit(kicker_mask).expect("kicker mask must be non-empty when quads is reachable");

    let rank_key = 4 * RANK_BASES[r] as u32 + RANK_BASES[kicker] as u32;
    Some(evaluate_rank_only_from_key(rank_key))
}

/// Best Full House (cat 6) reachable under Omaha's 2+3 rule, or
/// `None`.
///
/// FH has trips of `R1` + pair of `R2` (R1 ≠ R2) using exactly 2 hole
/// + 3 board. Three composition cases:
///
/// I. `(0, 3, 2, 0)`: board trips R1, hole pocket pair R2.
///    Needs `b[R1] ≥ 3` AND `h[R2] ≥ 2`.
/// II. `(1, 2, 1, 1)`: hole has 1×R1, 1×R2; board has 2×R1, ≥1×R2.
///    Needs `h[R1] ≥ 1 ∧ b[R1] ≥ 2 ∧ h[R2] ≥ 1 ∧ b[R2] ≥ 1`.
/// III. `(2, 1, 0, 2)`: hole pocket pair R1; board has ≥1×R1, ≥2×R2.
///    Needs `h[R1] ≥ 2 ∧ b[R1] ≥ 1 ∧ b[R2] ≥ 2`.
///
/// Best FH = max `R1`, then max `R2` (Hold'em FH is ranked by trips
/// then pair).
///
/// Implemented as bit-mask arithmetic: build per-case `R1` candidate
/// masks, OR them, take the highest bit; for that `R1`, union the
/// per-case `R2` candidate masks (each gated by whether that case
/// is applicable to this `R1`), take the highest bit. No per-rank
/// or per-pair scan loop.
#[inline]
fn check_fh(m: RankMasks) -> Option<u16> {
    let cand_i_r1 = m.b_ge_3;
    let cand_ii_r1 = m.h_ge_1 & m.b_ge_2;
    // Case III also requires `b[R1] ≥ 1` so the trips can include
    // one board card of rank R1 (otherwise h_R1=2 alone gives a
    // pair, not trips, and we can't add a board R1 from nothing).
    let cand_iii_r1 = m.h_ge_2 & m.b_ge_1;
    let r1_mask = cand_i_r1 | cand_ii_r1 | cand_iii_r1;
    let r1 = highest_bit(r1_mask)?;
    let r1_bit = 1u16 << r1;

    let mut r2_mask: u16 = 0;
    if cand_i_r1 & r1_bit != 0 {
        r2_mask |= m.h_ge_2 & !r1_bit;
    }
    if cand_ii_r1 & r1_bit != 0 {
        r2_mask |= m.h_ge_1 & m.b_ge_1 & !r1_bit;
    }
    if cand_iii_r1 & r1_bit != 0 {
        r2_mask |= m.b_ge_2 & !r1_bit;
    }
    let r2 = highest_bit(r2_mask)?;

    let rank_key = 3 * RANK_BASES[r1] as u32 + 2 * RANK_BASES[r2] as u32;
    Some(evaluate_rank_only_from_key(rank_key))
}

/// Path-3 entry point: max(SF, Quads, FH, Flush).
///
/// Single pass over the 9 cards builds per-rank counts AND per-suit
/// rank bitmasks for the flush suit at once. Then `sf_or_flush`,
/// `check_quads`, `check_fh` work off pre-built bitmasks.
#[inline]
pub(crate) fn evaluate(hole: &[usize; 4], board: &[usize; 5], suit: u8) -> u16 {
    let suit_u = suit as usize;

    let mut h = [0u8; 13];
    let mut b = [0u8; 13];
    let mut hole_mask: u16 = 0;
    let mut board_mask: u16 = 0;
    for &c in hole {
        let r = c / 4;
        h[r] += 1;
        if c & 3 == suit_u {
            hole_mask |= 1u16 << r;
        }
    }
    for &c in board {
        let r = c / 4;
        b[r] += 1;
        if c & 3 == suit_u {
            board_mask |= 1u16 << r;
        }
    }

    let mut best = sf_or_flush(hole_mask, board_mask);
    let masks = build_masks(&h, &b);
    if let Some(q) = check_quads(masks) {
        if q > best {
            best = q;
        }
    }
    if let Some(f) = check_fh(masks) {
        if f > best {
            best = f;
        }
    }
    best
}
