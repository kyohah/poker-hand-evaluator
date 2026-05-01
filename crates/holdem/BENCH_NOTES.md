# `phe-holdem` performance notes

## CPU baseline

The 5-7 card Hold'em evaluator uses the `phe-core` perfect-hash:
`key`/`mask` accumulate 52 (u64, u64) per-card constants, the flush
bit in `key` selects between `LOOKUP_FLUSH` and the
`OFFSETS`-indirected `LOOKUP`. The combined table footprint is:

| table          | size       | cumulative  |
|----------------|------------|-------------|
| `LOOKUP`       | 73 775 u16 | 145 KB      |
| `LOOKUP_FLUSH` | 8 129 u16  | 16 KB       |
| `OFFSETS`      | 12 500 i32 | 50 KB       |
| `CARDS`        | 52 × 16 B  | 832 B       |
| **total**      |            | **~212 KB** |

That fits comfortably in modern L2 (typically 256 KB–1 MB), so
random-hand throughput stays in the L2 regime even at 100K+
fixtures — quite different from `phe-omaha`'s 22 MB
`NOFLUSH_PLO4` which exceeds L3.

## CUDA backend (`cuda` feature, 2026-05-01)

Optional GPU evaluator gated by the `cuda` feature. 1 thread = 1
hand kernel. The 5 lookup tables are uploaded to device global
memory once on `HoldemEvalContext::new()`; with a working set of
~212 KB they stay L2-resident on the GPU through normal access
patterns (T4: 4 MB L2; A100: 40 MB L2; RTX 3060: 3 MB L2).

API surface mirrors `phe-omaha::cuda` intentionally:

* `evaluate_batch(cards, cards_per_hand)` — host slice in, host
  Vec out, includes upload + download every call.
* `evaluate_batch_device(...)` — caller-owned device buffers,
  default stream, no PCIe.
* `evaluate_batch_on_stream(stream, ...)` — caller-supplied stream,
  capturable into a CUDA graph. Use this from a solver that owns
  its own stream / graph / context.

Output is u16 with the workspace's "higher = stronger" convention
(LOOKUP / LOOKUP_FLUSH already store values that way) — matches
`HandRule::evaluate`, no Cactus-Kev inversion needed.

### Throughput at varying batch size (7-card)

NVIDIA GPU + LLVM Rust CPU on the same host, criterion `--quick`,
2026-05-01:

| N (hands)  | CPU 7-card    | GPU host (with PCIe) | GPU device-resident   |
|-----------:|--------------:|---------------------:|----------------------:|
|      1 000 |       ~5 ns/h |             63 ns/h  |              18 ns/h  |
|     10 000 |       ~5 ns/h |              8 ns/h  |          **1.77 ns/h** |
|    100 000 |    **4.4 ns/h** |          2.6 ns/h  |         **0.21 ns/h** |
|  1 000 000 |       5.2 ns/h |          1.95 ns/h  |        **0.062 ns/h** |

CPU stays L2-hot from N=100 to N=1M (the table is 212 KB ≤ L2).
GPU device-resident hits ~16 Gelem/s at 1 M hands — about **84×**
the CPU. GPU host (with PCIe) crosses CPU around N = 5-10 K.

### Solver integration recipe

`poker-cuda-solver` is the primary GPU-resident caller. Same
pattern as `phe-omaha::cuda`:

```rust
let ctx: Arc<CudaContext> = solver_context();
let holdem = HoldemEvalContext::from_context(ctx.clone())?;

// In CFR backward pass / equity computation:
holdem.evaluate_batch_on_stream(
    &solver_stream,
    &d_cards,         // n * cards_per_hand bytes on device
    &mut d_strengths, // n * u16 on device
    n,
    7,                // cards_per_hand
)?;
```

Note: `poker-cuda-solver`'s `gpu_equity.rs` previously inlined
this same `evaluate_hand` device function. Migrating to
`HoldemEvalContext` keeps a single source of truth in this crate
and lets the solver re-use the upload + compile state across all
CFR-with-Hold'em-rules paths instead of compiling the kernel per
equity-table build.

### Reproducing

NVRTC must be on `PATH`. On Windows with CUDA Toolkit 13.2:

```sh
PATH="/c/Program Files/NVIDIA GPU Computing Toolkit/CUDA/v13.2/bin/x64:$PATH" \
  cargo bench -p phe-holdem --bench eval_cuda --features cuda
```

Parity (GPU u16 strength bit-exact == CPU `HighRule::evaluate`)
across 5/6/7-card hands and uneven grid sizes:

```sh
PATH="/c/Program Files/NVIDIA GPU Computing Toolkit/CUDA/v13.2/bin/x64:$PATH" \
  cargo test -p phe-holdem --release --features cuda -- --ignored
```
