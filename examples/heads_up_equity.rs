//! Heads-up equity for a fixed pair of Hold'em hole hands, evaluated
//! by enumerating every possible 5-card board (~2.6 M).
//!
//! Run with:
//! ```sh
//! cargo run --release --example heads_up_equity
//! ```
//!
//! Release mode is essential — debug build runs the inner loop ~30×
//! slower.

use poker_hand_evaluator::{HandRule, HighRule};
use std::time::Instant;

const fn card(rank: u8, suit: u8) -> u8 {
    rank * 4 + suit
}

fn main() {
    // Player A: A♠ K♠
    // Player B: Q♥ Q♦
    let a_hole = [card(12, 3), card(11, 3)];
    let b_hole = [card(10, 2), card(10, 1)];

    let dead = [a_hole[0], a_hole[1], b_hole[0], b_hole[1]];
    let live: Vec<u8> = (0u8..52).filter(|c| !dead.contains(c)).collect();
    assert_eq!(live.len(), 48);

    let mut a_wins = 0u64;
    let mut b_wins = 0u64;
    let mut ties = 0u64;
    let mut total = 0u64;

    let t0 = Instant::now();
    for i0 in 0..live.len() {
        for i1 in (i0 + 1)..live.len() {
            for i2 in (i1 + 1)..live.len() {
                for i3 in (i2 + 1)..live.len() {
                    for i4 in (i3 + 1)..live.len() {
                        let board = [live[i0], live[i1], live[i2], live[i3], live[i4]];
                        let mut a_cards = [0u8; 7];
                        a_cards[..2].copy_from_slice(&a_hole);
                        a_cards[2..].copy_from_slice(&board);
                        let mut b_cards = [0u8; 7];
                        b_cards[..2].copy_from_slice(&b_hole);
                        b_cards[2..].copy_from_slice(&board);

                        let a = HighRule.evaluate(&a_cards);
                        let b = HighRule.evaluate(&b_cards);
                        match a.cmp(&b) {
                            std::cmp::Ordering::Greater => a_wins += 1,
                            std::cmp::Ordering::Less => b_wins += 1,
                            std::cmp::Ordering::Equal => ties += 1,
                        }
                        total += 1;
                    }
                }
            }
        }
    }
    let dt = t0.elapsed();

    let pct = |n: u64| 100.0 * n as f64 / total as f64;
    println!("AKs vs QQ on a random board ({total} boards enumerated):");
    println!("  AKs wins: {a_wins:>9}  ({:.2}%)", pct(a_wins));
    println!("  QQ  wins: {b_wins:>9}  ({:.2}%)", pct(b_wins));
    println!("  ties:     {ties:>9}  ({:.2}%)", pct(ties));
    println!(
        "  elapsed:  {dt:?} ({:.1} M boards/s)",
        total as f64 / dt.as_secs_f64() / 1e6
    );
}
