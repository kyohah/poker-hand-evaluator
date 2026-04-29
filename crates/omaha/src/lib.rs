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

use phe_core::{Hand, OFFSETS, OFFSET_SHIFT};
use phe_holdem::assets::{LOOKUP, LOOKUP_FLUSH};
use phe_holdem::HighRule;

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

/// Builds the 10 partial Hands for each board-triple selection.
#[inline]
fn build_board_partials(board: &[usize; 5]) -> [Hand; 10] {
    std::array::from_fn(|idx| {
        let (a, b, c) = BOARD_TRIPLES[idx];
        Hand::new()
            .add_card(board[a])
            .add_card(board[b])
            .add_card(board[c])
    })
}

/// Rank-only lookup. Skips the flush-mask check; the caller must
/// guarantee the hand is non-flush. Otherwise behaviour is undefined.
#[inline]
fn evaluate_rank_only(hand: &Hand) -> u16 {
    let rank_key = hand.get_key() as u32 as usize;
    unsafe {
        let offset = *OFFSETS.get_unchecked(rank_key >> OFFSET_SHIFT) as usize;
        *LOOKUP.get_unchecked(rank_key.wrapping_add(offset))
    }
}

/// Inner loop over all 60 (hole_pair, board_partial) combos.
#[inline]
fn evaluate_full_60<F>(hole: &[usize; 4], board_partials: &[Hand; 10], eval: F) -> u16
where
    F: Fn(&Hand) -> u16,
{
    let mut best: u16 = 0;
    for &(i, j) in &HOLE_PAIRS {
        let hp = Hand::new().add_card(hole[i]).add_card(hole[j]);
        for bp in board_partials {
            let h = hp + *bp;
            let r = eval(&h);
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
                // Path 1: no flush possible → rank-only inner.
                let board_partials = build_board_partials(board);
                evaluate_full_60(hole_cards, &board_partials, evaluate_rank_only)
            }
            Some(s) => {
                if board_has_no_pair(board) {
                    // Path 2: flush dominates → suit-restricted enum only.
                    evaluate_flush_dominate(hole_cards, board, s)
                } else {
                    // Path 3: flush + board pair → full eval (FH/Quads possible).
                    let board_partials = build_board_partials(board);
                    evaluate_full_60(hole_cards, &board_partials, HighRule::evaluate)
                }
            }
        }
    }
}
