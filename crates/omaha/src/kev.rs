//! Cactus-Kev 5-card hand evaluator (with Senzee's perfect-hash modification).
//!
//! Used as the inner kernel for the experimental Kev-based Omaha
//! evaluator (`evaluate_kev` in this crate). Smaller working set than
//! `phe-holdem`'s 145 KB perfect-hash table — total Kev tables are
//! ~49 KB and fit L1d on most x86. See
//! `docs/omaha-perf-investigation.md` for the rationale and
//! benchmark target.
//!
//! Convention: returned u16 is the **Cactus-Kev rank** in `1..=7462`,
//! where **smaller = stronger** (1 = royal SF, 7462 = worst HighCard
//! 7-5-4-3-2). To compare with our packed `(category << 12) | idx`
//! (higher = stronger) format, use [`kev_rank_to_packed`] **once
//! per Omaha eval** after taking the min over the 60 combos.
//!
//! Tables are verbatim from b-inary/holdem-hand-evaluator
//! (`scripts/src/kev/arrays.rs`), which in turn ports the original
//! Cactus-Kev / Senzee tables.

use crate::kev_tables::{FLUSHES, HASH_ADJUST, HASH_VALUES, UNIQUE5};

/// Card encoding for the Cactus-Kev kernel.
///
/// Layout (32 bits):
///
/// ```text
/// bits 16-28: 1 << rank      (used for OR-merged straight/flush detection)
/// bits 12-15: suit-bit       (1=spade, 2=heart, 4=diamond, 8=club)
/// bits  8-11: rank ordinal   (0..=12)
/// bits  0-7 : prime          (2, 3, 5, 7, 11, 13, ..., 41 for ranks 2..A)
/// ```
///
/// Indexed by our standard card id `rank * 4 + suit` (rank 0 = 2, 12 = A;
/// suit 0 = club, 1 = diamond, 2 = heart, 3 = spade), so the suit-bit
/// values look reversed vs. our suit indices because Kev numbers suits
/// as `1 << (3 - our_suit)`.
pub const KEV_CARDS: [u32; 52] = [
    0x18002, 0x14002, 0x12002, 0x11002, // 2c 2d 2h 2s
    0x28103, 0x24103, 0x22103, 0x21103, // 3
    0x48205, 0x44205, 0x42205, 0x41205, // 4
    0x88307, 0x84307, 0x82307, 0x81307, // 5
    0x10840b, 0x10440b, 0x10240b, 0x10140b, // 6
    0x20850d, 0x20450d, 0x20250d, 0x20150d, // 7
    0x408611, 0x404611, 0x402611, 0x401611, // 8
    0x808713, 0x804713, 0x802713, 0x801713, // 9
    0x1008817, 0x1004817, 0x1002817, 0x1001817, // T
    0x200891d, 0x200491d, 0x200291d, 0x200191d, // J
    0x4008a1f, 0x4004a1f, 0x4002a1f, 0x4001a1f, // Q
    0x8008b25, 0x8004b25, 0x8002b25, 0x8001b25, // K
    0x10008c29, 0x10004c29, 0x10002c29, 0x10001c29, // A
];

/// Senzee's perfect-hash function for the non-flush, non-straight,
/// non-pair "high card" branch. Maps a 32-bit prime product to a
/// 13-bit `HASH_VALUES` index.
#[inline(always)]
fn find_fast(u: u32) -> usize {
    let u = u.wrapping_add(0xe91aaa35);
    let u = u ^ (u >> 16);
    let u = u.wrapping_add(u << 8);
    let u = u ^ (u >> 4);
    let b = (u >> 8) & 0x1ff;
    let a = u.wrapping_add(u << 2) >> 19;
    (a as usize) ^ (HASH_ADJUST[b as usize] as usize)
}

/// 5-card Cactus-Kev hand evaluation. Returns Kev rank (1..=7462,
/// smaller = stronger).
///
/// Inputs are [`KEV_CARDS`] entries (i.e., 32-bit-encoded cards), not
/// raw card ids.
#[inline]
pub fn eval_5cards_kev(c1: u32, c2: u32, c3: u32, c4: u32, c5: u32) -> u16 {
    let q = ((c1 | c2 | c3 | c4 | c5) >> 16) as usize;
    if (c1 & c2 & c3 & c4 & c5 & 0xf000) != 0 {
        return FLUSHES[q];
    }
    let s = UNIQUE5[q];
    if s != 0 {
        return s;
    }
    let prime = (c1 & 0xff)
        .wrapping_mul(c2 & 0xff)
        .wrapping_mul(c3 & 0xff)
        .wrapping_mul(c4 & 0xff)
        .wrapping_mul(c5 & 0xff);
    HASH_VALUES[find_fast(prime)]
}

/// Convert a Cactus-Kev rank (1..=7462, smaller = stronger) into our
/// packed `(category << 12) | within_category_index` format
/// (higher = stronger).
///
/// Logic mirrors `adjust_hand_rank` in
/// `b-inary/holdem-hand-evaluator/scripts/src/02-lookup_tables.rs`:
/// reverse so the smallest Kev rank becomes the largest reversed
/// rank, then partition by the known per-category equivalence-class
/// ranges (1277 HighCard, 2860 OnePair, 858 TwoPair, 858 ThreeOfAKind,
/// 10 Straight, 1277 Flush, 156 FullHouse, 156 FourOfAKind, 10
/// StraightFlush; 7462 total).
#[inline]
pub fn kev_rank_to_packed(kev_rank: u16) -> u16 {
    debug_assert!(kev_rank >= 1 && kev_rank <= 7462, "kev_rank out of range: {}", kev_rank);
    let r = 7463u16 - kev_rank; // 7462 = best, 1 = worst
    match r {
        1..=1277 => r - 1,                           // HighCard, cat 0
        1278..=4137 => (1u16 << 12) | (r - 1278),    // OnePair
        4138..=4995 => (2u16 << 12) | (r - 4138),    // TwoPair
        4996..=5853 => (3u16 << 12) | (r - 4996),    // ThreeOfAKind
        5854..=5863 => (4u16 << 12) | (r - 5854),    // Straight
        5864..=7140 => (5u16 << 12) | (r - 5864),    // Flush
        7141..=7296 => (6u16 << 12) | (r - 7141),    // FullHouse
        7297..=7452 => (7u16 << 12) | (r - 7297),    // FourOfAKind
        7453..=7462 => (8u16 << 12) | (r - 7453),    // StraightFlush
        _ => unreachable!(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Cards using our standard ids: rank * 4 + suit, suit 0=c, 1=d, 2=h, 3=s.
    fn id(rank: u8, suit: u8) -> usize {
        (rank * 4 + suit) as usize
    }

    #[test]
    fn royal_flush_is_kev_rank_1() {
        // A♠ K♠ Q♠ J♠ T♠ → highest possible Kev hand.
        let cs = [
            KEV_CARDS[id(12, 3)], KEV_CARDS[id(11, 3)], KEV_CARDS[id(10, 3)],
            KEV_CARDS[id(9, 3)], KEV_CARDS[id(8, 3)],
        ];
        assert_eq!(eval_5cards_kev(cs[0], cs[1], cs[2], cs[3], cs[4]), 1);
    }

    #[test]
    fn worst_hand_is_kev_rank_7462() {
        // 7♥ 5♣ 4♣ 3♣ 2♣ — 7-high, no pair, no straight, no flush.
        let cs = [
            KEV_CARDS[id(5, 2)], KEV_CARDS[id(3, 0)], KEV_CARDS[id(2, 0)],
            KEV_CARDS[id(1, 0)], KEV_CARDS[id(0, 0)],
        ];
        assert_eq!(eval_5cards_kev(cs[0], cs[1], cs[2], cs[3], cs[4]), 7462);
    }

    #[test]
    fn kev_rank_to_packed_endpoints() {
        // Royal SF (Kev 1) → top packed rank in cat 8.
        assert_eq!(kev_rank_to_packed(1), (8 << 12) | 9); // 10 SFs, top is idx 9
        // Worst HighCard (Kev 7462) → bottom packed rank.
        assert_eq!(kev_rank_to_packed(7462), 0);
    }

    #[test]
    fn kev_rank_to_packed_is_monotonic_within_categories() {
        // Take a few neighboring Kev ranks, packed should be monotone-decreasing
        // (since lower Kev = better, but lower packed = worse).
        for k in 1..=7461u16 {
            let lhs = kev_rank_to_packed(k);
            let rhs = kev_rank_to_packed(k + 1);
            assert!(lhs > rhs, "non-monotone at k={}: lhs={}, rhs={}", k, lhs, rhs);
        }
    }

    #[test]
    fn straight_flush_lands_in_cat_8() {
        // 9♥ 8♥ 7♥ 6♥ 5♥
        let cs = [
            KEV_CARDS[id(7, 2)], KEV_CARDS[id(6, 2)], KEV_CARDS[id(5, 2)],
            KEV_CARDS[id(4, 2)], KEV_CARDS[id(3, 2)],
        ];
        let kev = eval_5cards_kev(cs[0], cs[1], cs[2], cs[3], cs[4]);
        let packed = kev_rank_to_packed(kev);
        assert_eq!(packed >> 12, 8);
    }

    #[test]
    fn quads_lands_in_cat_7() {
        // A♣ A♦ A♥ A♠ K♣
        let cs = [
            KEV_CARDS[id(12, 0)], KEV_CARDS[id(12, 1)], KEV_CARDS[id(12, 2)],
            KEV_CARDS[id(12, 3)], KEV_CARDS[id(11, 0)],
        ];
        let kev = eval_5cards_kev(cs[0], cs[1], cs[2], cs[3], cs[4]);
        assert_eq!(kev_rank_to_packed(kev) >> 12, 7);
    }

    #[test]
    fn full_house_lands_in_cat_6() {
        // 8♣ 8♦ 8♥ 5♣ 5♦
        let cs = [
            KEV_CARDS[id(6, 0)], KEV_CARDS[id(6, 1)], KEV_CARDS[id(6, 2)],
            KEV_CARDS[id(3, 0)], KEV_CARDS[id(3, 1)],
        ];
        let kev = eval_5cards_kev(cs[0], cs[1], cs[2], cs[3], cs[4]);
        assert_eq!(kev_rank_to_packed(kev) >> 12, 6);
    }

    #[test]
    fn flush_lands_in_cat_5() {
        // A♣ Q♣ 9♣ 5♣ 2♣ (no straight, all clubs)
        let cs = [
            KEV_CARDS[id(12, 0)], KEV_CARDS[id(10, 0)], KEV_CARDS[id(7, 0)],
            KEV_CARDS[id(3, 0)], KEV_CARDS[id(0, 0)],
        ];
        let kev = eval_5cards_kev(cs[0], cs[1], cs[2], cs[3], cs[4]);
        assert_eq!(kev_rank_to_packed(kev) >> 12, 5);
    }
}
