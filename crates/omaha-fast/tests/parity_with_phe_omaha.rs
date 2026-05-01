//! Cross-check `phe-omaha-fast` against the existing `phe-omaha`.
//!
//! The two implementations use different rank conventions:
//! - `phe-omaha-fast` returns Cactus-Kev rank in `[1, 7462]`,
//!   **lower = stronger** (matches HenryRLee's C output verbatim).
//! - `phe-omaha::OmahaHighRule::evaluate(hole, board)` returns a u16
//!   from phe-holdem's perfect-hash table, **higher = stronger**.
//!
//! Absolute values therefore differ. The meaningful invariant is
//! **ordering**: if `phe-omaha-fast` ranks A stronger than B, so must
//! `phe-omaha`. This test compares pairwise orderings on a deterministic
//! random sample of 1000 hands.

use phe_omaha::OmahaHighRule;
use phe_omaha_fast::evaluate_plo4_cards;

/// Tiny xorshift64 — no `rand` dep needed.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    fn pick(&mut self, n: usize) -> usize {
        (self.next() as usize) % n
    }
}

/// Sample a random PLO4 hand (4 hole + 5 board, all distinct cards).
fn sample_hand(rng: &mut Rng) -> ([usize; 4], [usize; 5]) {
    let mut deck: Vec<usize> = (0..52).collect();
    // Fisher-Yates draw 9.
    let mut drawn = [0usize; 9];
    for i in 0..9 {
        let pick = i + rng.pick(52 - i);
        deck.swap(i, pick);
        drawn[i] = deck[i];
    }
    let hole = [drawn[0], drawn[1], drawn[2], drawn[3]];
    let board = [drawn[4], drawn[5], drawn[6], drawn[7], drawn[8]];
    (hole, board)
}

#[test]
fn ordering_parity_1000_random_hands() {
    let mut rng = Rng(0xDEAD_BEEF_CAFE_BABE);
    let n = 1000;

    let mut fast_ranks = Vec::with_capacity(n);
    let mut omaha_ranks = Vec::with_capacity(n);

    for _ in 0..n {
        let (hole, board) = sample_hand(&mut rng);
        let fast = evaluate_plo4_cards(
            board[0] as i32, board[1] as i32, board[2] as i32,
            board[3] as i32, board[4] as i32,
            hole[0] as i32, hole[1] as i32, hole[2] as i32, hole[3] as i32,
        );
        let omaha = OmahaHighRule::evaluate(&hole, &board);
        fast_ranks.push(fast);
        omaha_ranks.push(omaha);
    }

    // Pairwise ordering check: for every pair (i, j), the two impls
    // must agree on which hand is stronger. Note convention flip:
    // fast=lower-stronger, omaha=higher-stronger.
    let mut mismatches = 0;
    let mut checked = 0;
    for i in 0..n {
        for j in (i + 1)..n {
            checked += 1;
            let fast_i_stronger = fast_ranks[i] < fast_ranks[j];
            let fast_j_stronger = fast_ranks[i] > fast_ranks[j];
            let omaha_i_stronger = omaha_ranks[i] > omaha_ranks[j];
            let omaha_j_stronger = omaha_ranks[i] < omaha_ranks[j];

            // Both must agree on tie / which-stronger.
            if fast_i_stronger != omaha_i_stronger || fast_j_stronger != omaha_j_stronger {
                mismatches += 1;
                if mismatches < 5 {
                    eprintln!(
                        "MISMATCH at i={i} j={j}: fast=({},{}) omaha=({},{})",
                        fast_ranks[i], fast_ranks[j],
                        omaha_ranks[i], omaha_ranks[j]
                    );
                }
            }
        }
    }
    assert_eq!(
        mismatches, 0,
        "{} pairwise ordering mismatches out of {} pairs",
        mismatches, checked
    );
}
