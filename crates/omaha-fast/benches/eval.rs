//! Throughput micro-bench for `phe-omaha-fast::evaluate_plo4_cards`.
//! 10 000 deterministic random hands per iteration.

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use phe_omaha_fast::{evaluate_plo4_batch, evaluate_plo4_cards};

const SEED: u64 = 0xDEAD_BEEF_CAFE_BABE;
const NUM_FIXTURES: usize = 100_000;

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

fn fixtures_i32() -> Vec<([i32; 4], [i32; 5])> {
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

fn fixtures_u8() -> Vec<([u8; 4], [u8; 5])> {
    fixtures_i32()
        .iter()
        .map(|(h, b)| {
            ([h[0] as u8, h[1] as u8, h[2] as u8, h[3] as u8],
             [b[0] as u8, b[1] as u8, b[2] as u8, b[3] as u8, b[4] as u8])
        })
        .collect()
}

fn bench_single(c: &mut Criterion) {
    let fixtures = fixtures_i32();
    c.bench_function("plo4_eval_100k_random_single", |b| {
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

fn bench_batch(c: &mut Criterion) {
    let fixtures = fixtures_u8();
    let mut out = vec![0i32; fixtures.len()];
    c.bench_function("plo4_eval_100k_random_batch", |b| {
        b.iter(|| {
            evaluate_plo4_batch(black_box(&fixtures), black_box(&mut out));
            black_box(&out);
        });
    });
}

criterion_group!(benches, bench_single, bench_batch);
criterion_main!(benches);
