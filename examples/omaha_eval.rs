//! Evaluate a single PLO4 (Omaha) hand and print its Cactus-Kev rank.
//!
//! Run with:
//! ```sh
//! cargo run --example omaha_eval
//! ```

use poker_hand_evaluator::omaha::evaluate_plo4_cards_arr;

const fn card(rank: u8, suit: u8) -> u8 {
    rank * 4 + suit
}

fn main() {
    // PLO4 forces "exactly 2 from hole + exactly 3 from board" — a
    // royal flush in PLO4 therefore needs at least 2 hole + 3 board
    // cards of one suit.
    //
    // Board: T♠ J♠ Q♠ 7♣ 2♥
    // Hole:  K♠ A♠ + 2♦ 3♦ → A♠K♠ from hole + Q♠J♠T♠ from board.
    let board = [
        card(8, 3),  // Ten of spades
        card(9, 3),  // Jack of spades
        card(10, 3), // Queen of spades
        card(5, 0),  // Seven of clubs
        card(0, 2),  // Two of hearts
    ];
    let hole = [
        card(11, 3), // King of spades
        card(12, 3), // Ace of spades
        card(0, 1),  // Two of diamonds
        card(1, 1),  // Three of diamonds
    ];

    let rank = evaluate_plo4_cards_arr(board, hole);
    println!("Royal flush (A♠K♠ + Q♠J♠T♠) — Cactus-Kev rank = {rank}");
    println!("(rank 1 = strongest possible hand)");
    assert_eq!(rank, 1, "royal flush must be rank 1");

    // A more realistic hand:
    // Board: A♥ A♦ K♣ J♠ 2♠
    // Hole:  A♠ A♣ Q♥ T♦
    // Best 2-from-hole + 3-from-board: A♠A♣ + A♥A♦K♣ = quad aces, kicker K
    let board = [
        card(12, 2),
        card(12, 1),
        card(11, 0),
        card(9, 3),
        card(0, 3),
    ];
    let hole = [card(12, 3), card(12, 0), card(10, 2), card(8, 1)];
    let rank = evaluate_plo4_cards_arr(board, hole);
    println!("\nQuad aces (kicker K) — Cactus-Kev rank = {rank}");
    println!("(quads are ranks 11-166; the K kicker puts us near the top of that band)");
}
