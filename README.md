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
| `phe-omaha`          | Omaha high (4 hole + 5 board) — three-path dispatch over `phe-holdem` |
| `phe-omaha-fast`     | Omaha high — direct port of HenryRLee's PLO4 perfect-hash (multiset-hash + best-of-60 precomputed) |
| `phe-badugi`         | 4-card Badugi                                          |

`phe-omaha` and `phe-omaha-fast` are intentional siblings — pick by
caller need. See `crates/omaha-fast/BENCH_NOTES.md` for the full
side-by-side methodology.

### Optional CUDA backend (`cuda` feature)

`phe-holdem` and `phe-omaha-fast` both ship an NVRTC-compiled GPU
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
| Omaha high (`phe-omaha`) | 4 + 5 | `OmahaHighRule::evaluate` (single-call) | ~62 | ~16.1 |
| Omaha high (`phe-omaha`) | 4 + 5 | `OmahaHighRule::evaluate_batch` (path-1 prefetch) | ~54 | ~18.5 |
| Omaha high (`phe-omaha-fast`) | 4 + 5 | `evaluate_plo4_batch` (cold-cache 100K) | ~58 | ~17.2 |
| Omaha high (`phe-omaha-fast`) | 4 + 5 | naive 60-combo enum (reference) | ~146 | ~6.8 |

GPU throughput at varying batch size (NVIDIA + LLVM Rust same host,
`cuda` feature, 2026-05-01):

| Crate | N | CPU batch | GPU host (PCIe round-trip) | GPU device-resident |
|---|---:|---:|---:|---:|
| `phe-holdem` | 1 K | ~5 ns/h | 63 ns/h | 18 ns/h |
| `phe-holdem` | 100 K | 4.4 ns/h | 2.6 ns/h | **0.21 ns/h** |
| `phe-holdem` | 1 M | 5.2 ns/h | 1.95 ns/h | **0.062 ns/h** (~84× CPU) |
| `phe-omaha-fast` | 1 K | 27 ns/h | 82 ns/h | 22 ns/h |
| `phe-omaha-fast` | 100 K | 69 ns/h | 6.2 ns/h | **0.71 ns/h** (~100× CPU) |
| `phe-omaha-fast` | 1 M | 43 ns/h | 7.2 ns/h | **0.51 ns/h** (~80× CPU) |

GPU host (with PCIe upload/download) crosses CPU around N = 3–10 K.
GPU device-resident — i.e., the path a GPU-resident solver actually
takes — wins everywhere above N ≈ 1 K. See each crate's
`BENCH_NOTES.md` for the full reproduction recipe (Windows + NVRTC
DLL path, criterion harness).

### Reference numbers from other libraries

For order-of-magnitude context, two other open-source poker
evaluators publish their own benchmark numbers in their READMEs.
The numbers below are reproduced verbatim from those projects —
they were run on different machines, different languages, and
different harnesses, so the cross-row comparison is **not**
apples-to-apples and shouldn't be read as "X is faster than Y" in
any rigorous sense. They're useful as ballpark calibration only.

| Variant | [`Nerdmaster/poker`](https://github.com/Nerdmaster/poker) (Go) | [`HenryRLee/PokerHandEvaluator`](https://github.com/HenryRLee/PokerHandEvaluator) (C++) | `phe-*` (this repo, Rust) |
|---|---|---|---|
| 5-card | ~6.4 ns | ~13.8 ns | ~1.4 ns |
| 7-card | ~145 ns | ~17.8 ns | ~1.5 ns |
| Omaha 4-hole | ~416 ns | ~30.5 ns | ~62 ns |

Caveats / things this table does not capture:

- **Different machines.** `Nerdmaster/poker`'s numbers are from a
  16-core Go test runner, `HenryRLee/PokerHandEvaluator`'s from a
  2.6 GHz 12-core Xeon-class CPU, and the `phe-*` numbers above are
  from an Intel i9-12900H laptop boosting to ~5 GHz. Even normalising
  for clock, individual numbers can shift by 1.5–2× across machines.
- **Different scope.** `Nerdmaster/poker` is a complete poker library
  with `Card` / `Deck` / `Hand` / dealing / ranking utilities — not
  just an evaluator. `HenryRLee/PokerHandEvaluator` ships 5/6/7-card
  + PLO4/5/6 evaluators across multiple languages.  `phe-*` is just
  the inner-hot-path evaluator, designed to be embedded in a solver
  / equity calculator that supplies its own deal & deck logic.
- **Different design goals.**
  - `phe-holdem` follows b-inary's "one perfect-hash lookup" design,
    so 5/6/7-card hands cost about the same. `Nerdmaster/poker`
    enumerates `C(7, 5) = 21` 5-card sub-hands for 7-card; that's
    where most of its 7-card cost goes.
  - For Omaha, `HenryRLee/PokerHandEvaluator`'s PLO4 table is **30.5
    MB** vs `phe-omaha`'s **22 MB** — similar size, but their
    throughput is roughly 2× ours. The gap looks algorithmic (denser
    key encoding) rather than language-level, and is the most
    interesting honest finding from this comparison.

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
| `phe-omaha-assets` | `noflush_lookup` (path-1 9-card direct) | **22 MB** | **build.rs**, no committed blob |
| `phe-omaha-fast-assets` | `FLUSH_PLO4` (~8 MB) + `NOFLUSH_PLO4` (~22.5 MB) | **~30 MB** | **build.rs** + 28 KB primitive seed bins |
| `phe-omaha::lookup_5card` | `OFFSETS_5C` + `LOOKUP_5C` (5-card-only L1d-fitting) | ~33 KB | textual |

The 22 MB `phe-omaha-assets` table and the 30 MB `phe-omaha-fast-assets`
table pair are **generated at build time** by each crate's `build.rs`
from algorithmic primitives (Hold'em rank-only LOOKUP and a 28 KB pair
of 5-card Cactus-Kev seed tables, respectively). Keeping the
algorithm in `build.rs` means a single source of truth, and the repo
ships zero pre-baked > 1 MB blobs. Workspace-level
`[profile.*.build-override] opt-level = 3` keeps the generation cost
to ~2 s (`omaha-assets`) + ~20 s (`omaha-fast-assets`) on a fresh
clean build.

### How (Omaha)

`OmahaHighRule::evaluate` dispatches to one of three "9-card direct"
paths from the suit counts and the board's pair structure:

1. **No-flush path** (no suit has both ≥2 hole and ≥3 board cards):
   answer is a single read from a 22 MB rank-multiset table keyed by
   the multiset combinatorial number system over the 4 hole + 5 board
   ranks. No 60-combo enumeration.
2. **Flush-dominates path** (flush reachable AND board has 5 distinct
   ranks): a 10-window straight-flush bitmask scan + top-2 hole /
   top-3 board bit-OR resolves the answer with **one** `LOOKUP_FLUSH`
   access.
3. **Flush + paired board path**: SF / Quads / Full House / Flush are
   each computed independently from per-rank-count bitmasks; the max
   wins. Lower categories are dominated by the guaranteed flush.

`evaluate_batch` adds an `_mm_prefetch` hint four iterations ahead of
each path-1 lookup, hiding the 22 MB table's memory latency on x86_64.

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
  omaha-fast/           Omaha high — direct port of HenryRLee's PLO4 perfect-hash (+ optional `cuda` feature)
  omaha-fast-assets/    FLUSH_PLO4 + NOFLUSH_PLO4 (~30 MB, generated by build.rs)
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
