# Follow-up: port HenryRLee's PLO4 perfect-hash to Rust

The current `phe-omaha` is a 60-combo wrapper around `phe-holdem` (`C(4,2)*C(5,3)`
enumeration over the 7-card high evaluator). On a recent benchmark this came in
around **~416 ns / hand**, vs **~30.5 ns / hand** for HenryRLee/PokerHandEvaluator's
PLO4 — a 13× gap driven entirely by algorithm choice, not implementation
quality.

Add a second Omaha evaluator, `phe-omaha-fast`, that ports HenryRLee's
multiset-hash + precomputed-best-of-combo approach from C to Rust. Keep the
existing `phe-omaha` for callers that don't want to pay the binary-size cost
(~+30 MB).

## Reference source

Local clone (already present):

    ~/ghq/github.com/HenryRLee/PokerHandEvaluator/

Files to read & port:

| C file | role | size |
|--------|------|------|
| `cpp/src/evaluator_plo4.c` | top-level `evaluate_plo4_cards()` (~70 lines) | 4.6 KB |
| `cpp/src/hash.c` + `hash.h` | `hash_binary()`, `hash_quinary()` (~50 lines) | small |
| `cpp/src/dptables.c` + `tables.h` | `dp[5][13][6]` array + `choose[][]` table | small |
| `cpp/src/tables_plo4.c` | `flush_plo4[4_099_095]` + `noflush_plo4[11_238_500]` const `short` arrays | 92 MB textual |
| `cpp/src/rank.c` | `get_rank_category()` mapping for category enum (optional, for parity) | small |

License: **Apache-2.0**. Compatible with the workspace's MIT license; the new
sub-crate must dual-license appropriately and ship a `LICENSE-APACHE` /
`NOTICE` covering the ported portion.

## Algorithm summary (so you can implement it without re-reading the C)

### Card encoding

HenryRLee uses `card = rank * 4 + suit`, identical to `phe-core::CARDS`. So
input cards can be passed through unchanged.

### Output convention

HenryRLee returns an `int` rank in `[1, 7462]` where **lower = stronger**
(Cactus-Kev convention). Convert at the Rust boundary so this crate's
`OmahaHighFastRule` follows the workspace's "higher = stronger" `u16` contract:

```rust
fn evaluate(&self, cards: &[u8]) -> u16 {
    assert_eq!(cards.len(), 9);
    let raw_low = evaluate_plo4_cards(...);   // [1, 7462], lower better
    7463 - (raw_low as u16)                   // [1, 7462], higher better
}
```

### Evaluation procedure (verbatim from `evaluator_plo4.c`)

1. **Compute suit counts** for the 5 board cards and 4 hole cards
   (`suit_count_board[4]`, `suit_count_hole[4]`).

2. **Flush path** — triggered only when some suit has `≥ 3` board cards AND
   `≥ 2` hole cards (the only way a 5-card flush can come from 3-of-board +
   2-of-hole):
   - Build per-suit rank bitmasks (`suit_binary_board[s]`, `suit_binary_hole[s]`).
   - **Special case** `suit_count_board == 3 && suit_count_hole == 2`: do a
     direct lookup `flush[suit_binary_board | suit_binary_hole]` — table is the
     same 5-card flush table phe-holdem already has, share it where possible.
   - **General case**: pad the rank binaries to 5 / 4 bits set respectively
     (using the documented `padding[3] = {0x0000, 0x2000, 0x6000}` constants),
     hash both with `hash_binary` (which gives indices `[0, 1364]`), then
     `flush_plo4[board_hash * 1365 + hole_hash]`.
   - Take the *minimum* (= strongest) flush rank found across the 4 suits.

3. **Non-flush path** (always evaluated):
   - Build "quinary" histograms: `quinary_board[13]` summing to 5,
     `quinary_hole[13]` summing to 4.
   - `hash_quinary(quinary_board, 5)` → board index in `[0, 1819]`.
   - `hash_quinary(quinary_hole, 4)`  → hole index  in `[0, 1819]`.
   - `noflush_plo4[board_hash * 1820 + hole_hash]`.

4. **Final**: `min(value_flush, value_noflush)` — lower = stronger in the C
   convention, before the boundary conversion above.

### `hash_quinary` and `hash_binary`

Both are lexicographic-rank hashes over multisets / k-subsets, computed in
~13 iterations using a precomputed DP table:

```rust
fn hash_quinary(q: &[u8; 13], mut k: i32) -> i32 {
    let mut sum = 0;
    for i in 0..13 {
        sum += DP[q[i] as usize][12 - i][k as usize];
        k -= q[i] as i32;
        if k <= 0 { break; }
    }
    sum
}

fn hash_binary(mut bin: i32, mut k: i32) -> i32 {
    let mut sum = 0;
    for i in 0..15 {
        if bin & (1 << i) != 0 {
            if 14 - i >= k { sum += CHOOSE[14 - i][k as usize]; }
            k -= 1;
            if k == 0 { break; }
        }
    }
    sum
}
```

`DP[q][len_remaining][k]` and `CHOOSE[n][k]` come from `cpp/src/dptables.c`.
Port that file as a static array.

### Tables

`tables_plo4.c` declares:

```c
const short flush_plo4[4099095];     // 4_099_095 × 2 B = 8.2 MB
const short noflush_plo4[11238500];  // 11_238_500 × 2 B = 22.5 MB
```

Two viable Rust strategies:

**(A) `include!`-style** — bring the C arrays in as Rust `const [u16; N]`
arrays. Easiest: a script that converts the `short` literals to a Rust file.
Compile-time only, no runtime parsing. This is what `phe-holdem-assets`
already does.

**(B) `include_bytes!`-style** — pre-pack the tables to a single `.bin` per
table, ship in `phe-omaha-fast-assets/`, parse at startup with a single
`OnceLock<Vec<u16>>`. Slightly faster build, slightly more startup work. This
matches `phe-eight-low-assets` if I recall.

Pick whichever lines up with the existing assets-crate style. Either way the
~30 MB pays for itself the moment downstream calls `evaluate` more than once.

## Workspace layout

```
poker-hand-evaluator/
├── crates/
│   ├── omaha/                  # existing 60-combo (keep)
│   ├── omaha-fast/             # NEW
│   │   ├── Cargo.toml
│   │   ├── LICENSE-APACHE      # full Apache-2.0 text
│   │   ├── NOTICE              # "Portions ported from HenryRLee/..."
│   │   └── src/
│   │       ├── lib.rs          # OmahaHighFastRule + public API
│   │       ├── eval.rs         # evaluate_plo4_cards
│   │       ├── hash.rs         # hash_binary, hash_quinary
│   │       └── dp.rs           # DP / CHOOSE constant arrays
│   └── omaha-fast-assets/      # NEW (or fold into omaha-fast if small enough)
│       ├── Cargo.toml
│       └── src/
│           ├── lib.rs          # pub use of FLUSH_PLO4 / NOFLUSH_PLO4
│           ├── flush_plo4.rs   # const arrays (generated)
│           └── noflush_plo4.rs # const arrays (generated)
└── scripts/
    └── port-plo4-tables/       # one-off binary that reads C source and
                                # writes the Rust const-array files
```

## Facade wiring

`src/lib.rs` (workspace root, the facade crate):

```rust
#[cfg(feature = "omaha-fast")]
pub use phe_omaha_fast as omaha_fast;

#[cfg(feature = "omaha-fast")]
pub use phe_omaha_fast::OmahaHighFastRule;

#[cfg(feature = "omaha-fast")]
impl HandRule for OmahaHighFastRule {
    type Strength = u16;
    fn evaluate(&self, cards: &[u8]) -> u16 {
        assert_eq!(
            cards.len(), 9,
            "OmahaHighFastRule expects 4 hole + 5 board = 9 cards, got {}",
            cards.len()
        );
        // First 4 = hole, last 5 = board (matches OmahaHighRule's existing
        // convention). The phe-omaha-fast inherent fn takes board first then
        // hole, mirroring HenryRLee's signature.
        phe_omaha_fast::OmahaHighFastRule::evaluate(
            cards[4] as i32, cards[5] as i32, cards[6] as i32, cards[7] as i32,
            cards[8] as i32,
            cards[0] as i32, cards[1] as i32, cards[2] as i32, cards[3] as i32,
        )
    }
}
```

`Cargo.toml` (workspace root):

```toml
[features]
default = ["all"]
holdem        = ["dep:phe-holdem"]
eight-low     = ["dep:phe-eight-low"]
deuce-seven   = ["dep:phe-deuce-seven"]
omaha         = ["dep:phe-omaha", "dep:phe-holdem"]
omaha-fast    = ["dep:phe-omaha-fast"]    # NEW
badugi        = ["dep:phe-badugi"]
all           = ["holdem", "eight-low", "deuce-seven", "omaha", "omaha-fast", "badugi"]
```

`omaha-fast` is **not** in `default` because the +30 MB binary cost is too
much for default. Callers opt in by feature.

## Verification

1. **Bit-exact parity with C**: cross-check against the C reference for a
   sample of (board, hole) pairs. The exhaustive comparison would be
   `C(52,5) * C(47,4) ≈ 700 B` evals, way too many; instead pick a few
   thousand random hands plus all the corner cases (royal-flush boards,
   four-flush boards, suited hole pairs, etc.) and assert byte-equal output
   with `cpp/src/evaluator_plo4.c` compiled separately.

2. **Internal cross-check vs `phe-omaha`**: for the same input,
   `phe_omaha::OmahaHighRule::evaluate(hole, board)` and
   `phe_omaha_fast::OmahaHighFastRule::evaluate(...)` must agree on
   *ordering* (not absolute u16, since the canonical scales differ). I.e.,
   for any pair of hands, both impls must agree on which one wins. This is
   the property the downstream solver actually depends on.

3. **Throughput target**: the criterion bench should show ≤ 50 ns / hand
   on the same hardware that runs the existing `phe-omaha` bench at ~416 ns
   — i.e., reproduce HenryRLee's ~30 ns at minimum within 50% slack.

## Out of scope for this task

- **PLO5 / PLO6**: HenryRLee ships these too (~33 ns / 34 ns) but at
  112 MB / 354 MB binary cost. Defer until / unless there's a real
  consumer demand. Fold into the same crate when added.
- **Lowball Omaha (Hi-Lo / 8-or-better)**: needs its own dedicated table
  and is structurally a different problem. Not part of this port.

## After it lands

Update `poker-cuda-solver/Cargo.toml` to enable the feature only if
realistic-range Omaha solving becomes a bottleneck — the solver only calls
the evaluator at setup time, so the 13× speedup is rounding noise on a
multi-iteration solve. The win is for downstream tools using
`poker-hand-evaluator` standalone (equity calculators, range-vs-range, etc.).

Commit as `feat(omaha-fast): port HenryRLee PLO4 perfect-hash to Rust`.
