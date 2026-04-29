# poker-hand-evaluator — Implementation Brief

You are picking up a brand-new Rust workspace for a unified poker hand evaluator. Nothing has been written yet beyond this document. Build the workspace from scratch following the spec below. Reply in Japanese to your invoking user, but write all code, comments, commit messages, READMEs, and design notes in English.

## Mission

Build a unified poker hand evaluator crate that covers Hold'em high, 8-or-better low, A-5 lowball (Razz), 2-7 lowball, Omaha high, and optionally Badugi.

License: **MIT throughout** (no AGPL contamination — this crate must be reusable by any downstream tool, including the AGPL-licensed `poker-cuda-solver`).

The crate exposes a single `HandRule` trait with one impl per evaluation rule, allowing downstream consumers (notably `poker-cuda-solver` at `~/ghq/github.com/kyohah/poker-cuda-solver/`) to plug in evaluators without crate-level coupling.

## Repository

- GitHub: `kyohah/poker-hand-evaluator` (repo not yet created — create it private when ready, or leave local-only)
- Local path: `~/ghq/github.com/kyohah/poker-hand-evaluator/` (already exists, contains only `docs/` with this prompt)

## Reference repositories (clone before starting if not already)

1. **`b-inary/holdem-hand-evaluator`** (MIT) — port the entire crate as `crates/holdem/`. The license is MIT, so verbatim copy is allowed and we explicitly waive the original-author concern. The 4-line `Hand::evaluate()` table-lookup hot path and the table generation pipeline (`scripts/01-offset_table` → `02-lookup_tables`) are the gold standard — replicate both.
   - Clone with: `ghq get b-inary/holdem-hand-evaluator`
2. **`kyohah/8low-evaluator`** (MIT, owned by user) — already at `~/ghq/github.com/kyohah/8low-evaluator/`. Absorb into `crates/eight-low/`. It was already adapted from holdem-hand-evaluator, so structure is similar. Bring it in, restructure to be a workspace member, fix any path references.

## Critical design constraint: holdem and deuce-seven share the evaluation core

The 2-7 lowball evaluator is structurally **almost identical** to Hold'em high. The only structural divergences:

| Aspect | Hold'em high | 2-7 lowball |
|--------|--------------|-------------|
| Wheel A-2-3-4-5 | counts as straight | NOT a straight (just A-high no-pair) |
| Broadway 10-J-Q-K-A | counts as straight | counts as straight |
| Strength ordering | higher rank = stronger | lower rank = stronger (use `Reverse<u16>`) |
| Card encoding (rank index) | rank 0=2 ... 12=A | identical (A high in both) |

**The hot-path code (`Hand::evaluate()` 4-line table lookup) is identical between the two**. What differs is:
- Lookup table contents (different equivalence classes — deuce-seven has more, since wheel hands shift from "straight" to "high card")
- A thin wrapper that maps the raw rank to `Strength` (use `Reverse` for deuce-seven)

### Implementation requirement

Factor the shared bits into `crates/core/` so that `crates/holdem/` and `crates/deuce-seven/` differ **only by**:
- Their lookup table asset (`.bin` file)
- A trivial wrapper struct (3-5 lines)

Specifically:
- `crates/core/` — `Hand` struct, key/mask manipulation, `Add`/`AddAssign`, `Card` encoding, and a generic `evaluate_via_lookup(hand: &Hand, lookup: &[u16], offsets: &[u32]) -> u16` function used by both holdem and deuce-seven.
- `crates/holdem/` — only the lookup table asset + wrapper struct (`HighRule`) that calls the core function with the holdem table.
- `crates/deuce-seven/` — only the lookup table asset + wrapper struct (`DeuceSevenLowRule`) that calls the core function with the deuce-seven table and wraps result in `Reverse`.

The **table generator** in `scripts/` should likewise be parameterized:

```rust
pub enum WheelMode {
    StraightAndFlush,  // wheel A-2-3-4-5 counts as straight (Hold'em)
    NoPair,            // wheel A-2-3-4-5 counts as A-high no-pair (2-7 lowball)
}
```

Same generator code produces both tables. Reuse holdem-hand-evaluator's generator scripts as the base, add the `WheelMode` parameter, run twice. Do NOT write two separate generators.

## Workspace layout

```
poker-hand-evaluator/
├── Cargo.toml                     # workspace + facade crate
├── README.md
├── LICENSE                        # MIT
├── docs/
│   └── implementation-prompt.md   # this file
├── src/
│   └── lib.rs                     # HandRule trait + feature-gated re-exports
├── crates/
│   ├── core/                      # Hand, Card, evaluate_via_lookup, common utils
│   ├── holdem/                    # holdem-hand-evaluator port; lookup asset + thin wrapper
│   ├── eight-low/                 # kyohah/8low-evaluator absorbed; restructured to use core/ where possible
│   ├── deuce-seven/               # NEW; lookup asset + thin wrapper, generator uses WheelMode::NoPair
│   ├── omaha/                     # NEW; depends on holdem; enumerates C(4,2)*C(5,3)=60 combos
│   └── badugi/                    # OPTIONAL; can defer to phase 2
├── scripts/                       # asset generators (workspace member, not built by default)
│   ├── 01-offset_table/
│   ├── 02-lookup_tables/          # parameterized by WheelMode
│   └── ...
└── benches/                       # criterion benches
```

## Public API (facade crate, `src/lib.rs`)

```rust
pub trait HandRule: Send + Sync {
    type Strength: Ord + Copy + Send + Sync;
    fn evaluate(&self, cards: &[u8]) -> Self::Strength;
}

#[cfg(feature = "holdem")]      pub use phe_holdem::HighRule;
#[cfg(feature = "eight-low")]   pub use phe_eight_low::{EightLowQualifiedRule, AceFiveLowRule};
#[cfg(feature = "deuce-seven")] pub use phe_deuce_seven::DeuceSevenLowRule;
#[cfg(feature = "omaha")]       pub use phe_omaha::OmahaHighRule;
#[cfg(feature = "badugi")]      pub use phe_badugi::BadugiRule;

pub struct HiLoRule<H: HandRule, L: HandRule> { pub hi: H, pub lo: L }
impl<H: HandRule, L: HandRule> HandRule for HiLoRule<H, L> {
    type Strength = (H::Strength, L::Strength);
    fn evaluate(&self, cards: &[u8]) -> Self::Strength {
        (self.hi.evaluate(cards), self.lo.evaluate(cards))
    }
}
```

`Strength` ordering convention: **higher = stronger**. Low rules use `std::cmp::Reverse<u16>` so smaller raw rank becomes larger `Strength`.

## Cargo features (facade crate)

```toml
[features]
default = ["holdem", "eight-low"]
holdem = ["dep:phe-holdem"]
eight-low = ["dep:phe-eight-low"]
deuce-seven = ["dep:phe-deuce-seven"]
omaha = ["dep:phe-omaha"]                # transitively requires phe-holdem
badugi = ["dep:phe-badugi"]
all = ["holdem", "eight-low", "deuce-seven", "omaha", "badugi"]
```

Sub-crate package names use `phe-` prefix to avoid namespace clashes when published.

## Card encoding (must match across all sub-crates that share `core::Hand`)

Card ID = `rank * 4 + suit`:
- **Hold'em / 2-7 / Omaha**: rank 0=2, 1=3, ..., 8=T, 9=J, 10=Q, 11=K, 12=A (A high)
- **8-low / A-5 lowball**: rank 0=A (low), 1=2, ..., 12=K
- Suit: 0=club, 1=diamond, 2=heart, 3=spade

This means **8-low cannot reuse holdem's `Hand` directly** — it has its own encoding. That's fine; `phe-eight-low` keeps its own `Hand` (as the current 8low-evaluator does). Only holdem and deuce-seven share `core::Hand`.

`crates/core/` should expose two `Hand` variants if needed (high-encoding and low-encoding) OR be parameterized by an `Encoding` trait. Pick whichever yields less code.

## Sub-crate implementation matrix

| Sub-crate | Source | Shares core with | Status |
|-----------|--------|------------------|--------|
| `phe-holdem` | port from `b-inary/holdem-hand-evaluator` | deuce-seven (high encoding) | PORT |
| `phe-eight-low` | absorb `kyohah/8low-evaluator` | (own low encoding) | MOVE |
| `phe-deuce-seven` | NEW; generator forked from holdem with `WheelMode::NoPair` | holdem (high encoding) | NEW |
| `phe-omaha` | NEW; thin wrapper enumerating C(4,2)*C(5,3) combos, delegates to phe-holdem | holdem | NEW |
| `phe-badugi` | NEW | (own impl, 4-card no-rank no-suit collisions) | OPTIONAL / DEFER |

### About `AceFiveLowRule` (Razz)

Razz uses A-5 lowball with no qualifier. The 8low-evaluator's lookup already returns the correct rank — Razz just doesn't apply the `qualifies_8_or_better` filter. So:

- Do NOT create a separate `phe-ace-five-low` crate.
- Inside `phe-eight-low`, expose **two** rule types from the same evaluator:
  - `EightLowQualifiedRule` — current behavior, `Strength = Option<Reverse<u16>>`, returns `None` when not 8-or-better.
  - `AceFiveLowRule` — same lookup, `Strength = Reverse<u16>`, returns rank unconditionally (no qualifier).

## Test requirements

Mirror `8low-evaluator/src/hand_test.rs`'s strategy:
- **Exhaustive 5-card validation**: enumerate all C(52, 5) = 2,598,960 hands, cross-check the fast evaluator against a naive reference, verify category counts match published combinatorics.
- **Exhaustive 7-card validation**: enumerate all C(52, 7) = 133,784,560 hands. This is slow (~5 min), gate behind `#[ignore]` so it can be run via `cargo test -- --ignored` and excluded from CI default.
- **Holdem 7-card known counts**: high card 23,294,460 / one pair 58,627,800 / two pair 31,433,400 / three of a kind 6,461,620 / straight 6,180,020 / flush 4,047,644 / full house 3,473,184 / four of a kind 224,848 / straight flush 41,584. Total = 133,784,560.
- **Deuce-seven 7-card counts**: derive from a naive reference; the generator should output these for documentation.
- Run all tests with `--release` (debug mode is too slow for full enumeration).

## Benchmarks (criterion, optional but recommended)

- 7-card full enumeration time. Target: match holdem-hand-evaluator's ~63ms baseline.
- Single-hand evaluation latency. Target: < 50 ns/call (lookup-table dominated).

## Sequencing

Strict order — do not skip ahead:

1. **Bootstrap**: `cargo init --lib`, write workspace `Cargo.toml`, MIT `LICENSE`, `README.md` skeleton, `.gitignore`. Commit `chore: bootstrap workspace`.
2. **`crates/core/`**: implement `Hand`, `Card`, `evaluate_via_lookup`, `Add`/`AddAssign`. No tests yet beyond compile checks.
3. **`crates/holdem/`** (PORT): copy holdem-hand-evaluator as the workspace member. Make it use `core::Hand` if sensible; otherwise keep its own and refactor later. Get the existing test suite passing under `cargo test --release -p phe-holdem`.
4. **`crates/eight-low/`** (MOVE): copy `kyohah/8low-evaluator` contents in. Adjust `Cargo.toml` (workspace path deps for `assets/`, `scripts/`). Add the `AceFiveLowRule` sibling export. Get the existing test suite passing.
5. **`crates/deuce-seven/`** (NEW): the proof of the "shared core" thesis. Should be ~50 lines + lookup asset. Generator is a parameterized version of holdem's. Run exhaustive 7-card cross-validation against a naive reference to verify wheel handling.
6. **`crates/omaha/`** (NEW): wrapper around `phe-holdem`. The hot path enumerates 4 hole + 5 board, picks best of C(4,2)*C(5,3) = 60 combos. Test against known Omaha equity calculations (e.g., AAxx vs 2-random has documented equity).
7. **Facade crate `src/lib.rs`**: expose `HandRule` trait, feature-gated re-exports, `HiLoRule` composite.
8. **Update `poker-cuda-solver`**: edit `~/ghq/github.com/kyohah/poker-cuda-solver/Cargo.toml` to replace the existing two evaluator deps:
   ```diff
   - holdem-hand-evaluator = { git = "https://github.com/b-inary/holdem-hand-evaluator" }
   - eight-low-evaluator = { path = "../8low-evaluator" }
   + poker-hand-evaluator = { path = "../poker-hand-evaluator", features = ["all"] }
   ```
   And adjust `poker-cuda-solver/src/rule/mod.rs` to either re-export from this crate or remove the `unimplemented!()` placeholders by delegating to `poker-hand-evaluator`.
9. **`crates/badugi/`** (OPTIONAL): defer. Open an issue or TODO if not done.

## Out of scope

- Do NOT implement `Game` trait, solver code, or anything CFR-related. This crate is pure hand evaluation. CFR / solver lives in `poker-cuda-solver`.
- Do NOT add async / GPU / SIMD optimizations. Lookup-table speed is already sufficient for setup-time evaluation; per-iteration evaluation never happens (caller caches).
- Do NOT publish to crates.io. Workspace stays local-only for now.
- Do NOT add CLI tools, REPLs, or example binaries beyond the asset generators in `scripts/`.

## Coordinate with the user

Once you finish each major step (1–8 above), report back to the user in Japanese with a one-line status. Do not batch all 8 into one report. The user wants to verify the "code save" claim particularly at step 5 (deuce-seven implementation) — at that point, present the diff between `phe-holdem` and `phe-deuce-seven` and confirm the only differences are the asset and the wrapper.

## Files you can ignore in `~/ghq/github.com/kyohah/poker-cuda-solver/`

- `src/rule/mod.rs` and `src/variants/mod.rs` are placeholder scaffolding from a prior session. They will be partly subsumed by this crate (specifically `src/rule/mod.rs::HandRule` moves to `poker-hand-evaluator`). Don't refactor them yet — the integration step (#8) handles that.
