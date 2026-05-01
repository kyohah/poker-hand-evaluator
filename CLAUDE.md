# Project rules

Rules specific to this workspace. Read this before making changes.

## Crate map

```
crates/
├── core                Hand struct, perfect-hash bases, lookup primitives
├── holdem              5-7 card high (port of b-inary/holdem-hand-evaluator)
├── holdem-assets       LOOKUP / LOOKUP_FLUSH / OFFSETS for phe-holdem
├── eight-low           8-or-better low (5-card) + Razz A-5 low rule wrapper
├── eight-low-assets    eight-low perfect-hash tables
├── deuce-seven         2-7 lowball (5-card)
├── deuce-seven-assets  deuce-seven perfect-hash tables
├── omaha               4-hole high — three-path dispatch over phe-holdem
├── omaha-assets        path-1 NOFLUSH_LOOKUP (22 MB) for phe-omaha
├── omaha-fast          4-hole high — direct port of HenryRLee's PLO4 perfect-hash
├── omaha-fast-assets   FLUSH_PLO4 + NOFLUSH_PLO4 (~30 MB) for phe-omaha-fast
└── badugi              4-card badugi
```

The facade crate (workspace root, `poker-hand-evaluator`) re-exports
the variant rules behind `#[cfg(feature = ...)]` gates and exposes the
shared `HandRule` trait. Default features are `["all"]`.

## Two omaha crates

`phe-omaha` and `phe-omaha-fast` are intentional siblings — they are
**not** alternatives where one is "the new" implementation. Pick by
caller need:

| crate            | speed (cold-cache 100K, this host) | binary size | algorithm |
|------------------|-----------------------------------:|-------------|-----------|
| `phe-omaha`      | ~60 ns / hand                      | ~22 MB      | path1 noflush table + path2/3 dispatch via phe-holdem |
| `phe-omaha-fast` | ~33-35 ns batch / ~37 ns single    | ~30 MB      | HenryRLee perfect-hash port (multiset-hash + best-of-60 precomputed) |

`phe-omaha-fast` adds an `evaluate_plo4_batch` API that uses software
`_mm_prefetch` to hide DRAM latency. On 100K cold-cache fixtures this
is ~2.2× faster than the single-hand loop and matches what same-host
`clang-cl /O2 -flto` gets on the C reference. See
`crates/omaha-fast/BENCH_NOTES.md` for the full methodology.

## No parallelism in `phe-*` crates

Do **not** add `rayon`, `std::thread`, `std::sync::atomic` parallel
loops, or thread-based fan-out inside any of the `phe-*` crates
(`phe-core`, `phe-holdem`, `phe-eight-low`, `phe-deuce-seven`,
`phe-omaha`, `phe-omaha-fast`, `phe-badugi`, the facade, or any future
variant crate).

**Reason**: this crate is designed to be embedded in a
solver / equity calculator that already parallelises at the
**outer** level (e.g., per-iteration in CFR, or per-hand in equity
sweeps). Adding parallelism inside the eval would either duplicate
the outer work or thrash the CPU caches by sub-dividing already hot
loops.

When an "obvious" speedup involves spawning threads, prefer
**single-thread cache-friendly** improvements (smaller tables,
tighter inner loops, branch-and-bound prune, software prefetch in
batch APIs) instead. The eval is **memory-bound on shared lookup
tables** (e.g. `phe-omaha-fast::NOFLUSH_PLO4` is 22 MB), so adding
threads typically loses to cache contention.

**Single-thread SIMD intrinsics are fine** — `_mm_prefetch`, AVX2
gather, packed integer ops, etc. The rule is about thread-level
concurrency, not vectorisation.

This rule applies to benches and examples in the eval crates too,
so `bench/history.csv` numbers stay comparable across runs.

## Bench convention

`crates/omaha/bench/history.csv` is the rolling record of
single-thread Omaha eval throughput. Schema:

```
unix_ts,commit,bench,count,elapsed_s,mevals_per_s,ns_per_eval
```

`cargo run --release -p phe-omaha --example exhaustive` auto-appends
one row per run (10 s wall-clock random-stream throughput). Keep that
example working — it's the regression gate when an "optimisation"
actually slows things down (the Cactus-Kev kernel switch was caught
this way).

`phe-omaha-fast` has its own criterion bench at
`crates/omaha-fast/benches/eval.rs` (single, batch, pass1-only).
Use cold-cache 100K fixtures for batch numbers; a 10K micro-bench
gives misleadingly fast results because the access set fits in L3.
The `BENCH_NOTES.md` reproducibility recipe also covers a same-host
C reference build (clang-cl + LTO), useful when comparing absolute
ns/eval numbers against published HenryRLee figures.

## Algorithm parity

When porting from a reference implementation (HenryRLee, Cactus-Kev,
b-inary, etc.), prove **bit-exact correctness** before pursuing
performance:

* The omaha-fast Royal SF spot-check (rank == 1) catches encoding /
  table-indexing bugs.
* The 1000-hand pairwise ordering parity test against the existing
  `phe-omaha` (`crates/omaha-fast/tests/parity_with_phe_omaha.rs`)
  catches algorithm-level divergence even when absolute u16 values
  differ between conventions.
* When adding a new optimisation (branchless variant, SIMD path,
  etc.), assert it produces the same output as the existing form on
  random fixtures. See `batch::tests::batch_matches_single` as a
  template.
