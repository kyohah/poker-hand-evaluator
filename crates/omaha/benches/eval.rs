//! Benchmark: optimised `OmahaHighRule::evaluate` vs naive 60-combo
//! enumeration over 10,000 deterministically random (hole, board)
//! configurations.
//!
//! The fixtures are seeded so the same 10K hands are used every run;
//! that lets numbers be compared across edits without sample-size
//! noise. The optimised path's two main wins are visible separately
//! by looking at the `path_breakdown` reporter, which prints how many
//! fixtures hit the rank-only fast path vs the full Hold'em-eval path.

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use phe_core::Hand;
use phe_holdem::HighRule;
use phe_omaha::{board_has_no_pair, flush_possible, flush_suit, OmahaHighRule};
use std::sync::OnceLock;

const NUM_FIXTURES: usize = 10_000;
const SEED: u64 = 0xDEAD_BEEF_CAFE_BABE;
const SEED_STRUCTURED: u64 = 0xC0FF_EE_DEAD_BEEF;

/// 10K random (hole, board) fixtures, generated **exactly once** for
/// the entire bench process. Both `optimized` and `naive` benches
/// read from the same slice, and `b.iter` just loops over it — no
/// regeneration per iteration.
static FIXTURES: OnceLock<Vec<([usize; 4], [usize; 5])>> = OnceLock::new();

/// 10K fixtures pre-filtered to the flush-dominates fast path: board
/// has no pair AND a flush_suit exists. Same OnceLock pattern.
static STRUCTURED_FIXTURES: OnceLock<Vec<([usize; 4], [usize; 5])>> = OnceLock::new();

fn fixtures() -> &'static [([usize; 4], [usize; 5])] {
    FIXTURES.get_or_init(generate_fixtures)
}

fn structured_fixtures() -> &'static [([usize; 4], [usize; 5])] {
    STRUCTURED_FIXTURES.get_or_init(generate_structured_fixtures)
}

/// Linear-congruential PRNG (PCG-style constants). Enough randomness
/// for fixture generation; reproducibility comes from the fixed seed.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Self(seed)
    }
    fn next_u64(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0
    }
}

/// Builds 10K deterministic random (hole, board) configurations.
/// Each hand uses 9 distinct cards via partial Fisher-Yates shuffle.
fn generate_fixtures() -> Vec<([usize; 4], [usize; 5])> {
    let mut rng = Rng::new(SEED);
    let mut fixtures = Vec::with_capacity(NUM_FIXTURES);
    for _ in 0..NUM_FIXTURES {
        let mut deck: [usize; 52] = std::array::from_fn(|i| i);
        for i in 0..9 {
            let j = i + (rng.next_u64() as usize) % (52 - i);
            deck.swap(i, j);
        }
        fixtures.push((
            [deck[0], deck[1], deck[2], deck[3]],
            [deck[4], deck[5], deck[6], deck[7], deck[8]],
        ));
    }
    fixtures
}

/// Builds 10K deterministic random (hole, board) configurations
/// **filtered** to the flush-dominates fast path: board has 5 distinct
/// ranks AND some suit has both ≥2 hole and ≥3 board cards.
fn generate_structured_fixtures() -> Vec<([usize; 4], [usize; 5])> {
    let mut rng = Rng::new(SEED_STRUCTURED);
    let mut fixtures = Vec::with_capacity(NUM_FIXTURES);
    while fixtures.len() < NUM_FIXTURES {
        let mut deck: [usize; 52] = std::array::from_fn(|i| i);
        for i in 0..9 {
            let j = i + (rng.next_u64() as usize) % (52 - i);
            deck.swap(i, j);
        }
        let hole = [deck[0], deck[1], deck[2], deck[3]];
        let board = [deck[4], deck[5], deck[6], deck[7], deck[8]];
        if board_has_no_pair(&board) && flush_possible(&hole, &board) {
            fixtures.push((hole, board));
        }
    }
    fixtures
}

/// Naive reference: enumerate every (2 hole + 3 board) and take the
/// max via the full Hold'em eval. No suit-aware dispatch, no board
/// partial caching.
#[inline]
fn naive_eval(hole: &[usize; 4], board: &[usize; 5]) -> u16 {
    const HOLE_PAIRS: [(usize, usize); 6] =
        [(0, 1), (0, 2), (0, 3), (1, 2), (1, 3), (2, 3)];
    const BOARD_TRIPLES: [(usize, usize, usize); 10] = [
        (0, 1, 2), (0, 1, 3), (0, 1, 4), (0, 2, 3), (0, 2, 4),
        (0, 3, 4), (1, 2, 3), (1, 2, 4), (1, 3, 4), (2, 3, 4),
    ];
    let mut best = 0u16;
    for &(i, j) in &HOLE_PAIRS {
        let hp = Hand::new().add_card(hole[i]).add_card(hole[j]);
        for &(a, b, c) in &BOARD_TRIPLES {
            let h = hp
                .add_card(board[a])
                .add_card(board[b])
                .add_card(board[c]);
            best = best.max(HighRule::evaluate(&h));
        }
    }
    best
}

fn bench_random_10k(c: &mut Criterion) {
    let f = fixtures();

    // Report path split for the random fixtures.
    let no_flush = f.iter().filter(|(h, b)| !flush_possible(h, b)).count();
    let flush_dominates = f
        .iter()
        .filter(|(h, b)| flush_suit(h, b).is_some() && board_has_no_pair(b))
        .count();
    let flush_with_pair = NUM_FIXTURES - no_flush - flush_dominates;
    eprintln!(
        "random fixtures path split: \
         no-flush {} ({:.1}%) | flush-dominates {} ({:.1}%) | flush+board-pair {} ({:.1}%)",
        no_flush,
        100.0 * no_flush as f64 / NUM_FIXTURES as f64,
        flush_dominates,
        100.0 * flush_dominates as f64 / NUM_FIXTURES as f64,
        flush_with_pair,
        100.0 * flush_with_pair as f64 / NUM_FIXTURES as f64,
    );

    let mut group = c.benchmark_group("omaha_eval_10k_random");

    group.bench_function("optimized", |b| {
        b.iter(|| {
            let mut acc: u32 = 0;
            for (hole, board) in f {
                acc = acc.wrapping_add(OmahaHighRule::evaluate(hole, board) as u32);
            }
            black_box(acc)
        })
    });

    group.bench_function("naive", |b| {
        b.iter(|| {
            let mut acc: u32 = 0;
            for (hole, board) in f {
                acc = acc.wrapping_add(naive_eval(hole, board) as u32);
            }
            black_box(acc)
        })
    });

    group.finish();
}

fn bench_structured_flush_dominates(c: &mut Criterion) {
    let f = structured_fixtures();
    eprintln!(
        "structured fixtures: {} total, all hit the flush-dominates fast path \
         (board no pair + flush suit eligible).",
        NUM_FIXTURES,
    );

    let mut group = c.benchmark_group("omaha_eval_10k_flush_dominates");

    group.bench_function("optimized", |b| {
        b.iter(|| {
            let mut acc: u32 = 0;
            for (hole, board) in f {
                acc = acc.wrapping_add(OmahaHighRule::evaluate(hole, board) as u32);
            }
            black_box(acc)
        })
    });

    group.bench_function("naive", |b| {
        b.iter(|| {
            let mut acc: u32 = 0;
            for (hole, board) in f {
                acc = acc.wrapping_add(naive_eval(hole, board) as u32);
            }
            black_box(acc)
        })
    });

    group.finish();
}

criterion_group!(benches, bench_random_10k, bench_structured_flush_dominates);
criterion_main!(benches);
