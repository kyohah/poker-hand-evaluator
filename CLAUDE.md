# Project rules

Rules specific to this workspace. Read this before making changes.

## No parallelism in `phe-*` crates

Do **not** add `rayon`, `std::thread`, `std::sync::atomic` parallel
loops, or SIMD-based multi-threading inside any of the `phe-*` crates
(`phe-core`, `phe-holdem`, `phe-eight-low`, `phe-deuce-seven`,
`phe-omaha`, `phe-badugi`, the facade, or any future variant crate).

**Reason**: the downstream consumer
(`~/ghq/github.com/kyohah/poker-cuda-solver`) already parallelises at
the **solver** level via `rayon` — every CFR iteration spawns the
parallel work. Adding parallelism inside the eval would either
duplicate the work or thrash the CPU caches by sub-dividing already
hot loops.

In particular: when an "obvious" speedup involves parallel iteration,
prefer **single-thread cache-friendly** improvements (smaller tables,
tighter inner loops, branch-and-bound prune) over thread spawning.
The eval is **memory-bound on shared lookup tables** (e.g. the 145 KB
`phe-holdem-assets::LOOKUP`), so adding threads typically loses to
cache contention.

This rule applies even to benches and examples in the eval crates —
keep them single-threaded so `bench/history.csv` numbers stay
comparable across runs.

## Bench history convention

`bench/history.csv` is the canonical record of single-thread Omaha
eval throughput across optimisation commits. The schema is:

```
unix_ts,commit,bench,count,elapsed_s,mevals_per_s,ns_per_eval
```

`cargo run --release -p phe-omaha --example exhaustive` auto-appends
one row per run (10 s wall-clock random-stream throughput). Keep this
working — it's the regression gate when an "optimisation" actually
slows things down (Cactus-Kev kernel switch was caught this way; see
`docs/omaha-perf-investigation.md`).
