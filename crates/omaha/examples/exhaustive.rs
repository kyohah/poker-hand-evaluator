//! Single-thread comprehensive Omaha eval throughput bench.
//!
//! Iterates every distinct (4-card hole, 5-card board) configuration
//! with no card overlap. Total count:
//!
//!   C(52, 4) × C(48, 5) = 270,725 × 1,712,304 = 463,563,500,400.
//!
//! That's far more than fits in 10 s at the current per-eval cost
//! (~75 ns × 60-combo-or-fewer dispatch). The bench measures how
//! many configurations the optimised eval can crunch through in a
//! 10-second budget and reports throughput, so we have a baseline
//! single-thread number before adding the iteration-order and
//! branch-and-bound optimisations.
//!
//! Run with:
//!     cargo run --release -p phe-omaha --example exhaustive
//!
//! Single-threaded on purpose: the eval is memory-bound on the
//! shared LOOKUP / OFFSETS tables (~150 KB total, exceeds L1), so
//! adding threads tends to thrash L2 instead of helping.

use phe_omaha::OmahaHighRule;
use std::time::{Duration, Instant};

const TIME_BUDGET: Duration = Duration::from_secs(10);

fn main() {
    let start = Instant::now();
    let mut count: u64 = 0;
    let mut acc: u32 = 0;
    let mut stopped_early = false;

    // Six nested loops over distinct cards. Tracking used-cards via
    // a u64 bitmask is faster than `.contains()` because each check
    // is one AND + branch.
    'outer: for h0 in 0..52usize {
        let m0 = 1u64 << h0;
        for h1 in (h0 + 1)..52 {
            let m1 = m0 | (1u64 << h1);
            for h2 in (h1 + 1)..52 {
                let m2 = m1 | (1u64 << h2);
                for h3 in (h2 + 1)..52 {
                    let mhole = m2 | (1u64 << h3);
                    let hole = [h0, h1, h2, h3];

                    for b0 in 0..52 {
                        if mhole & (1u64 << b0) != 0 {
                            continue;
                        }
                        let mb0 = mhole | (1u64 << b0);
                        for b1 in (b0 + 1)..52 {
                            if mb0 & (1u64 << b1) != 0 {
                                continue;
                            }
                            let mb1 = mb0 | (1u64 << b1);
                            for b2 in (b1 + 1)..52 {
                                if mb1 & (1u64 << b2) != 0 {
                                    continue;
                                }
                                let mb2 = mb1 | (1u64 << b2);
                                for b3 in (b2 + 1)..52 {
                                    if mb2 & (1u64 << b3) != 0 {
                                        continue;
                                    }
                                    let mb3 = mb2 | (1u64 << b3);
                                    for b4 in (b3 + 1)..52 {
                                        if mb3 & (1u64 << b4) != 0 {
                                            continue;
                                        }
                                        let board = [b0, b1, b2, b3, b4];
                                        acc = acc.wrapping_add(
                                            OmahaHighRule::evaluate(&hole, &board) as u32,
                                        );
                                        count += 1;

                                        // Check time every ~4M evals
                                        // (cheap when the inner loop is hot).
                                        if count & ((1u64 << 22) - 1) == 0
                                            && start.elapsed() >= TIME_BUDGET
                                        {
                                            stopped_early = true;
                                            break 'outer;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    let elapsed = start.elapsed();
    let secs = elapsed.as_secs_f64();
    let total: u64 = 463_563_500_400;
    println!(
        "count       = {} ({:.6}% of {})",
        count,
        100.0 * count as f64 / total as f64,
        total
    );
    println!("elapsed     = {:.3} s", secs);
    println!(
        "throughput  = {:.1} M evals/sec ({:.1} ns/eval)",
        count as f64 / secs / 1e6,
        1e9 * secs / count as f64
    );
    println!("stopped     = {}", if stopped_early { "time" } else { "all" });
    println!("acc check   = {:#010x}", acc);
}
