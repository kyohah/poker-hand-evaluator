//! Throughput micro-bench for `phe-omaha-fast`.
//!
//! Compares single-hand vs batch APIs on 100 000 random hands. The
//! large fixture set is chosen so the NOFLUSH_PLO4 access pattern
//! exceeds L3 cache size — the regime where batch + prefetch wins.

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use phe_omaha_fast::{evaluate_plo4_batch, evaluate_plo4_cards};

const SEED: u64 = 0xDEAD_BEEF_CAFE_BABE;
const NUM_FIXTURES: usize = 100_000;

struct Rng(u64);
impl Rng {
    fn new(seed: u64) -> Self {
        Self(seed)
    }
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
        for (i, slot) in deck.iter_mut().enumerate() {
            *slot = i;
        }
        for i in 0..9 {
            let pick = i + rng.pick(52 - i);
            deck.swap(i, pick);
        }
        let hole = [
            deck[0] as i32,
            deck[1] as i32,
            deck[2] as i32,
            deck[3] as i32,
        ];
        let board = [
            deck[4] as i32,
            deck[5] as i32,
            deck[6] as i32,
            deck[7] as i32,
            deck[8] as i32,
        ];
        out.push((hole, board));
    }
    out
}

fn fixtures_u8() -> Vec<([u8; 4], [u8; 5])> {
    fixtures_i32()
        .iter()
        .map(|(h, b)| {
            (
                [h[0] as u8, h[1] as u8, h[2] as u8, h[3] as u8],
                [b[0] as u8, b[1] as u8, b[2] as u8, b[3] as u8, b[4] as u8],
            )
        })
        .collect()
}

fn bench_single(c: &mut Criterion) {
    let fixtures = fixtures_i32();
    c.bench_function("plo4_eval_100k_random_single", |b| {
        b.iter(|| {
            for (hole, board) in &fixtures {
                let r = evaluate_plo4_cards(
                    black_box(board[0]),
                    black_box(board[1]),
                    black_box(board[2]),
                    black_box(board[3]),
                    black_box(board[4]),
                    black_box(hole[0]),
                    black_box(hole[1]),
                    black_box(hole[2]),
                    black_box(hole[3]),
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

/// Lower-bound bench — only computes the noflush index for each hand,
/// no NOFLUSH_PLO4 lookup. Tells us how much of the batch time is the
/// pass-1 CPU work (hash_quinary × 2 + histograms).
fn bench_pass1_only(c: &mut Criterion) {
    let fixtures = fixtures_u8();
    use phe_omaha_fast::dp::DP;
    c.bench_function("plo4_pass1_only_100k", |b| {
        b.iter(|| {
            let mut acc: u32 = 0;
            for (hole, board) in &fixtures {
                let mut qb = [0u8; 13];
                let mut qh = [0u8; 13];
                qb[(board[0] >> 2) as usize] += 1;
                qb[(board[1] >> 2) as usize] += 1;
                qb[(board[2] >> 2) as usize] += 1;
                qb[(board[3] >> 2) as usize] += 1;
                qb[(board[4] >> 2) as usize] += 1;
                qh[(hole[0] >> 2) as usize] += 1;
                qh[(hole[1] >> 2) as usize] += 1;
                qh[(hole[2] >> 2) as usize] += 1;
                qh[(hole[3] >> 2) as usize] += 1;
                // hash_quinary inline, branchless 13-iter
                let mut sb: u32 = 0;
                let mut k = 5i32;
                for i in 0..13 {
                    let kk = k.max(0) as usize;
                    sb = sb.wrapping_add(DP[qb[i] as usize][12 - i][kk]);
                    k -= qb[i] as i32;
                }
                let mut sh: u32 = 0;
                let mut k = 4i32;
                for i in 0..13 {
                    let kk = k.max(0) as usize;
                    sh = sh.wrapping_add(DP[qh[i] as usize][12 - i][kk]);
                    k -= qh[i] as i32;
                }
                acc ^= sb.wrapping_mul(1820).wrapping_add(sh);
                black_box(acc);
            }
        });
    });
}

criterion_group!(benches, bench_single, bench_batch, bench_pass1_only);
criterion_main!(benches);
