//! Hold'em high benchmark: `HighRule::evaluate` over 10K random
//! 5/6/7-card fixtures.
//!
//! Fixtures are seeded for reproducibility. `b.iter` only loops over
//! pre-built fixtures — fixture generation cost is excluded from the
//! reported time.

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use phe_core::Hand;
use phe_holdem::HighRule;
use std::sync::OnceLock;

const NUM_FIXTURES: usize = 10_000;

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

fn random_hands(seed: u64, k: usize) -> Vec<Hand> {
    let mut rng = Rng::new(seed);
    let mut out = Vec::with_capacity(NUM_FIXTURES);
    for _ in 0..NUM_FIXTURES {
        let mut deck: [usize; 52] = std::array::from_fn(|i| i);
        for i in 0..k {
            let j = i + (rng.next_u64() as usize) % (52 - i);
            deck.swap(i, j);
        }
        out.push(Hand::from_slice(&deck[..k]));
    }
    out
}

static HANDS_5: OnceLock<Vec<Hand>> = OnceLock::new();
static HANDS_6: OnceLock<Vec<Hand>> = OnceLock::new();
static HANDS_7: OnceLock<Vec<Hand>> = OnceLock::new();

fn bench_holdem(c: &mut Criterion) {
    let h5 = HANDS_5.get_or_init(|| random_hands(0x0000_0000_0AAA_0005, 5));
    let h6 = HANDS_6.get_or_init(|| random_hands(0x0000_0000_0AAA_0006, 6));
    let h7 = HANDS_7.get_or_init(|| random_hands(0x0000_0000_0AAA_0007, 7));

    let mut g = c.benchmark_group("holdem_eval_10k");
    g.bench_function("5card", |b| {
        b.iter(|| {
            let mut acc: u32 = 0;
            for h in h5 {
                acc = acc.wrapping_add(HighRule::evaluate(h) as u32);
            }
            black_box(acc)
        })
    });
    g.bench_function("6card", |b| {
        b.iter(|| {
            let mut acc: u32 = 0;
            for h in h6 {
                acc = acc.wrapping_add(HighRule::evaluate(h) as u32);
            }
            black_box(acc)
        })
    });
    g.bench_function("7card", |b| {
        b.iter(|| {
            let mut acc: u32 = 0;
            for h in h7 {
                acc = acc.wrapping_add(HighRule::evaluate(h) as u32);
            }
            black_box(acc)
        })
    });
    g.finish();
}

criterion_group!(benches, bench_holdem);
criterion_main!(benches);
