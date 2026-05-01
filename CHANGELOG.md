# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

While the crate is at `0.x`, breaking changes can land in any release;
once `1.0` ships, breaking changes will only happen in major-version
bumps.

## [Unreleased]

### Added
- `evaluate_plo4_cards_arr(board: [u8; 5], hole: [u8; 4]) -> i32` — array-arg
  ergonomic wrapper for the 9-positional-i32 `evaluate_plo4_cards`.
- `evaluate_plo4_batch_into(hands, out, scratch: &mut Vec<usize>)` — caller-owned
  scratch buffer variant for solver workloads that batch-evaluate millions
  of hands in a tight loop. `evaluate_plo4_batch` keeps its existing
  allocating behaviour.
- Property-based contract checks for `HandRule` (multiset / order
  invariance, `Ord` totality, lookup-range bounds) under
  `tests/proptest.rs`.
- CI matrix expansion: feature-subset combos, MSRV pin (Rust 1.78),
  rustdoc warning gate, CUDA `cargo build --features cuda`
  compile-check.
- `Default + Clone + Copy + Debug` derives on every unit-struct rule
  (`HighRule`, `OmahaHighRule`, `DeuceSevenLowRule`, `BadugiRule`,
  `EightLowQualifiedRule`, `AceFiveLowRule`, `HiLoRule`).

### Changed
- Workspace dual-licensed `MIT OR Apache-2.0` to comply with the
  upstream `HenryRLee/PokerHandEvaluator` Apache-2.0 derivation
  (PLO4 evaluator and CUDA kernel string). `LICENSE-APACHE` and
  `NOTICE` added at the workspace root.
- Every `phe-*` crate's `Cargo.toml` now carries the publish metadata
  needed for crates.io (`categories`, `keywords`, `homepage`,
  `rust-version = "1.78"`); path deps gained `version = "0.1.0"` so
  inter-crate publishing resolves.
- `phe-omaha-assets` exposes only the `pub static FLUSH_PLO4 /
  NOFLUSH_PLO4 / NO_FLUSH_5 / FLUSH_5` form. The duplicate
  `pub fn flush_plo4()` / `noflush_plo4()` accessors have been
  removed (no in-tree caller used them).

### Performance
- **PLO4 single-call: -19.8%** (29.8 -> 23.9 ns/hand on Apple Silicon
  M-series, 100K cold-cache fixtures).
- **PLO4 batch:        -20.8%** (30.3 -> 24.0 ns/hand, same hardware).
- Driven by the suit-counter packed-`u16` bit-trick that collapses
  the `scb >= 3 && sch >= 2` flush-reachable check to a single AND.
- See `crates/omaha/BENCH_NOTES.md` ("Suit padding bit-trick" section)
  for the full bench numbers and the `wide::u8x16` SIMD-quinary
  experiment that was tried in the same chunk and reverted as a
  negative result.

## [0.1.0] - 2026-05-01

Initial release. Workspace structure (`phe-core` + 5 variant
evaluators + matching `*-assets` data crates + facade crate
`poker-hand-evaluator`). Variants:

- Hold'em high (5/6/7-card), ported from
  `b-inary/holdem-hand-evaluator` (MIT).
- 8-or-better and A-5 lowball (5/6/7-card).
- 2-7 lowball (5-card only).
- Omaha PLO4 (4 hole + 5 board), ported from
  `HenryRLee/PokerHandEvaluator` (Apache-2.0). Same-host parity with
  clang-cl + LTO C reference at ~35 ns / eval.
- Badugi (4-card).

Optional `cuda` feature on `phe-holdem` and `phe-omaha` ships an
NVRTC-compiled GPU evaluator (1 thread = 1 hand) with the same API
shape on both crates: `from_context(Arc<CudaContext>)` for
solver-shared init and `evaluate_batch_on_stream(&CudaStream, …)`
for graph-capturable launch.
