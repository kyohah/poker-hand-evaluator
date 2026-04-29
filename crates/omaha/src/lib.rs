//! Omaha high evaluator.
//!
//! In Omaha each player has 4 hole cards and 5 community-board cards,
//! and **must** use exactly 2 hole cards and 3 board cards. The best
//! 5-card hand is the maximum-Hold'em-rank choice over the
//! `C(4,2) * C(5,3) = 6 * 10 = 60` candidate combinations.
//!
//! ## Optimisations
//!
//! Three dispatch paths, picked at the top of `evaluate` from suit
//! counts and a board-pair check:
//!
//! 1. **No-flush path.** No suit has both ≥2 hole and ≥3 board cards
//!    → flush is unreachable for every combo. The 60 combos use a
//!    rank-only inner lookup that skips the FLUSH_MASK check + the
//!    `LOOKUP_FLUSH` access entirely.
//!
//! 2. **Flush-dominates path.** A flush is reachable AND the board
//!    has 5 distinct ranks (i.e., no pair on board). On a no-pair
//!    board, FullHouse and Quads are mathematically unreachable for
//!    *any* combo, so the best 5-card hand is the best flush. We
//!    enumerate only the suit-restricted combos (`C(hole_s, 2) ×
//!    C(board_s, 3)`, often as few as 1) and skip the other 50+
//!    combos that can only produce HighCard / Pair / TwoPair / Trips
//!    / Straight — all dominated by the flush. Straight-Flush is
//!    captured automatically: the same `HighRule` lookup returns the
//!    SF rank when 5 of the chosen cards are sequential.
//!
//! 3. **Full path.** Flush reachable AND board has a pair (so FH /
//!    Quads might still win). All 60 combos go through the full
//!    `HighRule::evaluate` with the flush dispatch. Board-partial
//!    caching still applies.
//!
//! All three paths share board-partial precomputation so the inner
//! 5-card Hand is built as `hole_pair + board_partial` (one Hand
//! addition) instead of three `add_card` calls per combo.

use phe_core::{OFFSETS, OFFSET_SHIFT, RANK_BASES};
use phe_holdem::assets::{LOOKUP, LOOKUP_FLUSH};

mod kev;
mod kev_tables;
pub use kev::{
    eval_5cards_kev, eval_5cards_kev_v0, eval_5cards_kev_v1_precomp,
    eval_5cards_kev_v2_always_flush, eval_5cards_kev_v3_always_hash, kev_rank_to_packed,
    KEV_CARDS,
};

/// Omaha high rule.
///
/// `Strength = u16` (higher = stronger), reusing the Hold'em packing
/// scheme: bits 12-15 hold the [`phe_holdem::HandCategory`], bits 0-11
/// the within-category index.
pub struct OmahaHighRule;

const HOLE_PAIRS: [(usize, usize); 6] =
    [(0, 1), (0, 2), (0, 3), (1, 2), (1, 3), (2, 3)];

const BOARD_TRIPLES: [(usize, usize, usize); 10] = [
    (0, 1, 2),
    (0, 1, 3),
    (0, 1, 4),
    (0, 2, 3),
    (0, 2, 4),
    (0, 3, 4),
    (1, 2, 3),
    (1, 2, 4),
    (1, 3, 4),
    (2, 3, 4),
];

/// Returns the suit (0..3) with both ≥2 hole and ≥3 board cards (the
/// only suits where any 5-card combo can be a flush). At most one
/// such suit can exist because 5 - 3 < 3 (you can't fit two 3+-suit
/// groups in a 5-card board).
#[inline]
pub fn flush_suit(hole: &[usize; 4], board: &[usize; 5]) -> Option<u8> {
    let mut hole_s = [0u8; 4];
    let mut board_s = [0u8; 4];
    for &c in hole {
        hole_s[c & 3] += 1;
    }
    for &c in board {
        board_s[c & 3] += 1;
    }
    (0u8..4).find(|&s| hole_s[s as usize] >= 2 && board_s[s as usize] >= 3)
}

/// True if at least one (hole_pair, board_triple) combo can be a flush.
/// Equivalent to `flush_suit(hole, board).is_some()`.
#[inline]
pub fn flush_possible(hole: &[usize; 4], board: &[usize; 5]) -> bool {
    flush_suit(hole, board).is_some()
}

/// True if the board has 5 distinct ranks (no two cards share a rank).
/// On such a board, Full House and Four-of-a-Kind are unreachable for
/// *any* (hole_pair, board_triple) combo:
///   - Quads need 4 of one rank in 5 cards → impossible with 0 board
///     pair (max 2 hole + 1 board = 3).
///   - FH needs 3+2; with no board pair, you can only reach 3-of-rank
///     (via 2 hole + 1 board) but never a second rank's pair (would
///     need 1 hole + 1 board for that rank, but both hole cards are
///     spent on the trips).
#[inline]
pub fn board_has_no_pair(board: &[usize; 5]) -> bool {
    let mut seen = 0u16;
    for &c in board {
        let bit = 1u16 << (c / 4);
        if seen & bit != 0 {
            return false;
        }
        seen |= bit;
    }
    true
}

/// True if no 5-card straight can land in *any* (hole_pair,
/// board_triple) combo, regardless of what hole the player holds. A
/// 5-card combo uses exactly 3 board cards, so a straight via the
/// 5-rank window `P` is reachable only when board has ≥ 3 ranks in
/// `P`. We therefore check every 5-rank window (the 9 standard
/// windows starting at ranks 0..8 plus the wheel `{12, 0, 1, 2, 3}`)
/// and return `true` only when *every* window has ≤ 2 board ranks.
#[inline]
pub fn board_no_straight(board: &[usize; 5]) -> bool {
    let mut rank_mask: u16 = 0;
    for &c in board {
        rank_mask |= 1u16 << (c / 4);
    }
    // 9 standard windows: ranks {r, r+1, r+2, r+3, r+4} for r = 0..=8.
    for r in 0u32..=8 {
        let window: u16 = 0b1_1111u16 << r;
        if (rank_mask & window).count_ones() >= 3 {
            return false;
        }
    }
    // Wheel: A-2-3-4-5 = ranks {0, 1, 2, 3, 12}.
    let wheel: u16 = (1 << 0) | (1 << 1) | (1 << 2) | (1 << 3) | (1 << 12);
    if (rank_mask & wheel).count_ones() >= 3 {
        return false;
    }
    true
}

/// Returns the maximum-possible Hold'em-high category bits for a
/// 5-card combo formed from `(h1, h2)` plus 3 board cards, given
/// per-suit board card counts and structural board flags.
///
/// The bound is **safe**: it may over-estimate but must never
/// under-estimate the true max. Used by the branch-and-bound prune
/// in `evaluate` to skip hole pairs that can't beat the running
/// best's category.
///
/// Decision tree (high → low):
///   1. Suited pair AND ≥3 board in that suit → 8 (Straight Flush)
///   2. Pocket pair (same rank in hole) AND board has a pair → 7 (Quads)
///   3. Pocket pair, no board pair                            → 3 (Trips)
///   4. Mixed-rank, board has a pair                          → 6 (Full House)
///   5. Mixed-rank, no board pair, straight reachable         → 4 (Straight)
///   6. Mixed-rank, no board pair, no straight reachable      → 2 (Two Pair)
#[inline]
pub fn upper_bound_category(
    h1: usize,
    h2: usize,
    board_suit_counts: &[u8; 4],
    board_has_pair: bool,
    no_straight: bool,
) -> u8 {
    let r1 = h1 / 4;
    let r2 = h2 / 4;
    let s1 = h1 & 3;
    let s2 = h2 & 3;

    // Case 1: Flush / SF.
    if s1 == s2 && board_suit_counts[s1] >= 3 {
        return 8;
    }

    // Case 2/3: pocket pair (hole cards same rank).
    if r1 == r2 {
        if board_has_pair {
            return 7;
        }
        return 3;
    }

    // Case 4: mixed-rank with paired board.
    if board_has_pair {
        return 6;
    }

    // Case 5/6: mixed-rank, no pair on board.
    if no_straight {
        2
    } else {
        4
    }
}

/// Rank-only lookup driven by a pre-summed `rank_key` (the lower 32
/// bits of `Hand::get_key()`). Skips Hand construction entirely.
/// Caller must guarantee `rank_key = sum of RANK_BASES[rank] for the
/// 5 cards`, that none of those cards form a flush, and that the
/// sum fits in u32 (always true for ≤7 cards).
#[inline]
fn evaluate_rank_only_from_key(rank_key: u32) -> u16 {
    let rk = rank_key as usize;
    unsafe {
        let offset = *OFFSETS.get_unchecked(rk >> OFFSET_SHIFT) as usize;
        *LOOKUP.get_unchecked(rk.wrapping_add(offset))
    }
}

/// Path 3 inner: flush eligible AND board has a pair.
///
/// Both flush and non-flush combos may produce the max (Flush, but
/// also Full House / Quads from the board pair). Each combo's path
/// is decided by counting how many of its 5 cards are in `suit`:
/// 5 → flush (use `LOOKUP_FLUSH` with bit-OR'd ranks); otherwise
/// → rank-only lookup. We pre-sum partials for both paths once and
/// branch per combo.
#[inline]
fn evaluate_flush_with_pair_path(
    hole: &[usize; 4],
    board: &[usize; 5],
    suit: u8,
) -> u16 {
    let suit_u = suit as usize;

    // Per-hole-card precomputations.
    let mut hole_rk = [0u32; 4];
    let mut hole_inc = [0u8; 4]; // 1 if in flush suit, 0 otherwise
    let mut hole_fb = [0u16; 4]; // (1 << rank) if in suit, else 0
    for i in 0..4 {
        let c = hole[i];
        let rank = c / 4;
        hole_rk[i] = RANK_BASES[rank] as u32;
        let in_s = (c & 3) == suit_u;
        hole_inc[i] = in_s as u8;
        hole_fb[i] = if in_s { 1u16 << rank } else { 0 };
    }
    // 6 hole-pair partials.
    let mut pair_rk = [0u32; 6];
    let mut pair_inc = [0u8; 6];
    let mut pair_fb = [0u16; 6];
    for (idx, &(i, j)) in HOLE_PAIRS.iter().enumerate() {
        pair_rk[idx] = hole_rk[i].wrapping_add(hole_rk[j]);
        pair_inc[idx] = hole_inc[i] + hole_inc[j];
        pair_fb[idx] = hole_fb[i] | hole_fb[j];
    }

    // Per-board-card precomputations.
    let mut board_rk = [0u32; 5];
    let mut board_inc = [0u8; 5];
    let mut board_fb = [0u16; 5];
    for i in 0..5 {
        let c = board[i];
        let rank = c / 4;
        board_rk[i] = RANK_BASES[rank] as u32;
        let in_s = (c & 3) == suit_u;
        board_inc[i] = in_s as u8;
        board_fb[i] = if in_s { 1u16 << rank } else { 0 };
    }
    // 10 board-triple partials.
    let mut triple_rk = [0u32; 10];
    let mut triple_inc = [0u8; 10];
    let mut triple_fb = [0u16; 10];
    for (idx, &(a, b, c)) in BOARD_TRIPLES.iter().enumerate() {
        triple_rk[idx] = board_rk[a]
            .wrapping_add(board_rk[b])
            .wrapping_add(board_rk[c]);
        triple_inc[idx] = board_inc[a] + board_inc[b] + board_inc[c];
        triple_fb[idx] = board_fb[a] | board_fb[b] | board_fb[c];
    }

    let mut best: u16 = 0;
    for pi in 0..6usize {
        for ti in 0..10usize {
            // Combo is a 5-card flush iff all 5 cards share `suit`.
            let r = if pair_inc[pi] + triple_inc[ti] == 5 {
                let flush_key = pair_fb[pi] | triple_fb[ti];
                unsafe { *LOOKUP_FLUSH.get_unchecked(flush_key as usize) }
            } else {
                evaluate_rank_only_from_key(pair_rk[pi].wrapping_add(triple_rk[ti]))
            };
            if r > best {
                best = r;
            }
        }
    }
    best
}

/// Path 1 inner: no flush possible across the entire 9-card layout.
///
/// Bypasses the Hand build entirely. Each combo's `rank_key` is the
/// u32 sum of `RANK_BASES[rank]` over the 5 chosen cards; we
/// pre-sum the 6 hole-pair partials and the 10 board-triple
/// partials once, then the inner loop just does
/// `pair_rk + triple_rk` (one `wrapping_add`) before the OFFSETS /
/// LOOKUP indirections.
#[inline]
fn evaluate_no_flush_path(hole: &[usize; 4], board: &[usize; 5]) -> u16 {
    // Pre-sum hole-pair rank keys (6 of them).
    let hole_rk: [u32; 4] = [
        RANK_BASES[hole[0] / 4] as u32,
        RANK_BASES[hole[1] / 4] as u32,
        RANK_BASES[hole[2] / 4] as u32,
        RANK_BASES[hole[3] / 4] as u32,
    ];
    let mut pair_rks = [0u32; 6];
    for (idx, &(i, j)) in HOLE_PAIRS.iter().enumerate() {
        pair_rks[idx] = hole_rk[i].wrapping_add(hole_rk[j]);
    }

    // Pre-sum board-triple rank keys (10 of them).
    let board_rk: [u32; 5] = [
        RANK_BASES[board[0] / 4] as u32,
        RANK_BASES[board[1] / 4] as u32,
        RANK_BASES[board[2] / 4] as u32,
        RANK_BASES[board[3] / 4] as u32,
        RANK_BASES[board[4] / 4] as u32,
    ];
    let mut triple_rks = [0u32; 10];
    for (idx, &(a, b, c)) in BOARD_TRIPLES.iter().enumerate() {
        triple_rks[idx] = board_rk[a]
            .wrapping_add(board_rk[b])
            .wrapping_add(board_rk[c]);
    }

    let mut best: u16 = 0;
    for &pair_rk in &pair_rks {
        for &triple_rk in &triple_rks {
            let r = evaluate_rank_only_from_key(pair_rk.wrapping_add(triple_rk));
            if r > best {
                best = r;
            }
        }
    }
    best
}

/// Flush-dominates path: enumerate only the suit-restricted combos.
///
/// Preconditions: `flush_suit` is `Some(suit)` and the board has no
/// pair. Under these, the best combo is the best flush (or SF), and
/// every other 5-card combo (HighCard..Straight) loses to it.
///
/// Implementation: skip the [`Hand`]-building round-trip and the
/// [`HighRule::evaluate`] flush dispatch. Each chosen combo is 5
/// same-suit cards, so the 13-bit flush key is just
/// `OR (1 << rank_i)` over the 5 cards, looked up directly in
/// `LOOKUP_FLUSH`. The lookup table itself encodes Straight-Flush
/// vs. plain-Flush ranks, so this is correct regardless of whether
/// `board_no_straight` happens to hold (when it does, the lookup
/// just never returns the SF range — no extra logic needed).
fn evaluate_flush_dominate(
    hole: &[usize; 4],
    board: &[usize; 5],
    suit: u8,
) -> u16 {
    let suit_u = suit as usize;

    // Collect rank-bits (1 << rank) of cards in `suit` from hole and board.
    let mut hole_bits = [0u16; 4];
    let mut hh = 0;
    for &c in hole {
        if c & 3 == suit_u {
            hole_bits[hh] = 1u16 << (c / 4);
            hh += 1;
        }
    }
    let mut board_bits = [0u16; 5];
    let mut bb = 0;
    for &c in board {
        if c & 3 == suit_u {
            board_bits[bb] = 1u16 << (c / 4);
            bb += 1;
        }
    }
    debug_assert!(hh >= 2 && bb >= 3);

    // Pre-OR the C(bb, 3) board triples once. Up to 10.
    let mut triples = [0u16; 10];
    let mut tt = 0;
    for a in 0..bb {
        for b in (a + 1)..bb {
            for c in (b + 1)..bb {
                triples[tt] = board_bits[a] | board_bits[b] | board_bits[c];
                tt += 1;
            }
        }
    }

    let mut best: u16 = 0;
    for i in 0..hh {
        for j in (i + 1)..hh {
            let pair = hole_bits[i] | hole_bits[j];
            for &triple in &triples[..tt] {
                let flush_key = pair | triple;
                let r = unsafe { *LOOKUP_FLUSH.get_unchecked(flush_key as usize) };
                if r > best {
                    best = r;
                }
            }
        }
    }
    best
}

/// **Experimental v1**: Cactus-Kev with pre-summed OR/AND/prime partials.
///
/// Pre-computes hole-pair (6) and board-triple (10) partials of the
/// `c1|c2|...`, `c1&c2&...`, and `(c1&0xff) * (c2&0xff) * ...` reductions
/// outside the inner loop. Used to isolate per-combo arithmetic cost in
/// the Cactus-Kev kernel for the perf investigation.
pub fn evaluate_kev_v1(hole: &[usize; 4], board: &[usize; 5]) -> u16 {
    let kh: [u32; 4] = [
        KEV_CARDS[hole[0]], KEV_CARDS[hole[1]], KEV_CARDS[hole[2]], KEV_CARDS[hole[3]],
    ];
    let kb: [u32; 5] = [
        KEV_CARDS[board[0]], KEV_CARDS[board[1]], KEV_CARDS[board[2]],
        KEV_CARDS[board[3]], KEV_CARDS[board[4]],
    ];

    // 6 hole-pair partials
    let mut pair_or = [0u32; 6];
    let mut pair_and = [0u32; 6];
    let mut pair_prime = [0u32; 6];
    for (idx, &(i, j)) in HOLE_PAIRS.iter().enumerate() {
        pair_or[idx] = kh[i] | kh[j];
        pair_and[idx] = kh[i] & kh[j];
        pair_prime[idx] = (kh[i] & 0xff).wrapping_mul(kh[j] & 0xff);
    }
    // 10 board-triple partials
    let mut tri_or = [0u32; 10];
    let mut tri_and = [0u32; 10];
    let mut tri_prime = [0u32; 10];
    for (idx, &(a, b, c)) in BOARD_TRIPLES.iter().enumerate() {
        tri_or[idx] = kb[a] | kb[b] | kb[c];
        tri_and[idx] = kb[a] & kb[b] & kb[c];
        tri_prime[idx] = (kb[a] & 0xff)
            .wrapping_mul(kb[b] & 0xff)
            .wrapping_mul(kb[c] & 0xff);
    }

    let mut best_kev: u16 = u16::MAX;
    for pi in 0..6 {
        for ti in 0..10 {
            let r = eval_5cards_kev_v1_precomp(
                pair_or[pi], pair_and[pi], pair_prime[pi],
                tri_or[ti], tri_and[ti], tri_prime[ti],
            );
            if r < best_kev {
                best_kev = r;
            }
        }
    }
    kev_rank_to_packed(best_kev)
}

/// **Experimental v2**: always-flush variant. Returns wrong answers
/// (it just calls `FLUSHES[q]` per combo regardless of whether the
/// combo is actually a flush). Used to measure the minimum-cost
/// Cactus-Kev path's contribution to total time.
pub fn evaluate_kev_v2_always_flush(hole: &[usize; 4], board: &[usize; 5]) -> u16 {
    let kh: [u32; 4] = [
        KEV_CARDS[hole[0]], KEV_CARDS[hole[1]], KEV_CARDS[hole[2]], KEV_CARDS[hole[3]],
    ];
    let kb: [u32; 5] = [
        KEV_CARDS[board[0]], KEV_CARDS[board[1]], KEV_CARDS[board[2]],
        KEV_CARDS[board[3]], KEV_CARDS[board[4]],
    ];
    let mut best_kev: u16 = u16::MAX;
    for &(i, j) in &HOLE_PAIRS {
        let ki = kh[i]; let kj = kh[j];
        for &(a, b, c) in &BOARD_TRIPLES {
            let r = eval_5cards_kev_v2_always_flush(ki, kj, kb[a], kb[b], kb[c]);
            if r < best_kev { best_kev = r; }
        }
    }
    best_kev
}

/// **Experimental v3**: skip flush + unique5 checks; always run the
/// prime product + `find_fast` + `HASH_VALUES` chain. Wrong answers
/// for flush / unique5 hands. Measures the cost of Cactus-Kev's
/// imperfect-hash branch specifically.
pub fn evaluate_kev_v3_always_hash(hole: &[usize; 4], board: &[usize; 5]) -> u16 {
    let kh: [u32; 4] = [
        KEV_CARDS[hole[0]], KEV_CARDS[hole[1]], KEV_CARDS[hole[2]], KEV_CARDS[hole[3]],
    ];
    let kb: [u32; 5] = [
        KEV_CARDS[board[0]], KEV_CARDS[board[1]], KEV_CARDS[board[2]],
        KEV_CARDS[board[3]], KEV_CARDS[board[4]],
    ];
    let mut best_kev: u16 = u16::MAX;
    for &(i, j) in &HOLE_PAIRS {
        let ki = kh[i]; let kj = kh[j];
        for &(a, b, c) in &BOARD_TRIPLES {
            let r = eval_5cards_kev_v3_always_hash(ki, kj, kb[a], kb[b], kb[c]);
            if r < best_kev { best_kev = r; }
        }
    }
    best_kev
}

/// **Experimental** straight-short-circuit evaluator for Omaha high.
///
/// Whole-hand fast path that returns the packed Straight rank without
/// running the 60-combo loop, when:
///   - no flush is reachable
///   - the board has no pair (so FH/Quads are also unreachable —
///     anything below Straight category cannot beat a Straight)
///   - some 5-rank window has ≥3 ranks on board and ≥2 ranks in hole
///     covering the missing two
///
/// Bench scenario for the user's question about precomputing
/// straight-completing card combos. We don't need the full
/// "board-triple → completing-hole-pairs" table because the same
/// information is encoded compactly in two 13-bit rank masks plus a
/// per-window AND/popcount sweep.
pub fn evaluate_straight_short_circuit(hole: &[usize; 4], board: &[usize; 5]) -> u16 {
    // Suit counts for flush eligibility.
    let mut hole_s = [0u8; 4];
    let mut board_s = [0u8; 4];
    for &c in hole {
        hole_s[c & 3] += 1;
    }
    for &c in board {
        board_s[c & 3] += 1;
    }
    let flush_eligible = (0..4).any(|s| hole_s[s] >= 2 && board_s[s] >= 3);

    // Rank masks.
    let mut hole_mask: u16 = 0;
    let mut board_mask: u16 = 0;
    let mut hole_dup = false;
    {
        let mut h_seen: u16 = 0;
        for &c in hole {
            let bit = 1u16 << (c / 4);
            if h_seen & bit != 0 {
                hole_dup = true;
            }
            h_seen |= bit;
            hole_mask |= bit;
        }
    }
    let mut board_dup = false;
    {
        let mut b_seen: u16 = 0;
        for &c in board {
            let bit = 1u16 << (c / 4);
            if b_seen & bit != 0 {
                board_dup = true;
            }
            b_seen |= bit;
            board_mask |= bit;
        }
    }

    // Short-circuit condition: no flush, no board pair, straight reachable.
    if !flush_eligible && !board_dup {
        let _ = hole_dup; // unused in this path (pocket pair OK for straight detect, but rules out it being the max if it gives Trips — actually Trips < Straight so still fine)
        if let Some(top) = quick_max_straight_top(hole_mask, board_mask) {
            // Packed Straight rank = (cat 4 << 12) | (top - 3).
            // top range: 3 (wheel 5-high) ..= 12 (broadway A-high), idx 0..9.
            return (4u16 << 12) | (top - 3) as u16;
        }
    }

    // Fall back to the production evaluator.
    OmahaHighRule::evaluate(hole, board)
}

/// Highest top-rank of a straight reachable from
/// `(2 hole cards + 3 board cards)`, or `None` if no straight is
/// reachable. `top` is the highest rank of the 5-card window
/// (3 for wheel A-2-3-4-5, 4..=12 for 6-high through broadway).
#[inline]
fn quick_max_straight_top(hole_mask: u16, board_mask: u16) -> Option<u8> {
    let combined = hole_mask | board_mask;
    let mut best_top: Option<u8> = None;
    // 9 standard windows: ranks {r, r+1, r+2, r+3, r+4} for r = 0..=8.
    // Iterate ascending — higher r overrides a lower-found straight.
    for r in 0u8..=8 {
        let window: u16 = 0b1_1111u16 << r;
        if (combined & window).count_ones() < 5 {
            continue;
        }
        if (board_mask & window).count_ones() < 3 {
            continue;
        }
        if (hole_mask & window).count_ones() < 2 {
            continue;
        }
        // The 2 missing-from-board ranks must be in hole.
        let need_from_hole = window & !board_mask;
        if need_from_hole & hole_mask != need_from_hole {
            continue;
        }
        if need_from_hole.count_ones() > 2 {
            continue;
        }
        best_top = Some(r + 4);
    }
    // Wheel only matters if no higher straight was found.
    if best_top.is_none() {
        let wheel: u16 = (1u16 << 0) | (1u16 << 1) | (1u16 << 2) | (1u16 << 3) | (1u16 << 12);
        if (combined & wheel).count_ones() == 5
            && (board_mask & wheel).count_ones() >= 3
            && (hole_mask & wheel).count_ones() >= 2
        {
            let need_from_hole = wheel & !board_mask;
            if need_from_hole & hole_mask == need_from_hole
                && need_from_hole.count_ones() <= 2
            {
                best_top = Some(3); // wheel = 5-high straight
            }
        }
    }
    best_top
}

/// **Experimental** Cactus-Kev based evaluator for Omaha high.
///
/// Uses the ~49 KB Kev tables (`HASH_*`, `FLUSHES`, `UNIQUE5`) instead
/// of `phe-holdem`'s 145 KB perfect-hash, on the theory that the
/// smaller working set fits L1d and pays off across the 60-combo
/// inner loop. Behaviorally identical to [`OmahaHighRule::evaluate`];
/// pin the equivalence via the integration tests in
/// `tests/kev_equivalence.rs`.
///
/// Convention: takes the **min** of 60 Kev ranks (smaller = stronger
/// in the Kev convention) and converts once at the end via
/// [`kev::kev_rank_to_packed`].
pub fn evaluate_kev(hole: &[usize; 4], board: &[usize; 5]) -> u16 {
    let kh: [u32; 4] = [
        KEV_CARDS[hole[0]],
        KEV_CARDS[hole[1]],
        KEV_CARDS[hole[2]],
        KEV_CARDS[hole[3]],
    ];
    let kb: [u32; 5] = [
        KEV_CARDS[board[0]],
        KEV_CARDS[board[1]],
        KEV_CARDS[board[2]],
        KEV_CARDS[board[3]],
        KEV_CARDS[board[4]],
    ];

    let mut best_kev: u16 = u16::MAX;
    for &(i, j) in &HOLE_PAIRS {
        let ki = kh[i];
        let kj = kh[j];
        for &(a, b, c) in &BOARD_TRIPLES {
            let r = eval_5cards_kev(ki, kj, kb[a], kb[b], kb[c]);
            if r < best_kev {
                best_kev = r;
            }
        }
    }
    kev_rank_to_packed(best_kev)
}

impl OmahaHighRule {
    /// Evaluates an Omaha high hand from `hole_cards` (length 4) and
    /// `board` (length 5).
    ///
    /// # Panics
    /// Panics if the slice lengths are wrong (enforced by the array
    /// types at compile time).
    #[inline]
    pub fn evaluate(hole_cards: &[usize; 4], board: &[usize; 5]) -> u16 {
        match flush_suit(hole_cards, board) {
            None => {
                // Path 1: no flush possible → Hand-free rank-only inner.
                evaluate_no_flush_path(hole_cards, board)
            }
            Some(s) => {
                if board_has_no_pair(board) {
                    // Path 2: flush dominates → suit-restricted enum only.
                    evaluate_flush_dominate(hole_cards, board, s)
                } else {
                    // Path 3: flush + board pair → Hand-free per-combo
                    // dispatch (FH/Quads still possible via rank path).
                    evaluate_flush_with_pair_path(hole_cards, board, s)
                }
            }
        }
    }
}
