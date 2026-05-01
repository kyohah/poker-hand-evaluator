//! CPU vs GPU Hold'em throughput at varying batch sizes (5/6/7-card).
//!
//! Run with:
//! ```text
//! cargo bench -p phe-holdem --bench eval_cuda --features cuda
//! ```
//! NVRTC must be on PATH (Windows: add CUDA toolkit's `bin/x64`).

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use phe_core::Hand;
use phe_holdem::cuda::HoldemEvalContext;
use phe_holdem::HighRule;

const SEED: u64 = 0xDEAD_BEEF_FACE_CAFE;
const SIZES: &[usize] = &[1_000, 10_000, 100_000, 1_000_000];

struct Rng(u64);
impl Rng {
    fn new(s: u64) -> Self { Self(s) }
    fn next_u64(&mut self) -> u64 {
        let mut x = self.0; x ^= x << 13; x ^= x >> 7; x ^= x << 17; self.0 = x; x
    }
    fn pick(&mut self, n: usize) -> usize { (self.next_u64() as usize) % n }
}

/// Returns `n` hands of `cards_per_hand` cards each, flattened.
fn fixtures(n: usize, cards_per_hand: usize) -> Vec<u8> {
    let mut rng = Rng::new(SEED);
    let mut out = Vec::with_capacity(n * cards_per_hand);
    for _ in 0..n {
        let mut deck: [u8; 52] = [0; 52];
        for i in 0..52 { deck[i] = i as u8; }
        for i in 0..cards_per_hand {
            let p = i + rng.pick(52 - i);
            deck.swap(i, p);
        }
        for i in 0..cards_per_hand {
            out.push(deck[i]);
        }
    }
    out
}

fn bench_cpu_7card(c: &mut Criterion) {
    let mut group = c.benchmark_group("holdem_cpu_7card");
    for &n in SIZES {
        let cards = fixtures(n, 7);
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| {
                let mut acc: u32 = 0;
                for i in 0..n {
                    let mut hand = Hand::new();
                    for j in 0..7 {
                        hand = hand.add_card(cards[i * 7 + j] as usize);
                    }
                    let r = HighRule::evaluate(&hand);
                    acc = acc.wrapping_add(r as u32);
                }
                black_box(acc);
            });
        });
    }
    group.finish();
}

fn bench_gpu_host_7card(c: &mut Criterion) {
    let ctx = match HoldemEvalContext::new() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("skip GPU host bench (init failed): {e}");
            return;
        }
    };
    let mut group = c.benchmark_group("holdem_gpu_host_7card");
    for &n in SIZES {
        let cards = fixtures(n, 7);
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| {
                let r = ctx.evaluate_batch(black_box(&cards), 7).expect("kernel");
                black_box(r);
            });
        });
    }
    group.finish();
}

fn bench_gpu_device_7card(c: &mut Criterion) {
    use cudarc::driver::{CudaContext, CudaSlice};
    let ctx = match HoldemEvalContext::new() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("skip GPU device bench (init failed): {e}");
            return;
        }
    };
    let cuda = CudaContext::new(0).expect("CudaContext");
    let stream = cuda.default_stream();

    let mut group = c.benchmark_group("holdem_gpu_device_7card");
    for &n in SIZES {
        let cards = fixtures(n, 7);
        let d_cards: CudaSlice<u8> = stream.clone_htod(&cards).expect("upload cards");
        let mut d_out: CudaSlice<u16> = unsafe { stream.alloc::<u16>(n) }.expect("alloc out");
        stream.synchronize().expect("sync");

        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| {
                ctx.evaluate_batch_on_stream(
                    &stream,
                    black_box(&d_cards),
                    black_box(&mut d_out),
                    n,
                    7,
                ).expect("kernel");
                stream.synchronize().expect("sync");
            });
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_cpu_7card,
    bench_gpu_host_7card,
    bench_gpu_device_7card,
);
criterion_main!(benches);
