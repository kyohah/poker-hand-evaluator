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
| `phe-omaha`          | Omaha high (4 hole + 5 board, 2-from-hole + 3-from-board) |

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
| Omaha high | 4 + 5 | `OmahaHighRule::evaluate` (single-call) | ~62 | ~16.1 |
| Omaha high | 4 + 5 | `OmahaHighRule::evaluate_batch` (path-1 prefetch) | ~54 | ~18.5 |
| Omaha high | 4 + 5 | naive 60-combo enum (reference) | ~146 | ~6.8 |

### Comparison vs other libraries

For reference, the Go library
[`Nerdmaster/poker`](https://github.com/Nerdmaster/poker) publishes its
own `go test -bench` numbers (different machine, different language,
caveat lector):

| Variant | `Nerdmaster/poker` (Go) | `phe-*` (this repo, Rust) | Speed-up |
|---|---|---|---|
| 5-card | ~6.4 ns/eval (~150 M/s) | ~1.4 ns/eval (~705 M/s) | ~4.5× |
| 7-card | ~145 ns/eval (~6.5 M/s) | ~1.5 ns/eval (~666 M/s) | ~96× |
| Omaha (9-card) | ~416 ns/eval (~2.4 M/s) | ~62 ns/eval (~16.1 M/s) | ~6.7× |

The 7-card / Omaha gaps come from algorithmic differences, not just
language: `Nerdmaster/poker` enumerates `C(7, 5) = 21` 5-card sub-hands
for 7-card and `C(4, 2) × C(5, 3) = 60` for Omaha, whereas
`phe-holdem` does **one** perfect-hash table read for any 5/6/7-card
hand (b-inary's design) and `phe-omaha` dispatches to one of three
"9-card direct" paths for Omaha (see below).

### Memory footprint (lookup tables)

Most variants share the structure introduced by
[`b-inary/holdem-hand-evaluator`](https://github.com/b-inary/holdem-hand-evaluator)
(perfect-hashed `OFFSETS + LOOKUP` for the rank-only path,
`LOOKUP_FLUSH` for the flush path). Sizes are runtime, not
source-file size:

| Crate | Tables | Total runtime size |
|---|---|---|
| `phe-core` (shared) | `OFFSETS [i32; 12500]` | ~50 KB |
| `phe-holdem-assets` | `LOOKUP [u16; 73775]` + `LOOKUP_FLUSH [u16; 8129]` | ~163 KB |
| `phe-eight-low-assets` | `OFFSETS [i32; 12500]` + `LOOKUP [u16; 74285]` | ~199 KB |
| `phe-deuce-seven-assets` | `LOOKUP [u16; 73770]` + `LOOKUP_FLUSH [u16; 7937]` | ~163 KB |
| `phe-omaha-assets` | `noflush_lookup` (path-1 9-card direct) | **22 MB** |
| `phe-omaha::lookup_5card` | `OFFSETS_5C` + `LOOKUP_5C` (5-card-only L1d-fitting) | ~33 KB |

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
  holdem/               port of b-inary/holdem-hand-evaluator (MIT)
  holdem-assets/        precomputed lookup + offset tables
  eight-low/            ported from kyohah/8low-evaluator
  eight-low-assets/
  deuce-seven/          lookup tables generated with WheelMode::NoPair
  deuce-seven-assets/
  omaha/                Omaha high evaluator on top of phe-holdem
  omaha-assets/         path-1 no-flush direct lookup table (22 MB)
scripts/                asset generators (offset tables + lookup tables)
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
