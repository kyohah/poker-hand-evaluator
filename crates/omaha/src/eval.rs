//! `evaluate_plo4_cards` — direct port of HenryRLee
//! `cpp/src/evaluator_plo4.c`.
//!
//! Hot-path array accesses use `get_unchecked` (see SAFETY comments).
//! All inputs are card ids in `[0, 51]`; the C reference is UB outside
//! that range and we preserve that contract.

use crate::dp::BIT_OF_DIV_4;
use crate::flush_5card::FLUSH;
use crate::hash::{hash_binary, hash_quinary};
use phe_omaha_assets::{FLUSH_PLO4, NOFLUSH_PLO4};

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
///
/// # Safety contract (informal)
///
/// All 9 card arguments must be in `[0, 51]`. Out-of-range inputs are
/// UB (matches the C reference). Duplicates within the 9 cards are
/// silently accepted (the function does not validate); behaviour
/// mirrors HenryRLee's library.
#[inline(always)]
pub fn evaluate_plo4_cards(
    c1: i32,
    c2: i32,
    c3: i32,
    c4: i32,
    c5: i32,
    h1: i32,
    h2: i32,
    h3: i32,
    h4: i32,
) -> i32 {
    let mut value_flush: i32 = 10000;

    let mut suit_count_board = [0i32; 4];
    let mut suit_count_hole = [0i32; 4];

    // SAFETY: c & 0x3 ∈ [0, 3], within suit_count_*'s 4-element bounds.
    unsafe {
        *suit_count_board.get_unchecked_mut((c1 & 0x3) as usize) += 1;
        *suit_count_board.get_unchecked_mut((c2 & 0x3) as usize) += 1;
        *suit_count_board.get_unchecked_mut((c3 & 0x3) as usize) += 1;
        *suit_count_board.get_unchecked_mut((c4 & 0x3) as usize) += 1;
        *suit_count_board.get_unchecked_mut((c5 & 0x3) as usize) += 1;

        *suit_count_hole.get_unchecked_mut((h1 & 0x3) as usize) += 1;
        *suit_count_hole.get_unchecked_mut((h2 & 0x3) as usize) += 1;
        *suit_count_hole.get_unchecked_mut((h3 & 0x3) as usize) += 1;
        *suit_count_hole.get_unchecked_mut((h4 & 0x3) as usize) += 1;
    }

    for i in 0..4 {
        // SAFETY: i ∈ [0,3], suit_count_*[i] valid.
        let scb = unsafe { *suit_count_board.get_unchecked(i) };
        let sch = unsafe { *suit_count_hole.get_unchecked(i) };
        if scb >= 3 && sch >= 2 {
            // Flush is reachable in suit `i`.
            let mut suit_binary_board = [0i32; 4];
            let mut suit_binary_hole = [0i32; 4];

            // SAFETY: c & 0x3 ∈ [0,3] for suit_binary index;
            // c ∈ [0,51] for BIT_OF_DIV_4 (length 52).
            unsafe {
                *suit_binary_board.get_unchecked_mut((c1 & 0x3) as usize) |=
                    *BIT_OF_DIV_4.get_unchecked(c1 as usize) as i32;
                *suit_binary_board.get_unchecked_mut((c2 & 0x3) as usize) |=
                    *BIT_OF_DIV_4.get_unchecked(c2 as usize) as i32;
                *suit_binary_board.get_unchecked_mut((c3 & 0x3) as usize) |=
                    *BIT_OF_DIV_4.get_unchecked(c3 as usize) as i32;
                *suit_binary_board.get_unchecked_mut((c4 & 0x3) as usize) |=
                    *BIT_OF_DIV_4.get_unchecked(c4 as usize) as i32;
                *suit_binary_board.get_unchecked_mut((c5 & 0x3) as usize) |=
                    *BIT_OF_DIV_4.get_unchecked(c5 as usize) as i32;

                *suit_binary_hole.get_unchecked_mut((h1 & 0x3) as usize) |=
                    *BIT_OF_DIV_4.get_unchecked(h1 as usize) as i32;
                *suit_binary_hole.get_unchecked_mut((h2 & 0x3) as usize) |=
                    *BIT_OF_DIV_4.get_unchecked(h2 as usize) as i32;
                *suit_binary_hole.get_unchecked_mut((h3 & 0x3) as usize) |=
                    *BIT_OF_DIV_4.get_unchecked(h3 as usize) as i32;
                *suit_binary_hole.get_unchecked_mut((h4 & 0x3) as usize) |=
                    *BIT_OF_DIV_4.get_unchecked(h4 as usize) as i32;
            }

            // SAFETY: i ∈ [0,3]; suit_binary_*[i] is at most a
            // 13-bit mask, so the OR is at most 13 bits → FLUSH index
            // ∈ [0, 8192). 5 - scb ∈ {0,1,2}, 4 - sch ∈ {0,1,2} for
            // PADDING (length 3).
            let sbb = unsafe { *suit_binary_board.get_unchecked(i) };
            let sbh = unsafe { *suit_binary_hole.get_unchecked(i) };

            if scb == 3 && sch == 2 {
                value_flush = unsafe { *FLUSH.get_unchecked((sbb | sbh) as usize) } as i32;
            } else {
                let board_padded = sbb | unsafe { *PADDING.get_unchecked((5 - scb) as usize) };
                let hole_padded = sbh | unsafe { *PADDING.get_unchecked((4 - sch) as usize) };

                let board_hash = hash_binary(board_padded, 5);
                let hole_hash = hash_binary(hole_padded, 4);

                // SAFETY: board_hash ∈ [0, 1365), hole_hash ∈ [0, 1365),
                // so combined index < 1365*1365 = 1_863_225 < 4_099_095.
                value_flush =
                    unsafe { *FLUSH_PLO4.get_unchecked((board_hash * 1365 + hole_hash) as usize) }
                        as i32;
            }

            break;
        }
    }

    let mut quinary_board = [0u8; 13];
    let mut quinary_hole = [0u8; 13];

    // SAFETY: c >> 2 ∈ [0, 12] (since c ∈ [0, 51]), within quinary_*'s
    // 13-element bounds.
    unsafe {
        *quinary_board.get_unchecked_mut((c1 >> 2) as usize) += 1;
        *quinary_board.get_unchecked_mut((c2 >> 2) as usize) += 1;
        *quinary_board.get_unchecked_mut((c3 >> 2) as usize) += 1;
        *quinary_board.get_unchecked_mut((c4 >> 2) as usize) += 1;
        *quinary_board.get_unchecked_mut((c5 >> 2) as usize) += 1;

        *quinary_hole.get_unchecked_mut((h1 >> 2) as usize) += 1;
        *quinary_hole.get_unchecked_mut((h2 >> 2) as usize) += 1;
        *quinary_hole.get_unchecked_mut((h3 >> 2) as usize) += 1;
        *quinary_hole.get_unchecked_mut((h4 >> 2) as usize) += 1;
    }

    let board_hash = hash_quinary(&quinary_board, 5);
    let hole_hash = hash_quinary(&quinary_hole, 4);

    // SAFETY: board_hash ∈ [0, 6175), hole_hash ∈ [0, 1820), so
    // combined index < 6175*1820 + 1820 ≈ 11.2M < NOFLUSH_PLO4.len().
    let value_noflush: i32 =
        unsafe { *NOFLUSH_PLO4.get_unchecked((board_hash * 1820 + hole_hash) as usize) } as i32;

    value_flush.min(value_noflush)
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn card(rank: i32, suit: i32) -> i32 {
        rank * 4 + suit
    }

    const TWO_C: i32 = card(0, 0);
    const TWO_D: i32 = card(0, 1);
    const TWO_H: i32 = card(0, 2);
    const TWO_S: i32 = card(0, 3);
    const THREE_C: i32 = card(1, 0);
    const THREE_H: i32 = card(1, 2);
    const THREE_S: i32 = card(1, 3);
    const FOUR_C: i32 = card(2, 0);
    const FIVE_C: i32 = card(3, 0);
    const FIVE_S: i32 = card(3, 3);
    const SEVEN_S: i32 = card(5, 3);
    const TEN_S: i32 = card(8, 3);
    const JACK_S: i32 = card(9, 3);
    const QUEEN_S: i32 = card(10, 3);
    const KING_S: i32 = card(11, 3);
    const ACE_C: i32 = card(12, 0);
    const ACE_D: i32 = card(12, 1);
    const ACE_H: i32 = card(12, 2);
    const ACE_S: i32 = card(12, 3);

    #[test]
    fn output_in_cactus_kev_range() {
        let r = evaluate_plo4_cards(
            ACE_S, KING_S, QUEEN_S, JACK_S, TEN_S, ACE_C, ACE_D, ACE_H, TWO_C,
        );
        assert!((1..=7462).contains(&r));
    }

    #[test]
    fn royal_straight_flush() {
        let r = evaluate_plo4_cards(
            QUEEN_S, JACK_S, TEN_S, TWO_C, THREE_C, ACE_S, KING_S, TWO_D, TWO_H,
        );
        assert_eq!(r, 1);
    }

    #[test]
    fn deterministic() {
        let r1 = evaluate_plo4_cards(
            FOUR_C, THREE_C, TWO_D, ACE_C, ACE_D, SEVEN_S, FIVE_S, KING_S, QUEEN_S,
        );
        let r2 = evaluate_plo4_cards(
            FOUR_C, THREE_C, TWO_D, ACE_C, ACE_D, SEVEN_S, FIVE_S, KING_S, QUEEN_S,
        );
        assert_eq!(r1, r2);
        assert!((1..=7462).contains(&r1));
    }

    #[test]
    fn pair_of_aces_ranges() {
        let r = evaluate_plo4_cards(
            KING_S, FIVE_C, FOUR_C, THREE_C, TWO_C, ACE_C, ACE_D, ACE_H, ACE_S,
        );
        assert!((3326..=6185).contains(&r));
    }

    #[test]
    fn flush_5card_special_case() {
        let r = evaluate_plo4_cards(
            card(3, 2),
            card(4, 2),
            card(5, 2),
            TWO_C,
            FOUR_C,
            TWO_H,
            THREE_H,
            KING_S,
            QUEEN_S,
        );
        assert!((323..=1599).contains(&r));
    }

    #[test]
    fn flush_beats_no_flush() {
        let board = (card(3, 2), card(4, 2), card(5, 2), TWO_C, FOUR_C);
        let with_flush = evaluate_plo4_cards(
            board.0, board.1, board.2, board.3, board.4, TWO_H, THREE_H, KING_S, QUEEN_S,
        );
        let no_flush = evaluate_plo4_cards(
            board.0, board.1, board.2, board.3, board.4, TWO_S, THREE_S, KING_S, QUEEN_S,
        );
        assert!(with_flush < no_flush);
    }
}
