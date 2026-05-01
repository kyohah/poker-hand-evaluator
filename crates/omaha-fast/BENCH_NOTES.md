# `phe-omaha-fast` performance notes

Direct comparison against the upstream HenryRLee/PokerHandEvaluator C
reference, run on the **same Windows host** (no cross-host noise).

## Numbers

10 000 random PLO4 hands, deterministic xorshift64 seed
`0xDEAD_BEEF_CAFE_BABE`. Measured 2026-05-01.

| build                                  | speed (ns / eval) |
|----------------------------------------|-------------------|
| **Rust `phe-omaha-fast` (LLVM)**       | **~35 ns** (best 34.9) |
| **C clang-cl `/O2 -flto -fuse-ld=lld`**| **~35 ns** (best 34.7) |
| C MSVC `cl /O2 /GL /LTCG`              | ~46 ns            |
| C MSVC `cl /O2` (no LTO)               | ~52 – 62 ns       |
| HenryRLee published (their host, gcc)  | 30.5 ns           |

When both are compiled with LLVM at `-O2 +LTO`, the Rust port and the
C reference are **at parity on this host** (within ~0.2 ns,
indistinguishable from noise). The 30.5 ns HenryRLee published was on
a different machine; we cannot reproduce that absolute number here,
but Rust matches what an LLVM-built C does.

## Why MSVC is slower

Cross-translation-unit inlining: with `cl /O2` alone, MSVC compiles
each `.c` file separately and the linker doesn't inline across
boundaries, so every `evaluate_plo4_cards` call is a real function
call. `/GL /LTCG` enables whole-program optimisation, which recovers
~6 ns but still trails LLVM by ~10 ns on this kernel.

## How to reproduce

### Rust

```sh
cd ~/ghq/github.com/kyohah/poker-hand-evaluator
cargo bench -p phe-omaha-fast --bench eval -- --quick
```

### C (clang-cl + LTO)

```sh
# Install LLVM if needed:
scoop install llvm        # or: winget install LLVM.LLVM

# In ~/ghq/github.com/HenryRLee/PokerHandEvaluator:
# (1) Save bench_plo4.c (snippet below)
# (2) From an x64 Native Tools Command Prompt:

mkdir obj
"C:\Users\kyoha\scoop\apps\llvm\current\bin\clang-cl.exe" \
    /O2 -flto -fuse-ld=lld /nologo /I cpp\include \
    bench_plo4.c \
    cpp\src\evaluator_plo4.c cpp\src\dptables.c cpp\src\tables_bitwise.c \
    cpp\src\tables_plo4.c cpp\src\hash.c cpp\src\hashtable.c \
    cpp\src\rank.c cpp\src\7462.c \
    /Fe:bench_clang.exe /Fo:obj\

bench_clang.exe   # prints `PLO4 C: XX.XX ns/eval ...`
```

### C (MSVC LTCG, for comparison)

```sh
mkdir obj
cl /O2 /GL /nologo /I cpp\include bench_plo4.c \
   cpp\src\evaluator_plo4.c cpp\src\dptables.c cpp\src\tables_bitwise.c \
   cpp\src\tables_plo4.c cpp\src\hash.c cpp\src\hashtable.c \
   cpp\src\rank.c cpp\src\7462.c \
   /link /LTCG /OUT:bench_msvc.exe
```

```c
// bench_plo4.c — same fixture-generation as our criterion bench
// so the two numbers are directly comparable.
#include <stdio.h>
#include <stdint.h>
#include <windows.h>
#include "phevaluator/phevaluator.h"

static uint64_t xs(uint64_t* s) {
    uint64_t x = *s;
    x ^= x << 13; x ^= x >> 7; x ^= x << 17;
    *s = x;
    return x;
}

#define NF 10000
static int holes[NF][4], boards[NF][5];

int main(void) {
    uint64_t state = 0xDEADBEEFCAFEBABEULL;
    for (int i = 0; i < NF; ++i) {
        int deck[52];
        for (int j = 0; j < 52; ++j) deck[j] = j;
        for (int j = 0; j < 9; ++j) {
            int p = j + (int)(xs(&state) % (uint64_t)(52 - j));
            int t = deck[j]; deck[j] = deck[p]; deck[p] = t;
        }
        for (int j = 0; j < 4; ++j) holes[i][j] = deck[j];
        for (int j = 0; j < 5; ++j) boards[i][j] = deck[4 + j];
    }
    LARGE_INTEGER freq, t0, t1;
    QueryPerformanceFrequency(&freq);
    int sink = 0;
    for (int it = 0; it < 10; ++it)        // warmup
        for (int i = 0; i < NF; ++i)
            sink ^= evaluate_plo4_cards(
                boards[i][0], boards[i][1], boards[i][2], boards[i][3], boards[i][4],
                holes[i][0], holes[i][1], holes[i][2], holes[i][3]);
    int N = 1000;
    QueryPerformanceCounter(&t0);
    for (int it = 0; it < N; ++it)
        for (int i = 0; i < NF; ++i)
            sink ^= evaluate_plo4_cards(
                boards[i][0], boards[i][1], boards[i][2], boards[i][3], boards[i][4],
                holes[i][0], holes[i][1], holes[i][2], holes[i][3]);
    QueryPerformanceCounter(&t1);
    double sec = (double)(t1.QuadPart - t0.QuadPart) / (double)freq.QuadPart;
    printf("PLO4 C: %.2f ns/eval (%lld evals, sink=%d, %.3fs)\n",
           sec * 1e9 / ((double)N * (double)NF),
           (long long)N * (long long)NF, sink, sec);
    return 0;
}
```

## Implication

`phe-omaha-fast` is at parity with `clang-cl`-built C on this host.
The "13× slower than HenryRLee" worry that motivated this port was
eliminated entirely once we measured on the same host with the same
LLVM backend — both implementations land at ~35 ns / eval, and the
gap to HenryRLee's published 30.5 ns is microarchitectural (CPU clock
and L3 hit-rate on a different machine), not algorithmic or
language-level.

The next meaningful improvement is a cache-friendlier table layout
(phase 2 in the omaha-fast roadmap), not micro-optimisation of the
existing kernel — both LLVM C and LLVM Rust have already squeezed
this algorithm dry.

## CUDA backend (`cuda` feature, 2026-05-01)

Optional GPU backend. 1 thread = 1 hand kernel; FLUSH/NOFLUSH/DP
tables uploaded to device once on `PloEvalContext::new()`. Two entry
points:

* `evaluate_batch(&[(hole, board)])` — host slice in, host Vec out.
  Includes upload + kernel + download every call.
* `evaluate_batch_device(d_holes, d_boards, d_out, n)` — caller
  already owns device buffers. Zero PCIe per call. This is the path
  a GPU-resident solver should use.

Both share the same kernel. NVRTC compilation + ~30 MB table upload
costs ~1 s the first time, amortised across the process.

### Throughput at varying batch size

NVIDIA GPU + LLVM Rust CPU on the same host, criterion `--quick`,
2026-05-01:

| N (hands)  | CPU batch    | GPU host (with PCIe) | GPU device-resident |
|-----------:|-------------:|---------------------:|--------------------:|
|      1 000 |  **27 ns/h** |             82 ns/h  |            22 ns/h  |
|     10 000 |     38 ns/h  |          13.3 ns/h   |        **2.0 ns/h** |
|    100 000 |     69 ns/h  |          6.2 ns/h    |       **0.71 ns/h** |
|  1 000 000 |     43 ns/h  |          7.2 ns/h    |       **0.51 ns/h** |

(CPU 1K is fast because the access set fits in L3; CPU 100K is the
cold-cache regime that motivates the prefetch design.)

Crossover points:

* **GPU host vs CPU**: ~3-5 K hands. Below that, kernel launch +
  upload overhead exceeds the kernel time. Above that, GPU wins
  6-12× even with PCIe round-trip.
* **GPU device vs CPU**: GPU wins everywhere above N = 1 K, but the
  big asymmetry shows up at scale: **~100× at 100 K**, ~80× at 1 M.

When the user's solver already has hands and boards on the GPU
(matrix-form CFR with terminal evaluation in-kernel), the
`_device` path is essentially free: 0.5 ns/hand asymptotic, no PCIe.
This is the case PLO4 / Stud high / multiway equity actually live
in — the pure single-eval CUDA worry ("kernel launch overhead is
slower than CPU") doesn't apply once you batch.

### Solver integration recipe

`poker-cuda-solver` and similar GPU-resident callers should:

1. Share the `Arc<CudaContext>` instead of letting `PloEvalContext`
   spin up its own:
   ```rust
   let ctx: Arc<CudaContext> = solver_context();
   let plo_eval = PloEvalContext::from_context(ctx.clone())?;
   ```
2. Launch on the solver's per-pass stream so the eval kernel orders
   correctly with the surrounding CFR forward / backward / showdown
   kernels (and so it can be captured into the same CUDA graph):
   ```rust
   plo_eval.evaluate_batch_on_stream(
       &solver_stream,
       &d_holes,
       &d_boards,
       &mut d_ranks_i32,
       n,
   )?;
   ```
3. The kernel writes **i32 Cactus-Kev rank** (`[1, 7462]`, lower =
   stronger). The workspace's `HandRule::evaluate` convention is
   "u16 higher = stronger", so the showdown kernel that consumes
   strength values needs `strength = 7463 - rank`. Two options:
   * Run a one-line kernel
     `out_u16[i] = (unsigned short)(7463 - in_i32[i])` after
     `evaluate_batch_on_stream`. Trivially fused with the next
     showdown kernel if you write a custom backward pass.
   * If you already have `fill_showdown_cfv_*` kernels, pass the
     `i32 rank` array and apply `7463 - rank` inside the strength
     comparison (no extra memory traffic).

The kernel uses zero shared memory and `block_dim = 256`, so it's
easy to schedule alongside the solver's heavier kernels (which on
T4 want 2 blocks/SM with `block_dim = 512`). No occupancy conflict.

### Reproducing

NVRTC must be on `PATH`. On Windows with CUDA Toolkit 13.2:
```sh
PATH="/c/Program Files/NVIDIA GPU Computing Toolkit/CUDA/v13.2/bin/x64:$PATH" \
  cargo bench -p phe-omaha-fast --bench eval_cuda --features cuda
```

Parity (GPU output bit-exact == CPU `evaluate_plo4_cards`) is
guarded by `tests/cuda_parity.rs`:
```sh
PATH="/c/Program Files/NVIDIA GPU Computing Toolkit/CUDA/v13.2/bin/x64:$PATH" \
  cargo test -p phe-omaha-fast --release --features cuda -- --ignored
```

## Negative results recorded here so we don't retry them

### AVX2 8-wide pass-1 (`hash_quinary` SIMD across hands, 2026-05-01)

Tried vectorising `noflush_index` across 8 hands at a time using
`vpgatherdd` over the small 2.8 KB `DP[5][14][10]` table. The kernel
itself emits 26 `vpgatherdd` instructions and runs the full 13-iter
branchless form (lanes diverge so early-exit is impossible).

Bench numbers on 100K cold-cache fixtures:

| pass-1 form                                 | ns / hand |
|---------------------------------------------|----------:|
| `hash_quinary_branchless` (forced 13-iter)  | 45.5      |
| `hash_quinary` (early-exit)                 | **38.7**  |
| AVX2 8-wide gather (forced 13-iter)         | 39.1      |

AVX2 ties the scalar early-exit but doesn't beat it. Two reasons
compound:

1. The early-exit form averages ~11 of 13 iterations on random hands
   (bails out as soon as `k` hits 0), whereas the SIMD form runs all
   13 because lanes diverge. That's a built-in 13/11 ≈ 1.18× advantage
   for scalar that no amount of in-lane parallelism cancels.
2. The AVX2 path has to build SoA quinary histograms (8 hands ×
   9 cards = 72 scattered byte writes into a `[u8; 8 × 13]` buffer)
   before it can `vpgatherdd`. That scatter alone eats most of the
   8-way kernel speedup on Skylake-class hosts.

If revisiting on AVX-512 (where scatter is native and gather is
cheaper), or on a uarch with very expensive branch mispredicts, the
calculus may flip — but on this host the simpler scalar wins. Picking
early-exit `hash_quinary` for `noflush_index_scalar` (replacing the
batch-oriented branchless variant) is the actual win that came out
of this exercise.

### Packed bitmap for suit counting (batch path, 2026-05-01)

Tried replacing the 9 scattered `suit_count_*[c & 3] += 1` /
`suit_binary_*[c & 3] |= BIT_OF_DIV_4[c]` writes with a single OR of
9 `1u64 << (16 * suit + rank)` values — same idea as
`phe-core::Hand` for 7-card hold'em. Per suit, the count is
`popcount` of the 16-bit window and the rank-bitmap falls out for
free as the same window's low 13 bits.

Even with tree-reduced ORs, this regressed batch by ~5 ns / hand
(3.15 → 4.20 ms / 100K). Why: the OR chain has a 5-cycle dependency
chain (5 cards) that the CPU can't break, while the scattered store
form lets store-buffer parallelism issue 9 stores out-of-order. The
wider issue width wins on Skylake-class hosts.

If revisiting on a microarchitecture with narrower stores or wider
ORs, re-bench from the original `evaluate_with_noflush_idx` form
before switching.
