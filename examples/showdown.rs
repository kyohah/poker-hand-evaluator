//! Compare two Hold'em hands at showdown and pick the winner.
//!
//! Run with:
//! ```sh
//! cargo run --example showdown
//! ```

use poker_hand_evaluator::{HandRule, HighRule};

/// `card = rank * 4 + suit`. Rank `0='2', …, 12='A'`. Suit `0=c, 1=d,
/// 2=h, 3=s`.
const fn card(rank: u8, suit: u8) -> u8 {
    rank * 4 + suit
}

fn main() {
    // Board: K♠ Q♠ J♠ 5♣ 2♥
    let board = [
        card(11, 3), // King of spades
        card(10, 3), // Queen of spades
        card(9, 3),  // Jack of spades
        card(3, 0),  // Five of clubs
        card(0, 2),  // Two of hearts
    ];

    // Player A: A♠ T♠  → makes a royal flush with the board
    let a_hole = [card(12, 3), card(8, 3)];
    // Player B: A♥ A♦  → just a pair of aces
    let b_hole = [card(12, 2), card(12, 1)];

    let a_cards: Vec<u8> = a_hole.iter().chain(board.iter()).copied().collect();
    let b_cards: Vec<u8> = b_hole.iter().chain(board.iter()).copied().collect();

    let a_strength = HighRule.evaluate(&a_cards);
    let b_strength = HighRule.evaluate(&b_cards);

    println!("Player A (A♠ T♠):  strength = {a_strength:5}  (royal flush expected)");
    println!("Player B (A♥ A♦):  strength = {b_strength:5}  (pair of aces expected)");

    use std::cmp::Ordering;
    match a_strength.cmp(&b_strength) {
        Ordering::Greater => println!("=> Player A wins"),
        Ordering::Less => println!("=> Player B wins"),
        Ordering::Equal => println!("=> Tie"),
    }
}
