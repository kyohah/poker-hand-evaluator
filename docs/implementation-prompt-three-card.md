# Implementation Prompt — `phe-three-card` (3-card high evaluator)

You are a fresh Claude Code session. You have no prior context for this task. Read this entire document, then execute it. Stop when the completion criteria are met.

## Mission (one sentence)

Add a new sub-crate `phe-three-card` to this workspace that evaluates 3-card high hands (HighCard / Pair / Trips) and wire it through the facade as a `HandRule` impl, **so a downstream OFC solver can score the top row of an Open-Face Chinese poker board**.

This is **PR #1** of a cross-repo project. It is a single, focused PR — do not scope-creep.

## Why this exists (so you can make judgment calls)

Open-Face Chinese poker has 3 rows: top (3 cards), middle (5), bottom (5). Middle and bottom are scored by the existing `phe_holdem::HighRule`. Top is **not** a subset of HighRule's domain — `AKQ` (high card) cannot exist as a 5-card hand, but is a perfectly valid 3-card top — so a dedicated 3-card evaluator is required.

The downstream consumer is a new crate `ofc-solver` living in a sibling repo (`~/ghq/github.com/kyohah/poker-ofc-solver/`, not yet created). That solver runs branch-and-bound over 8.6 × 10⁷ partitions per Fantasy-Land hand and hits this evaluator on every leaf, so the implementation must be **fast** and **branchy** (no allocation, no parallelism, inline-friendly). Joker handling is the OFC solver's concern, **not yours** — see §"Out of scope" below.

The full architectural plan, including why `phe-three-card` lives here vs. up-stream, is in the host repo:
`~/ghq/github.com/kyohah/poker-solver/docs/ofc-fl-solver-plan.md`. Read it if a design choice in this prompt is ambiguous; don't read it if everything is already clear.

## Required reading before you write code

1. `CLAUDE.md` at the workspace root — the workspace conventions, especially **"No parallelism in `phe-*` crates"** (this rule is non-negotiable; do not add `rayon`, threads, or atomic loops).
2. `src/lib.rs` (the facade) — understand the `HandRule` trait, the card encoding contract, and how existing rules are gated by features.
3. `crates/badugi/Cargo.toml` and `crates/badugi/src/lib.rs` — your closest analogue. Badugi is a small, self-contained evaluator that does **not** depend on `phe-core`'s perfect-hash machinery. Mirror its layout. (You may copy its `Cargo.toml` shape verbatim.)

You do **not** need to read `phe-holdem`, `phe-omaha`, or any `*-assets` crate. They use a perfect-hash construction that is overkill for 3 cards.

## Concrete spec

### Card encoding (input contract)

Hold'em-style: `card = rank * 4 + suit`, with rank `0='2', ..., 12='A'` (Ace high) and suit `0=club, 1=diamond, 2=heart, 3=spade`. This is exactly what the facade `HandRule::evaluate(&[u8])` accepts. Three-card hands cannot make a flush or straight, so suit only matters for the (currently empty) tiebreak set — but you must still accept the suit byte and ignore it correctly.

### Strength encoding

`type Strength = u16`. **Higher = stronger** (matches `HandRule` contract).

Layout (16 bits, MSB to LSB):

```
  bits 15..12  : category   (0=HighCard, 1=Pair, 2=Trips)
  bits 11..0   : within-category index, packed below
```

Within-category packing:

| Category   | Encoding                                                                |
|------------|-------------------------------------------------------------------------|
| HighCard   | `(top << 8) \| (mid << 4) \| low`, ranks sorted descending              |
| Pair       | `(pair_rank << 4) \| kicker`                                            |
| Trips      | `trip_rank` (only the bottom 4 bits used)                               |

This makes derived `Ord` correct: trips beat any pair, any pair beats any high card, and within a category the higher rank tuple wins.

Write at minimum these spot-checks as an early test (in `tests/ordering.rs`):

- `evaluate(AKQ off-suit) < evaluate(2 2 3 off-suit)`
- `evaluate(AAK) < evaluate(2 2 2)`     // any pair < any trips
- `evaluate(AAA) > evaluate(KKK)`       // trip rank tiebreak
- `evaluate(AAQ) > evaluate(AAJ)`       // pair kicker tiebreak
- `evaluate(AKQ) > evaluate(AKJ)`       // high-card kicker

(Use real Hold'em-encoded cards in the test, e.g. `12*4 + 3` for A♠.)

### Public API (sub-crate `phe-three-card`)

```rust
//! 3-card high evaluator for OFC top row.
//!
//! …doc comment explaining strength encoding and contract…

#![warn(missing_docs)]
#![forbid(unsafe_code)]

/// Tag-only struct so callers can write `ThreeCardRule.evaluate(&cards)`
/// through `HandRule` without constructing anything.
pub struct ThreeCardRule;

impl ThreeCardRule {
    /// Direct, allocation-free entry. Branches on the rank-multiset
    /// of the three cards. Inlinable into the OFC solver hot path.
    #[inline]
    pub fn evaluate(cards: [u8; 3]) -> u16 { /* impl */ }
}
```

You do **not** need to expose a `ThreeCardStrength` newtype — `u16` is the contract and downstream code (`ofc-royalty`) will decode the category bits directly. If a private helper struct makes the implementation cleaner, keep it `pub(crate)`.

### Facade integration (workspace root `src/lib.rs` + `Cargo.toml`)

1. Add `phe-three-card = { path = "crates/three-card", optional = true }` to the facade's `[dependencies]`.
2. Add a `three-card = ["dep:phe-three-card"]` feature, and add `"three-card"` to the `all` feature list.
3. Add `crates/three-card` to the workspace `[workspace] members =` list (root `Cargo.toml`).
4. In facade `src/lib.rs`, mirror the existing pattern:

```rust
#[cfg(feature = "three-card")]
pub use phe_three_card as three_card;
#[cfg(feature = "three-card")]
pub use phe_three_card::ThreeCardRule;

#[cfg(feature = "three-card")]
impl HandRule for ThreeCardRule {
    type Strength = u16;
    fn evaluate(&self, cards: &[u8]) -> u16 {
        assert_eq!(cards.len(), 3, "ThreeCardRule expects exactly 3 cards");
        phe_three_card::ThreeCardRule::evaluate([cards[0], cards[1], cards[2]])
    }
}
```

Place the facade `impl` block alongside the other `HandRule` impls — they're easy to spot.

### File layout

```
crates/three-card/
├── Cargo.toml          (mirror badugi: name = "phe-three-card", description …)
└── src/
    └── lib.rs          (~120 LoC: doc comment, ThreeCardRule, evaluate fn, #[cfg(test)] mod)
```

Workspace-root tests live under `crates/three-card/tests/` only if a test grows beyond what fits inline in `src/lib.rs`'s `#[cfg(test)] mod tests`. Two separate files are fine if it improves readability:

```
crates/three-card/tests/
├── strength.rs         (Hold'em-encoded fixture-driven category checks)
└── ordering.rs         (the spot-checks above + ~70 random pairs proven by sort)
```

### Implementation hint (use it or argue against it in a comment)

Branch on the rank-multiset count pattern. Given three rank indices `r1 ≥ r2 ≥ r3` (sort first):

- If `r1 == r2 == r3` → Trips
- Else if `r1 == r2` or `r2 == r3` → Pair (find which two are equal; identify pair_rank and kicker)
- Else → HighCard

This is ~20 lines of plain Rust with a single `match` on `(r1 == r2, r2 == r3)`. No lookup table needed for 3 cards (`13³ = 2197` distinct rank-tuples, all fit in a branchy `match`).

## Test strategy

In addition to the spot-check ordering tests above:

- **Inline unit tests** in `src/lib.rs` covering each of the three categories (≥ 1 fixture each).
- **Property test** (no `proptest` dep needed; a hand-rolled exhaustive loop is fine — only `52³` ≈ 140 K combinations): for every triple of distinct cards, `evaluate(triple) == evaluate(any_permutation_of_triple)`. Suit-permutation invariance + rank-permutation invariance both hold.
- **Facade-level test** (in `tests/` of the workspace root, *not* in this sub-crate) confirming `(ThreeCardRule as HandRule).evaluate(&[A♠, K♠, Q♠]) > (ThreeCardRule as HandRule).evaluate(&[2♣, 7♦, 9♥])` — proves the facade gate works.

Aim for **~15 distinct test cases total**, not "80+". The earlier spec said 80; that was a copy-paste from a perfect-hash crate's coverage requirement. For a 3-card branchy evaluator, the input space is small enough that exhaustive property-style coverage **is** the comprehensive test.

## Out of scope (do NOT do these)

- **Joker / wildcard support.** The OFC rules involve 0–2 Jokers per hand. `phe-three-card` must remain **joker-free**. The OFC solver will run `argmax over substitutions` on top of this evaluator. Do not add a `num_jokers` parameter, do not accept `card == 52 || card == 53`, do not add a `evaluate_with_jokers` method anywhere.
- **CUDA backend.** No GPU port for 3-card evaluation; the table is too small to amortize launch overhead.
- **OFC concepts.** No royalty tables, foul detection, stay-in-FL detection, or scoring helpers. Those live in the OFC solver crate.
- **Any other `phe-*` crate changes.** Do not touch `phe-holdem`, `phe-core`, etc. Facade integration is the only top-level change.
- **Workspace-wide refactors.** If you spot something off in another crate, mention it in the PR description; do not fix it in this PR.

## Completion criteria

All of these must hold before you call the work done:

1. `cargo build --workspace` succeeds with default features.
2. `cargo test -p phe-three-card` succeeds.
3. `cargo test --features three-card` (workspace root) succeeds — proves the facade gate works.
4. `cargo test --no-default-features --features three-card` succeeds — proves you didn't accidentally couple to default features.
5. `cargo doc --features three-card --no-deps` is warning-free (`#![warn(missing_docs)]` is enforced).
6. `cargo clippy --features three-card -- -D warnings` is clean.
7. The new evaluator has **no `rayon`, no thread spawn, no `Arc<Mutex<…>>`** in the implementation.
8. Test count for the new crate ≥ 15, and at least one of them is the exhaustive permutation-invariance test described in §Test strategy.

When all 8 are green, write a single commit with message `feat(three-card): 3-card high evaluator for OFC top row` and stop.

## Likely pitfalls (read once, save yourself an hour)

- **Don't over-pack the strength.** The spec gives 4 bits for category and 12 for within-category. You only need ~13 = 4 bits for the highest rank. Don't use bit-fiddling to compress further; clarity beats one extra bit.
- **Don't forget `assert_eq!(cards.len(), 3)`** in the facade impl. The trait takes `&[u8]`, so length is a runtime contract.
- **Don't be tempted to reuse `phe_holdem::evaluate`** by padding to 5 cards with low duplicates. It produces wrong category boundaries (e.g. `AKQ22` is a pair, but the top row `AKQ` is a high card).
- **Don't add `#[derive(PartialOrd, Ord)]` on a strength struct.** The contract is plain `u16`. Derived ordering is what you want, but on the bare integer.
- **Don't make the API generic** (`evaluate<T: Into<[u8; 3]>>(...)`). The hot path needs to inline; a concrete `[u8; 3]` is the right signature.
- **The facade `HandRule` impl block** lives in `src/lib.rs`, not in the sub-crate. The sub-crate has zero knowledge of `HandRule`.

## Done?

When complete, the next consumer is the OFC solver workspace at `~/ghq/github.com/kyohah/poker-ofc-solver/` (not yet created). The handoff is purely the published `phe-three-card` API + the facade `HandRule` impl. You don't need to do anything to set up the consumer.
