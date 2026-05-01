//! `evaluate_plo4_cards` — direct port of HenryRLee
//! `cpp/src/evaluator_plo4.c`.

use crate::dp::BIT_OF_DIV_4;
use crate::flush_5card::FLUSH;
use crate::hash::{hash_binary, hash_quinary};
use phe_omaha_fast_assets::{FLUSH_PLO4, NOFLUSH_PLO4};

/// Padding constants from `evaluator_plo4.c`. Used to normalise a
/// per-suit rank-bitmap to exactly 5 (board) or 4 (hole) bits set
/// before passing to `hash_binary`.
const PADDING: [i32; 3] = [0x0000, 0x2000, 0x6000];

/// Evaluates a PLO4 hand: 5 community cards + 4 hole cards.
///
/// Returns the Cactus-Kev rank in `[1, 7462]` where **lower =
/// stronger**. Caller is responsible for converting to the workspace's
/// "higher = stronger" u16 contract.
///
/// Card encoding: `card = rank * 4 + suit`, ranks 0=2…12=A,
/// suits 0=c, 1=d, 2=h, 3=s. Identical to `phe_core::CARDS`.
pub fn evaluate_plo4_cards(c1: i32, c2: i32, c3: i32, c4: i32, c5: i32,
                           h1: i32, h2: i32, h3: i32, h4: i32) -> i32 {
    let mut value_flush: i32 = 10000;
    let value_noflush: i32;

    let mut suit_count_board = [0i32; 4];
    let mut suit_count_hole = [0i32; 4];

    suit_count_board[(c1 & 0x3) as usize] += 1;
    suit_count_board[(c2 & 0x3) as usize] += 1;
    suit_count_board[(c3 & 0x3) as usize] += 1;
    suit_count_board[(c4 & 0x3) as usize] += 1;
    suit_count_board[(c5 & 0x3) as usize] += 1;

    suit_count_hole[(h1 & 0x3) as usize] += 1;
    suit_count_hole[(h2 & 0x3) as usize] += 1;
    suit_count_hole[(h3 & 0x3) as usize] += 1;
    suit_count_hole[(h4 & 0x3) as usize] += 1;

    for i in 0..4 {
        if suit_count_board[i] >= 3 && suit_count_hole[i] >= 2 {
            // Flush is reachable in suit `i`.
            let mut suit_binary_board = [0i32; 4];
            suit_binary_board[(c1 & 0x3) as usize] |= BIT_OF_DIV_4[c1 as usize] as i32;
            suit_binary_board[(c2 & 0x3) as usize] |= BIT_OF_DIV_4[c2 as usize] as i32;
            suit_binary_board[(c3 & 0x3) as usize] |= BIT_OF_DIV_4[c3 as usize] as i32;
            suit_binary_board[(c4 & 0x3) as usize] |= BIT_OF_DIV_4[c4 as usize] as i32;
            suit_binary_board[(c5 & 0x3) as usize] |= BIT_OF_DIV_4[c5 as usize] as i32;

            let mut suit_binary_hole = [0i32; 4];
            suit_binary_hole[(h1 & 0x3) as usize] |= BIT_OF_DIV_4[h1 as usize] as i32;
            suit_binary_hole[(h2 & 0x3) as usize] |= BIT_OF_DIV_4[h2 as usize] as i32;
            suit_binary_hole[(h3 & 0x3) as usize] |= BIT_OF_DIV_4[h3 as usize] as i32;
            suit_binary_hole[(h4 & 0x3) as usize] |= BIT_OF_DIV_4[h4 as usize] as i32;

            if suit_count_board[i] == 3 && suit_count_hole[i] == 2 {
                // Special case: exactly 3+2 → 5-card flush via `FLUSH[8192]`.
                value_flush = FLUSH[(suit_binary_board[i] | suit_binary_hole[i]) as usize] as i32;
            } else {
                let board_padded =
                    suit_binary_board[i] | PADDING[(5 - suit_count_board[i]) as usize];
                let hole_padded =
                    suit_binary_hole[i] | PADDING[(4 - suit_count_hole[i]) as usize];

                let board_hash = hash_binary(board_padded, 5);
                let hole_hash = hash_binary(hole_padded, 4);

                value_flush =
                    FLUSH_PLO4[(board_hash * 1365 + hole_hash) as usize] as i32;
            }

            break;
        }
    }

    let mut quinary_board = [0u8; 13];
    let mut quinary_hole = [0u8; 13];

    quinary_board[(c1 >> 2) as usize] += 1;
    quinary_board[(c2 >> 2) as usize] += 1;
    quinary_board[(c3 >> 2) as usize] += 1;
    quinary_board[(c4 >> 2) as usize] += 1;
    quinary_board[(c5 >> 2) as usize] += 1;

    quinary_hole[(h1 >> 2) as usize] += 1;
    quinary_hole[(h2 >> 2) as usize] += 1;
    quinary_hole[(h3 >> 2) as usize] += 1;
    quinary_hole[(h4 >> 2) as usize] += 1;

    let board_hash = hash_quinary(&quinary_board, 5);
    let hole_hash = hash_quinary(&quinary_hole, 4);

    value_noflush = NOFLUSH_PLO4[(board_hash * 1820 + hole_hash) as usize] as i32;

    value_flush.min(value_noflush)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Card encoding helpers: rank in 0..13, suit in 0..4.
    // 0=2c, 1=2d, 2=2h, 3=2s, 4=3c, ..., 48=Ac, 49=Ad, 50=Ah, 51=As.
    const fn card(rank: i32, suit: i32) -> i32 { rank * 4 + suit }

    const TWO_C:  i32 = card(0, 0);
    const TWO_D:  i32 = card(0, 1);
    const TWO_H:  i32 = card(0, 2);
    const TWO_S:  i32 = card(0, 3);
    const THREE_C:i32 = card(1, 0);
    const THREE_D:i32 = card(1, 1);
    const THREE_H:i32 = card(1, 2);
    const THREE_S:i32 = card(1, 3);
    const FOUR_C: i32 = card(2, 0);
    const FOUR_S: i32 = card(2, 3);
    const FIVE_C: i32 = card(3, 0);
    const FIVE_D: i32 = card(3, 1);
    const FIVE_S: i32 = card(3, 3);
    const SIX_C:  i32 = card(4, 0);
    const SEVEN_S:i32 = card(5, 3);
    const NINE_S: i32 = card(7, 3);
    const TEN_S:  i32 = card(8, 3);
    const JACK_S: i32 = card(9, 3);
    const QUEEN_S:i32 = card(10, 3);
    const KING_S: i32 = card(11, 3);
    const ACE_C:  i32 = card(12, 0);
    const ACE_D:  i32 = card(12, 1);
    const ACE_H:  i32 = card(12, 2);
    const ACE_S:  i32 = card(12, 3);

    /// Bounds: every legal 9-card input must produce a rank in
    /// `[1, 7462]` (Cactus-Kev range, lower=stronger).
    #[test]
    fn output_in_cactus_kev_range() {
        let r = evaluate_plo4_cards(
            ACE_S, KING_S, QUEEN_S, JACK_S, TEN_S,
            ACE_C, ACE_D, ACE_H, TWO_C,
        );
        assert!(r >= 1 && r <= 7462, "rank {r} out of range");
    }

    /// Royal SF on the board with hole containing 2 spades → Royal SF
    /// is reachable via 2-hole + 3-board. Cactus-Kev rank 1.
    #[test]
    fn royal_straight_flush() {
        // hole has As + Ks (2 spades), board has Qs Js Ts + 2 fillers.
        let r = evaluate_plo4_cards(
            QUEEN_S, JACK_S, TEN_S, TWO_C, THREE_C,  // board
            ACE_S, KING_S, TWO_D, TWO_H,             // hole (As Ks plus filler)
        );
        assert_eq!(r, 1, "royal SF should be rank 1, got {r}");
    }

    /// Lowest possible 5-card hand: 7-5-4-3-2 unsuited, no straight/flush.
    /// Cactus-Kev rank 7462.
    /// In PLO4: pick 2 from hole and 3 from board.
    /// hole = [7s, 5s, Ks, Qs] (so we have 7s+5s in hole and pick 2 lowest)
    /// board = [4c, 3c, 2d, Ac, Ad]
    /// Best 5-card from 2-hole + 3-board:
    /// - try 7s+5s + 4c+3c+2d = 7-high no straight (5+7 with 4-3-2 gap)
    ///   Wait: 7-5-4-3-2 IS a no-straight (gap between 5 and 7? no, 7-5-4-3-2:
    ///   ranks 5,4,3,2,7 — 7-6-5-4-3-2 would be straight, but we lack 6).
    ///   So 7-5-4-3-2 = high-card-7 = rank 7462.
    /// - other 2-hole choices: 7s+Ks, etc., all give worse (higher rank
    ///   than 7-high, e.g., K-high). Since "lower=stronger" in C convention,
    ///   the BEST (= lowest rank) is the strongest combo.
    ///   But the function returns the STRONGEST, i.e., min over combos.
    ///   Strongest from this hole+board includes Ace-anything → A-high
    ///   = rank ~6300. So rank ≠ 7462.
    /// Skip absolute assert, just check determinism + bounds.
    #[test]
    fn deterministic() {
        let r1 = evaluate_plo4_cards(
            FOUR_C, THREE_C, TWO_D, ACE_C, ACE_D,
            SEVEN_S, FIVE_S, KING_S, QUEEN_S,
        );
        let r2 = evaluate_plo4_cards(
            FOUR_C, THREE_C, TWO_D, ACE_C, ACE_D,
            SEVEN_S, FIVE_S, KING_S, QUEEN_S,
        );
        assert_eq!(r1, r2);
        assert!(r1 >= 1 && r1 <= 7462);
    }

    /// Quad aces with a king kicker: AAAA-K.
    /// hole = [Ac, Ad, Ah, As] (all 4 aces). board = [Kc, 2c, 3c, 4c, 5c].
    /// In PLO4 you must use exactly 2 hole + 3 board. So the best 5
    /// is 2 aces (from hole) + 3 board cards. With 4 aces in hole, you
    /// can only put 2 of them in your 5-card hand → AA pair, NOT quads.
    /// So actually the strongest hand here is wheel: A+A+5+4+3+2 …
    /// but that's still 6 cards. Pick 2 aces + 5+4+3 from board:
    /// AA + 5 + 4 + 3 = pair AA, K-5-4 unused → AA + 5-4-3 (pair).
    /// Wait, only 5 cards total. So 2 aces + (5,4,3) = AA-5-4-3.
    /// Or 2 aces + (4,3,2) = AA-4-3-2. Pair of aces, low kickers.
    /// Or 2 aces + (K,5,4) = AA-K-5-4. Pair of aces with K kicker
    /// — strongest pair.
    /// Cactus Kev one-pair: range is [3326, 6185] roughly.
    /// Skip exact value but ensure it's in one-pair range.
    #[test]
    fn pair_of_aces_ranges() {
        let r = evaluate_plo4_cards(
            KING_S, FIVE_C, FOUR_C, THREE_C, TWO_C, // board
            ACE_C, ACE_D, ACE_H, ACE_S,             // hole (all aces)
        );
        // Cactus-Kev pair range: [3326, 6185].
        assert!(r >= 3326 && r <= 6185, "pair-AA hand should be in pair range, got {r}");
    }

    /// Flush dispatch path 1 (3-board + 2-hole exact 5-card flush, → FLUSH[8192]).
    /// hole has 2 hearts, board has 3 hearts (no other matching suit).
    /// Result must beat any non-flush from same input.
    #[test]
    fn flush_5card_special_case() {
        // hole: 2h + 3h + filler
        // board: 5h + 6h + 7h + filler clubs
        let r = evaluate_plo4_cards(
            card(3, 2), card(4, 2), card(5, 2), TWO_C, FOUR_C, // board: 5h 6h 7h 2c 4c
            TWO_H, THREE_H, KING_S, QUEEN_S,                   // hole: 2h 3h Ks Qs
        );
        // 2h+3h+5h+6h+7h = 7-high flush (no straight). Cactus-Kev
        // flush range: [323, 1599]. 7-high flush is near the bottom.
        assert!(r >= 323 && r <= 1599, "5-card flush should be in flush range, got {r}");
    }

    /// Ordering sanity: a hand with a flush available should beat
    /// the same board with no flush.
    #[test]
    fn flush_beats_no_flush() {
        // same board for both: 5h 6h 7h 2c 4c
        let board = (card(3, 2), card(4, 2), card(5, 2), TWO_C, FOUR_C);
        // Hole A: hearts → flush
        let with_flush = evaluate_plo4_cards(
            board.0, board.1, board.2, board.3, board.4,
            TWO_H, THREE_H, KING_S, QUEEN_S,
        );
        // Hole B: no extra hearts → no flush, just whatever pairs.
        let no_flush = evaluate_plo4_cards(
            board.0, board.1, board.2, board.3, board.4,
            TWO_S, THREE_S, KING_S, QUEEN_S,
        );
        assert!(with_flush < no_flush,
            "flush rank {with_flush} should be < no-flush rank {no_flush} (lower=stronger)");
    }
}
