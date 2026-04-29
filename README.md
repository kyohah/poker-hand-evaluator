# poker-hand-evaluator

A unified poker hand evaluator covering multiple variants behind a single
`HandRule` trait, designed to be embedded by downstream solvers.

## Status

Bootstrap stage. See [`docs/implementation-prompt.md`](docs/implementation-prompt.md)
for the full specification and roadmap.

## Variants

| Feature flag   | Rule                                          | Crate                |
|----------------|-----------------------------------------------|----------------------|
| `holdem`       | Hold'em high (5-7 cards)                      | `phe-holdem`         |
| `eight-low`    | 8-or-better low + A-5 lowball (Razz)          | `phe-eight-low`      |
| `deuce-seven`  | 2-7 lowball                                   | `phe-deuce-seven`    |
| `omaha`        | Omaha high (4 hole + 5 board, best-of-60)     | `phe-omaha`          |

`default = ["holdem", "eight-low"]`. Use `features = ["all"]` to pull every
variant.

## Workspace layout

```
crates/
  core/                 Hand / Card / lookup-driven evaluator core
  holdem/               port of b-inary/holdem-hand-evaluator (MIT)
  holdem-assets/        precomputed lookup + offset tables
  eight-low/            absorbed kyohah/8low-evaluator
  eight-low-assets/
  deuce-seven/          new; lookup tables generated with WheelMode::NoPair
  deuce-seven-assets/
  omaha/                wrapper around phe-holdem (60-combo enumeration)
scripts/                asset generators (offset table + lookup tables)
src/lib.rs              facade crate exposing HandRule + feature-gated re-exports
```

## License

MIT. The Hold'em evaluator core is derived from
[`b-inary/holdem-hand-evaluator`](https://github.com/b-inary/holdem-hand-evaluator)
(MIT). See `LICENSE` for the combined notice.
