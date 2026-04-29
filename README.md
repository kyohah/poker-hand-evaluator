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

## Performance (Omaha high)

Single-thread, 10 000 random `(hole, board)` fixtures, AMD Ryzen 7
3700X-class machine.

| API                                       | ns/eval | M evals/sec |
|-------------------------------------------|---------|-------------|
| `OmahaHighRule::evaluate` (single-call)   | ~65     | ~15.4       |
| `OmahaHighRule::evaluate_batch` (path-1 prefetch) | ~54  | ~18.5       |
| Naive 60-combo enumeration (reference)    | ~600    | ~1.7        |

For reference, the Go library `Nerdmaster/poker` reports ~416 ns/eval
(~2.4 M/s) for `BenchmarkEvaluateOmaha` — `phe-omaha` is roughly
**6–8× faster** at the cost of a 22 MB precomputed lookup table.

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
