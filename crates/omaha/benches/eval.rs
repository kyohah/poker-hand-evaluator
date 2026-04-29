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
use phe_omaha::{
    board_has_no_pair, board_no_straight, evaluate_kev, evaluate_kev_v1,
    evaluate_kev_v2_always_flush, evaluate_kev_v3_always_hash, evaluate_straight_short_circuit,
    flush_possible, flush_suit, OmahaHighRule,
};
use std::sync::OnceLock;

const NUM_FIXTURES: usize = 10_000;
const SEED: u64 = 0xDEAD_BEEF_CAFE_BABE;
const SEED_STRUCTURED: u64 = 0xC0FF_EE_DEAD_BEEF;
const SEED_NO_STRAIGHT: u64 = 0xBADD_CAFE_F00D_F00D;

/// 10K random (hole, board) fixtures, generated **exactly once** for
/// the entire bench process. Both `optimized` and `naive` benches
/// read from the same slice, and `b.iter` just loops over it — no
/// regeneration per iteration.
static FIXTURES: OnceLock<Vec<([usize; 4], [usize; 5])>> = OnceLock::new();

/// 10K fixtures pre-filtered to the flush-dominates fast path: board
/// has no pair AND a flush_suit exists. Same OnceLock pattern.
static STRUCTURED_FIXTURES: OnceLock<Vec<([usize; 4], [usize; 5])>> = OnceLock::new();

/// 10K fixtures pre-filtered to the *most* restrictive structural
/// case: board no pair AND board no straight AND flush_suit exists.
/// Under these conditions FH/Quads AND Straight AND SF are all
/// unreachable, so the best 5-card hand can only be a Flush — pure
/// flush-domination, no SF dispatch needed.
static STRUCTURED_NO_STRAIGHT_FIXTURES: OnceLock<Vec<([usize; 4], [usize; 5])>> = OnceLock::new();

fn fixtures() -> &'static [([usize; 4], [usize; 5])] {
    FIXTURES.get_or_init(generate_fixtures)
}

fn structured_fixtures() -> &'static [([usize; 4], [usize; 5])] {
    STRUCTURED_FIXTURES.get_or_init(generate_structured_fixtures)
}

fn structured_no_straight_fixtures() -> &'static [([usize; 4], [usize; 5])] {
    STRUCTURED_NO_STRAIGHT_FIXTURES.get_or_init(generate_structured_no_straight_fixtures)
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

/// Builds 10K fixtures filtered to the maximally-restrictive case:
/// no pair AND no straight (so FH, Quads, Straight, SF all impossible)
/// AND flush eligible. The only reachable category at the top is
/// Flush — every combo is dominated by the flush combo.
fn generate_structured_no_straight_fixtures() -> Vec<([usize; 4], [usize; 5])> {
    let mut rng = Rng::new(SEED_NO_STRAIGHT);
    let mut fixtures = Vec::with_capacity(NUM_FIXTURES);
    while fixtures.len() < NUM_FIXTURES {
        let mut deck: [usize; 52] = std::array::from_fn(|i| i);
        for i in 0..9 {
            let j = i + (rng.next_u64() as usize) % (52 - i);
            deck.swap(i, j);
        }
        let hole = [deck[0], deck[1], deck[2], deck[3]];
        let board = [deck[4], deck[5], deck[6], deck[7], deck[8]];
        if board_has_no_pair(&board)
            && board_no_straight(&board)
            && flush_possible(&hole, &board)
        {
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

    // 8-cell structural breakdown so we can see how the various
    // impossibility predicates partition random Omaha hands.
    let mut counts = [[[0usize; 2]; 2]; 2]; // [no_pair][no_straight][flush_possible]
    for (h, b) in f {
        let i = board_has_no_pair(b) as usize;
        let j = board_no_straight(b) as usize;
        let k = flush_possible(h, b) as usize;
        counts[i][j][k] += 1;
    }
    eprintln!("random fixtures structural breakdown ({} total):", NUM_FIXTURES);
    for i in 0..2 {
        for j in 0..2 {
            for k in 0..2 {
                eprintln!(
                    "  no_pair={} no_straight={} flush={}: {} ({:.1}%)",
                    i,
                    j,
                    k,
                    counts[i][j][k],
                    100.0 * counts[i][j][k] as f64 / NUM_FIXTURES as f64,
                );
            }
        }
    }

    // Path split (which dispatch path the optimised eval would pick).
    let no_flush = f.iter().filter(|(h, b)| !flush_possible(h, b)).count();
    let flush_dominates = f
        .iter()
        .filter(|(h, b)| flush_suit(h, b).is_some() && board_has_no_pair(b))
        .count();
    let flush_with_pair = NUM_FIXTURES - no_flush - flush_dominates;
    eprintln!(
        "path dispatch: no-flush {} ({:.1}%) | flush-dominates {} ({:.1}%) | flush+board-pair {} ({:.1}%)",
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

    // Cactus-Kev experimental path: 60-combo enumeration with the
    // ~49 KB Kev tables instead of the 145 KB phe-holdem LOOKUP.
    group.bench_function("kev", |b| {
        b.iter(|| {
            let mut acc: u32 = 0;
            for (hole, board) in f {
                acc = acc.wrapping_add(evaluate_kev(hole, board) as u32);
            }
            black_box(acc)
        })
    });

    // Decomposition variants for the 2× slowdown root-cause analysis.
    group.bench_function("kev_v1_precomp", |b| {
        b.iter(|| {
            let mut acc: u32 = 0;
            for (hole, board) in f {
                acc = acc.wrapping_add(evaluate_kev_v1(hole, board) as u32);
            }
            black_box(acc)
        })
    });
    group.bench_function("kev_v2_always_flush", |b| {
        b.iter(|| {
            let mut acc: u32 = 0;
            for (hole, board) in f {
                acc = acc.wrapping_add(evaluate_kev_v2_always_flush(hole, board) as u32);
            }
            black_box(acc)
        })
    });
    group.bench_function("kev_v3_always_hash", |b| {
        b.iter(|| {
            let mut acc: u32 = 0;
            for (hole, board) in f {
                acc = acc.wrapping_add(evaluate_kev_v3_always_hash(hole, board) as u32);
            }
            black_box(acc)
        })
    });

    // Batch API + path-1 prefetch: overlaps the 22 MB no-flush
    // table's memory latency with subsequent iterations' compute.
    group.bench_function("optimized_batch", |b| {
        let mut out = vec![0u16; f.len()];
        b.iter(|| {
            OmahaHighRule::evaluate_batch(f, &mut out);
            let acc: u32 = out.iter().map(|&v| v as u32).sum();
            black_box(acc)
        })
    });

    // User-proposed straight-short-circuit: precomputed structural
    // detection of "Straight is the max" hands; falls back to
    // production eval otherwise.
    group.bench_function("straight_short_circuit", |b| {
        b.iter(|| {
            let mut acc: u32 = 0;
            for (hole, board) in f {
                acc = acc.wrapping_add(evaluate_straight_short_circuit(hole, board) as u32);
            }
            black_box(acc)
        })
    });

    // Frequency report: how often does the short-circuit actually fire?
    let mut hits = 0usize;
    for (hole, board) in f {
        let prod = OmahaHighRule::evaluate(hole, board);
        let ssc = evaluate_straight_short_circuit(hole, board);
        // SSC equals prod by definition (cross-check tests pass).
        // Detect fast-path firing: it's exactly when prod is a Straight
        // (cat 4) AND it would have come from the SSC path. Approximate
        // by counting Straight outcomes on the no-flush, no-pair-board
        // hands (which is what the SSC condition allows).
        let _ = (prod, ssc);
        let mut hole_s = [0u8; 4];
        let mut board_s = [0u8; 4];
        for &c in hole { hole_s[c & 3] += 1; }
        for &c in board { board_s[c & 3] += 1; }
        let flush_eligible = (0..4).any(|s| hole_s[s] >= 2 && board_s[s] >= 3);
        if flush_eligible { continue; }
        if !board_has_no_pair(board) { continue; }
        if (prod >> 12) == 4 {
            hits += 1;
        }
    }
    eprintln!(
        "straight_short_circuit fires on {} / {} ({:.2}%) of fixtures",
        hits,
        NUM_FIXTURES,
        100.0 * hits as f64 / NUM_FIXTURES as f64,
    );

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

fn bench_structured_no_straight(c: &mut Criterion) {
    let f = structured_no_straight_fixtures();
    eprintln!(
        "no-straight fixtures: {} total — board no pair AND no straight \
         AND flush eligible (FH/Quads/Straight/SF all unreachable; only Flush at top).",
        NUM_FIXTURES,
    );

    let mut group = c.benchmark_group("omaha_eval_10k_no_straight");

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

criterion_group!(
    benches,
    bench_random_10k,
    bench_structured_flush_dominates,
    bench_structured_no_straight,
);
criterion_main!(benches);
