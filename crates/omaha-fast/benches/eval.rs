//! Throughput micro-bench for `phe-omaha-fast::evaluate_plo4_cards`.
//! 10 000 deterministic random hands per iteration.

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use phe_omaha_fast::evaluate_plo4_cards;

const SEED: u64 = 0xDEAD_BEEF_CAFE_BABE;
const NUM_FIXTURES: usize = 10_000;

struct Rng(u64);
impl Rng {
    fn new(seed: u64) -> Self { Self(seed) }
    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    fn pick(&mut self, n: usize) -> usize {
        (self.next_u64() as usize) % n
    }
}

fn fixtures() -> Vec<([i32; 4], [i32; 5])> {
    let mut rng = Rng::new(SEED);
    let mut out = Vec::with_capacity(NUM_FIXTURES);
    for _ in 0..NUM_FIXTURES {
        let mut deck: [usize; 52] = [0; 52];
        for i in 0..52 { deck[i] = i; }
        for i in 0..9 {
            let pick = i + rng.pick(52 - i);
            deck.swap(i, pick);
        }
        let hole = [deck[0] as i32, deck[1] as i32, deck[2] as i32, deck[3] as i32];
        let board = [deck[4] as i32, deck[5] as i32, deck[6] as i32, deck[7] as i32, deck[8] as i32];
        out.push((hole, board));
    }
    out
}

fn bench_eval(c: &mut Criterion) {
    let fixtures = fixtures();
    c.bench_function("plo4_eval_10k_random", |b| {
        b.iter(|| {
            for (hole, board) in &fixtures {
                let r = evaluate_plo4_cards(
                    black_box(board[0]), black_box(board[1]), black_box(board[2]),
                    black_box(board[3]), black_box(board[4]),
                    black_box(hole[0]),  black_box(hole[1]),
                    black_box(hole[2]),  black_box(hole[3]),
                );
                black_box(r);
            }
        });
    });
}

criterion_group!(benches, bench_eval);
criterion_main!(benches);
