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
use phe_omaha::{flush_possible, OmahaHighRule};

const NUM_FIXTURES: usize = 10_000;
const SEED: u64 = 0xDEAD_BEEF_CAFE_BABE;

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

/// Builds 10K deterministic (hole, board) configurations. Each hand
/// uses 9 distinct cards drawn from the 52-card deck via Fisher-Yates
/// partial shuffle (first 9 positions only).
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
    let fixtures = generate_fixtures();

    // Report fast-path vs full-path split so wins are interpretable.
    let flush_count = fixtures
        .iter()
        .filter(|(h, b)| flush_possible(h, b))
        .count();
    eprintln!(
        "fixtures: {} total | rank-only fast path: {} ({:.1}%) | full path: {} ({:.1}%)",
        NUM_FIXTURES,
        NUM_FIXTURES - flush_count,
        100.0 * (NUM_FIXTURES - flush_count) as f64 / NUM_FIXTURES as f64,
        flush_count,
        100.0 * flush_count as f64 / NUM_FIXTURES as f64,
    );

    let mut group = c.benchmark_group("omaha_eval_10k");

    group.bench_function("optimized", |b| {
        b.iter(|| {
            let mut acc: u32 = 0;
            for (hole, board) in &fixtures {
                acc = acc.wrapping_add(OmahaHighRule::evaluate(hole, board) as u32);
            }
            black_box(acc)
        })
    });

    group.bench_function("naive", |b| {
        b.iter(|| {
            let mut acc: u32 = 0;
            for (hole, board) in &fixtures {
                acc = acc.wrapping_add(naive_eval(hole, board) as u32);
            }
            black_box(acc)
        })
    });

    group.finish();
}

criterion_group!(benches, bench_random_10k);
criterion_main!(benches);
