# poker-hand-evaluator

A unified, single-thread, high-throughput poker hand evaluator covering
multiple variants behind a single `HandRule` trait. Designed for
embedding in solvers / equity calculators where evaluation cost is the
hot path.

## Variants

| Crate                | Rule                                                   |
|----------------------|--------------------------------------------------------|
| `phe-holdem`         | Hold'em high (5–7 cards)                               |
| `phe-eight-low`      | 8-or-better low + A-5 lowball (Razz)                   |
| `phe-deuce-seven`    | 2-7 lowball                                            |
| `phe-omaha`          | Omaha high (4 hole + 5 board) — port of HenryRLee's PLO4 perfect-hash (multiset-hash + best-of-60 precomputed), with optional CUDA backend |
| `phe-badugi`         | 4-card Badugi                                          |

### Optional CUDA backend (`cuda` feature)

`phe-holdem` and `phe-omaha` both ship an NVRTC-compiled GPU
evaluator behind the `cuda` feature. 1 thread = 1 hand kernel,
caller-shareable `Arc<CudaContext>` and caller-supplied `CudaStream`
so it composes into a larger CUDA app's existing graphs. Output
matches the CPU evaluator bit-exactly (verified by `cuda_parity`
tests). Designed for solver / equity-table / multiway showdown
workloads where evaluation runs in batches of 10 K–1 M+ hands with
data already on the GPU. See each crate's `BENCH_NOTES.md` for the
GPU vs CPU throughput table.

## Performance

Single-thread, 10 000 random fixtures per row, criterion mean over
100 samples. Fixture generation cost is excluded from the reported
time.

Machine: Intel Core i9-12900H (Alder Lake, 14C / 20T), Windows 11,
`rustc 1.95 stable`, `--release` profile, default `target-cpu`
(no `-march=native`).

### Throughput

| Variant | Hand size | API | ns/eval | M evals/sec |
|---|---|---|---|---|
| Hold'em high | 5 | `HighRule::evaluate` | ~1.4 | ~705 |
| Hold'em high | 6 | `HighRule::evaluate` | ~1.7 | ~605 |
| Hold'em high | 7 | `HighRule::evaluate` | ~1.5 | ~666 |
| 8-or-better low | 5 | `EightLowQualifiedRule::evaluate` | ~1.3 | ~756 |
| 8-or-better low | 7 | `EightLowQualifiedRule::evaluate` | ~1.4 | ~694 |
| A-5 lowball (Razz) | 5 | `AceFiveLowRule::evaluate` | ~1.0 | ~1020 |
| A-5 lowball (Razz) | 7 | `AceFiveLowRule::evaluate` | ~1.2 | ~806 |
| 2-7 lowball | 5 | `DeuceSevenLowRule::evaluate` | ~2.9 | ~344 |
| Omaha high | 4 + 5 | `OmahaHighRule::evaluate` (single-call, 100K cold-cache) | ~144 | ~6.9 |
| Omaha high | 4 + 5 | `evaluate_plo4_batch` (100K cold-cache, prefetch) | ~58 | ~17.2 |
| Omaha high | 4 + 5 | naive 60-combo enum (reference) | ~146 | ~6.8 |

GPU throughput at varying batch size (NVIDIA + LLVM Rust same host,
`cuda` feature, 2026-05-01):

| Crate | N | CPU batch | GPU host (PCIe round-trip) | GPU device-resident |
|---|---:|---:|---:|---:|
| `phe-holdem` | 1 K | ~5 ns/h | 63 ns/h | 18 ns/h |
| `phe-holdem` | 100 K | 4.4 ns/h | 2.6 ns/h | **0.21 ns/h** |
| `phe-holdem` | 1 M | 5.2 ns/h | 1.95 ns/h | **0.062 ns/h** (~84× CPU) |
| `phe-omaha` | 1 K | 27 ns/h | 82 ns/h | 22 ns/h |
| `phe-omaha` | 100 K | 69 ns/h | 6.2 ns/h | **0.71 ns/h** (~100× CPU) |
| `phe-omaha` | 1 M | 43 ns/h | 7.2 ns/h | **0.51 ns/h** (~80× CPU) |

GPU host (with PCIe upload/download) crosses CPU around N = 3–10 K.
GPU device-resident — i.e., the path a GPU-resident solver actually
takes — wins everywhere above N ≈ 1 K. See each crate's
`BENCH_NOTES.md` for the full reproduction recipe (Windows + NVRTC
DLL path, criterion harness).

### Reference numbers from other libraries

For Omaha, the upstream `HenryRLee/PokerHandEvaluator` C++ library
is the closest comparison. To make it apples-to-apples we re-built
the C reference on **the same host** (`clang-cl /O2 -flto
-fuse-ld=lld`) with the same fixture set as our criterion bench,
rather than quoting their published numbers from a different
machine. See `crates/omaha/BENCH_NOTES.md` for the
reproducibility recipe.

10 000 random PLO4 hands, deterministic xorshift64 seed
`0xDEAD_BEEF_CAFE_BABE`, this host:

| build                                  | speed (ns / eval) |
|----------------------------------------|-------------------|
| Rust `phe-omaha` (LLVM)           | ~35 ns (best 34.9) |
| C clang-cl `/O2 -flto -fuse-ld=lld`    | ~35 ns (best 34.7) |
| C MSVC `cl /O2 /GL /LTCG`              | ~46 ns            |
| C MSVC `cl /O2` (no LTO)               | ~52 – 62 ns       |
| HenryRLee published number, **their host** (gcc) | 30.5 ns |

When both are compiled with LLVM at `-O2 +LTO`, the Rust port and
the C reference are **at parity on this host** (within ~0.2 ns,
indistinguishable from noise). The 30.5 ns HenryRLee published is
on a different machine; we cannot reproduce that absolute number
here, but Rust matches what an LLVM-built C does. The gap to their
published figure is microarchitectural (CPU clock and L3 hit-rate
on a different machine), not algorithmic or language-level.

For the smaller variants the same picture holds at order-of-magnitude:
[`Nerdmaster/poker`](https://github.com/Nerdmaster/poker) (Go) does
~6.4 ns 5-card and ~145 ns 7-card on its own runner;
`HenryRLee/PokerHandEvaluator` publishes ~13.8 / ~17.8 ns; on this
host `phe-holdem` lands at ~1.4 / ~1.5 ns. Different machines,
languages, and harnesses, so the cross-library numbers are
ballpark only — but the same-host LLVM C-vs-Rust parity finding
above is the rigorous one.

Memory-footprint side-by-side (numbers from each project's own
README; for `phe-*` these are runtime u16/i32 array sizes):

| Variant | `HenryRLee/PokerHandEvaluator` table | `phe-*` table |
|---|---|---|
| 5-card lookup | 60 KB | 163 KB (covers 5/6/7 in one table) |
| 7-card lookup | 144 KB | 163 KB (same as 5-card) |
| Omaha (PLO4) lookup | 30.5 MB | 22 MB |

Also note that `HenryRLee/PokerHandEvaluator` ships PLO5 (5-hole) and
PLO6 (6-hole) Omaha variants which need 110 MB and 345 MB lookup
tables respectively — `phe-omaha` only handles standard 4-hole Omaha.

### Memory footprint (lookup tables)

Most variants share the structure introduced by
[`b-inary/holdem-hand-evaluator`](https://github.com/b-inary/holdem-hand-evaluator)
(perfect-hashed `OFFSETS + LOOKUP` for the rank-only path,
`LOOKUP_FLUSH` for the flush path). Sizes are runtime, not
source-file size:

| Crate | Tables | Total runtime size | Source-tree footprint |
|---|---|---|---|
| `phe-core` (shared) | `OFFSETS [i32; 12500]` | ~50 KB | textual |
| `phe-holdem-assets` | `LOOKUP [u16; 73775]` + `LOOKUP_FLUSH [u16; 8129]` | ~163 KB | textual |
| `phe-eight-low-assets` | `OFFSETS [i32; 12500]` + `LOOKUP [u16; 74285]` | ~199 KB | textual |
| `phe-deuce-seven-assets` | `LOOKUP [u16; 73770]` + `LOOKUP_FLUSH [u16; 7937]` | ~163 KB | textual |
| `phe-omaha-assets` | `FLUSH_PLO4` (~8 MB) + `NOFLUSH_PLO4` (~22.5 MB) | **~30 MB** | **build.rs** + 28 KB primitive seed bins |

The ~30 MB `phe-omaha-assets` table pair is **generated at build
time** by `build.rs` from algorithmic primitives (a 28 KB pair of
5-card Cactus-Kev seed tables, ported from HenryRLee's Python
distribution). Keeping the algorithm in `build.rs` means a single
source of truth, and the repo ships zero pre-baked > 1 MB blobs.
Workspace-level `[profile.*.build-override] opt-level = 3` keeps
the generation cost to ~20 s on a fresh clean build.

### How (Omaha)

`OmahaHighRule::evaluate` is a direct port of HenryRLee's PLO4
perfect-hash. For each (4 hole, 5 board) layout:

1. Compute per-suit counts. If any suit has ≥3 board and ≥2 hole
   cards, a flush sub-hand is reachable: hash the per-suit rank
   bitmaps via `hash_binary` and look up the precomputed best-of-60
   flush rank in `FLUSH_PLO4`.
2. Compute per-rank counts (quinary histogram). Hash the histogram
   via `hash_quinary` and look up the precomputed best-of-60
   non-flush rank in `NOFLUSH_PLO4`.
3. Take the min Cactus-Kev rank across the two paths (lower =
   stronger). The facade flips this to `7463 - rank` for the
   workspace's "higher = stronger" `u16` strength convention.

The 60-combo enumeration is **precomputed at build time**, so the
runtime call is dominated by ~one DRAM round-trip on
`NOFLUSH_PLO4` (the 22.5 MB table exceeds typical L3). The
`evaluate_plo4_batch` API hides that latency with
`_mm_prefetch` 8 hands ahead of each lookup, dropping cold-cache
batch throughput from ~144 ns/hand to ~58 ns/hand on x86_64.

Reproduce locally:

```sh
cargo bench -p phe-holdem
cargo bench -p phe-eight-low
cargo bench -p phe-deuce-seven
cargo bench -p phe-omaha
```

## Workspace layout

```
crates/
  core/                 Hand / Card / lookup-driven evaluator core
  holdem/               port of b-inary/holdem-hand-evaluator (MIT) (+ optional `cuda` feature)
  holdem-assets/        precomputed lookup + offset tables
  eight-low/            ported from kyohah/8low-evaluator
  eight-low-assets/
  deuce-seven/          lookup tables generated with WheelMode::NoPair
  deuce-seven-assets/
  omaha/                Omaha high evaluator on top of phe-holdem
  omaha-assets/         path-1 no-flush direct lookup table (22 MB, generated by build.rs)
  omaha/           Omaha high — direct port of HenryRLee's PLO4 perfect-hash (+ optional `cuda` feature)
  omaha-assets/    FLUSH_PLO4 + NOFLUSH_PLO4 (~30 MB, generated by build.rs)
  badugi/               4-card Badugi
scripts/                asset generators retained for one-shot debugging (production assets are built by each crate's own build.rs)
src/lib.rs              facade crate (`HandRule` + feature-gated re-exports)
```

## Acknowledgements

The Hold'em core (the `Hand` type, the perfect-hash design, the
table-generation pipeline) is a Rust port of
[`b-inary/holdem-hand-evaluator`](https://github.com/b-inary/holdem-hand-evaluator)
(MIT). The Omaha optimisations (9-card direct dispatch + path-1
multiset lookup + path-2 SF window scan + path-3 bitmask category
detection + batch prefetch) are added on top of that core.

The Cactus-Kev / Senzee 5-card kernel used as a cross-check evaluator
(`evaluate_kev`) is verbatim from `b-inary/holdem-hand-evaluator`'s
`scripts/src/kev/arrays.rs`, which in turn ports the original
Cactus-Kev / Paul Senzee tables.

## License

MIT. See `LICENSE` for the combined notice covering the parts derived
from `b-inary/holdem-hand-evaluator`.
