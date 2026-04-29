//! 2-7 lowball benchmark: `DeuceSevenLowRule::evaluate` over 10K
//! random 5-card fixtures (the rule supports 5 cards only).

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use phe_core::Hand;
use phe_deuce_seven::DeuceSevenLowRule;
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

fn random_5card_hands(seed: u64) -> Vec<Hand> {
    let mut rng = Rng::new(seed);
    let mut out = Vec::with_capacity(NUM_FIXTURES);
    for _ in 0..NUM_FIXTURES {
        let mut deck: [usize; 52] = std::array::from_fn(|i| i);
        for i in 0..5 {
            let j = i + (rng.next_u64() as usize) % (52 - i);
            deck.swap(i, j);
        }
        out.push(Hand::from_slice(&deck[..5]));
    }
    out
}

static HANDS: OnceLock<Vec<Hand>> = OnceLock::new();

fn bench_deuce_seven(c: &mut Criterion) {
    let hands = HANDS.get_or_init(|| random_5card_hands(0x0000_0000_2070_DEAD));

    let mut g = c.benchmark_group("deuce_seven_eval_10k");
    g.bench_function("5card", |b| {
        b.iter(|| {
            let mut acc: u32 = 0;
            for h in hands {
                acc = acc.wrapping_add(DeuceSevenLowRule::evaluate(h).0 as u32);
            }
            black_box(acc)
        })
    });
    g.finish();
}

criterion_group!(benches, bench_deuce_seven);
criterion_main!(benches);
