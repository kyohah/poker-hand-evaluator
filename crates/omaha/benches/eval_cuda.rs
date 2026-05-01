//! CPU vs GPU PLO4 throughput at varying batch sizes.
//!
//! Run with:
//! ```text
//! cargo bench -p phe-omaha --bench eval_cuda --features cuda
//! ```
//! NVRTC must be on PATH (Windows: add CUDA toolkit's `bin/x64`).
//!
//! Three measurements per batch size:
//! * `cpu_batch_*` — `evaluate_plo4_batch` (single-thread, prefetch).
//! * `gpu_batch_*` — `PloEvalContext::evaluate_batch` (host slice in,
//!   host Vec out — includes upload + kernel + download).
//! * `gpu_device_*` — `PloEvalContext::evaluate_batch_device` with
//!   pre-uploaded device buffers (no PCIe per call). This is the
//!   number a GPU-resident solver would actually see.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use phe_omaha::cuda::PloEvalContext;
use phe_omaha::evaluate_plo4_batch;

const SEED: u64 = 0xDEAD_BEEF_CAFE_BABE;
const SIZES: &[usize] = &[1_000, 10_000, 100_000, 1_000_000];

struct Rng(u64);
impl Rng {
    fn new(s: u64) -> Self {
        Self(s)
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

fn fixtures(n: usize) -> Vec<([u8; 4], [u8; 5])> {
    let mut rng = Rng::new(SEED);
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        let mut deck: [u8; 52] = [0; 52];
        for (i, slot) in deck.iter_mut().enumerate() {
            *slot = i as u8;
        }
        for i in 0..9 {
            let p = i + rng.pick(52 - i);
            deck.swap(i, p);
        }
        out.push((
            [deck[0], deck[1], deck[2], deck[3]],
            [deck[4], deck[5], deck[6], deck[7], deck[8]],
        ));
    }
    out
}

fn bench_cpu(c: &mut Criterion) {
    let mut group = c.benchmark_group("plo4_cpu_batch");
    for &n in SIZES {
        let f = fixtures(n);
        let mut out = vec![0i32; n];
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| {
                evaluate_plo4_batch(black_box(&f), black_box(&mut out));
                black_box(&out);
            });
        });
    }
    group.finish();
}

fn bench_gpu_host(c: &mut Criterion) {
    let ctx = match PloEvalContext::new() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("skip GPU bench (init failed): {e}");
            return;
        }
    };
    let mut group = c.benchmark_group("plo4_gpu_batch_host");
    for &n in SIZES {
        let f = fixtures(n);
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| {
                let r = ctx.evaluate_batch(black_box(&f)).expect("kernel");
                black_box(r);
            });
        });
    }
    group.finish();
}

fn bench_gpu_device(c: &mut Criterion) {
    use cudarc::driver::{CudaContext, CudaSlice};
    let ctx = match PloEvalContext::new() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("skip GPU device bench (init failed): {e}");
            return;
        }
    };
    // Need a stream to upload fixture buffers. Reuse default stream
    // from a fresh CudaContext (same device 0 → same context cached
    // by the driver).
    let cuda = CudaContext::new(0).expect("CudaContext");
    let stream = cuda.default_stream();

    let mut group = c.benchmark_group("plo4_gpu_batch_device");
    for &n in SIZES {
        let f = fixtures(n);
        let mut holes_flat = Vec::with_capacity(n * 4);
        let mut boards_flat = Vec::with_capacity(n * 5);
        for (h, b) in &f {
            holes_flat.extend_from_slice(h);
            boards_flat.extend_from_slice(b);
        }
        let d_holes: CudaSlice<u8> = stream.clone_htod(&holes_flat).expect("upload holes");
        let d_boards: CudaSlice<u8> = stream.clone_htod(&boards_flat).expect("upload boards");
        let mut d_out: CudaSlice<i32> = unsafe { stream.alloc::<i32>(n) }.expect("alloc out");
        stream.synchronize().expect("sync");

        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| {
                ctx.evaluate_batch_device(
                    black_box(&d_holes),
                    black_box(&d_boards),
                    black_box(&mut d_out),
                    n,
                )
                .expect("kernel");
                stream.synchronize().expect("sync");
            });
        });
    }
    group.finish();
}

criterion_group!(benches, bench_cpu, bench_gpu_host, bench_gpu_device);
criterion_main!(benches);
