//! Single-thread random-stream Omaha eval throughput bench.
//!
//! Generates a fresh random (4-card hole, 5-card board) configuration
//! every iteration and runs the optimised eval, looped for ~10 s
//! wall-clock. Reports total count and ns/eval, and auto-appends a
//! row to `bench/history.csv` with commit hash + Unix timestamp so
//! optimisation progress can be tracked across edits.
//!
//! Random (rather than full enumeration) so:
//!   - we don't pay enumeration overhead on top of eval cost;
//!   - the LOOKUP / OFFSETS access pattern stays scattered (closer
//!     to real solver workloads) instead of artificially structured;
//!   - throughput numbers are directly comparable to the criterion
//!     `omaha_eval_10k_random` group (just at much larger sample
//!     count).
//!
//! Single-threaded on purpose: the eval is memory-bound on the
//! shared LOOKUP / OFFSETS tables (~150 KB total, exceeds L1), so
//! adding threads thrashes L2 instead of helping.
//!
//! Run with:
//!     cargo run --release -p phe-omaha --example exhaustive

use phe_omaha::OmahaHighRule;
use std::fs::OpenOptions;
use std::io::Write;
use std::process::Command;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const TIME_BUDGET: Duration = Duration::from_secs(10);
const SEED: u64 = 0xDEAD_BEEF_CAFE_BABE;
const BATCH: u64 = 1 << 18; // ~260K iters between time checks
const CSV_PATH: &str = "bench/history.csv";
const CSV_HEADER: &str = "unix_ts,commit,bench,count,elapsed_s,mevals_per_s,ns_per_eval";
const BENCH_NAME: &str = "exhaustive_random_10s";

/// Linear-congruential PRNG (PCG-style constants). Reseed-free,
/// fast — not crypto, just enough randomness for fixture spread.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Self(seed)
    }
    #[inline(always)]
    fn next_u64(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0
    }
}

/// Fisher-Yates partial shuffle: generate 9 distinct cards in [0,52)
/// from the deck, return as (hole, board).
#[inline(always)]
fn random_hand(rng: &mut Rng) -> ([usize; 4], [usize; 5]) {
    let mut deck: [usize; 52] = std::array::from_fn(|i| i);
    for i in 0..9 {
        let j = i + (rng.next_u64() as usize) % (52 - i);
        deck.swap(i, j);
    }
    (
        [deck[0], deck[1], deck[2], deck[3]],
        [deck[4], deck[5], deck[6], deck[7], deck[8]],
    )
}

fn main() {
    let mut rng = Rng::new(SEED);
    let start = Instant::now();
    let mut count: u64 = 0;
    let mut acc: u32 = 0;

    loop {
        // Tight inner batch — avoid timing overhead per iter.
        for _ in 0..BATCH {
            let (hole, board) = random_hand(&mut rng);
            acc = acc.wrapping_add(OmahaHighRule::evaluate(&hole, &board) as u32);
        }
        count += BATCH;
        if start.elapsed() >= TIME_BUDGET {
            break;
        }
    }

    let elapsed = start.elapsed();
    let secs = elapsed.as_secs_f64();
    let mevals = count as f64 / secs / 1e6;
    let ns_per_eval = 1e9 * secs / count as f64;
    println!("count       = {}", count);
    println!("elapsed     = {:.3} s", secs);
    println!(
        "throughput  = {:.1} M evals/sec ({:.1} ns/eval)",
        mevals, ns_per_eval
    );
    println!("acc check   = {:#010x}", acc);

    // Auto-append to bench/history.csv with commit + timestamp so
    // we can track optimisation progress across edits.
    if let Err(e) = append_history(count, secs, mevals, ns_per_eval) {
        eprintln!("warning: failed to append CSV history: {}", e);
    } else {
        println!("history     = appended to {}", CSV_PATH);
    }
}

fn append_history(
    count: u64,
    elapsed_s: f64,
    mevals_per_s: f64,
    ns_per_eval: f64,
) -> std::io::Result<()> {
    if let Some(parent) = std::path::Path::new(CSV_PATH).parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    let exists = std::path::Path::new(CSV_PATH).exists();
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(CSV_PATH)?;
    if !exists {
        writeln!(file, "{}", CSV_HEADER)?;
    }
    writeln!(
        file,
        "{},{},{},{},{:.4},{:.2},{:.2}",
        unix_ts(),
        git_commit_short(),
        BENCH_NAME,
        count,
        elapsed_s,
        mevals_per_s,
        ns_per_eval
    )?;
    Ok(())
}

fn unix_ts() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn git_commit_short() -> String {
    Command::new("git")
        .args(["rev-parse", "--short=8", "HEAD"])
        .output()
        .ok()
        .filter(|out| out.status.success())
        .and_then(|out| String::from_utf8(out.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string())
}
