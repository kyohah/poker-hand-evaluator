# Omaha evaluator performance — investigation report

Date: 2026-04-29 (during a 5-hour offline window).
Branch: `main` at HEAD `2e88c26` (path 3 Hand bypass landed).
Bench: `cargo run --release -p phe-omaha --example exhaustive` (10s
random-stream throughput, single-threaded), full results in
`bench/history.csv`.

## TL;DR

- **Current**: ~140-150 ns/eval (~6.7 M evals/s) on random Omaha hands.
- **Holdem 7-card single-eval**: ~10 ns/eval (one `LOOKUP`).
- **Slowdown vs Hold'em**: 14× — user wants this down to ~3× (~30 ns).
- **Within-architecture limit**: ~120-180 ns (60 lookups × 2-3 ns each
  in our 144 KB `LOOKUP` table). Already mostly there.
- **The big lever**: switch the 5-card kernel from our 144 KB perfect-
  hash table (L2-bound) to **Cactus-Kev's 48 KB tables** (L1-resident).
  Same 60-combo enumeration; faster per-combo because the working set
  fits L1. Reference impls (Nerdmaster Go) hit **37 ns/eval = ~4× our
  current**. Estimated effort: 2-4 hours.
- **The bigger lever (deferred)**: PHEvaluator-style **quinary
  perfect-hash** — encodes the 9-card "must use 2 hole + 3 board"
  constraint into a single lookup table. ~30 MB. Reduces non-flush
  Omaha to **1 lookup**, theoretical limit ≈ Hold'em + small overhead.
  Effort 1-2 weeks.

---

## What we tried in this session

| Commit  | Change                                                         | mean ns/eval |
|---------|----------------------------------------------------------------|-------------:|
| 4767ffc | baseline (60-combo brute force, `Hand` build per combo)       | 195         |
| dbf6532 | board-partial cache + flush-impossible fast path              | 171         |
| 891ad07 | flush-dominates path (suit-restricted enum, no-pair board)    | 160         |
| 9a16dd1 | + `board_no_straight` predicate (no eval change)              | 161 (noise) |
| cb041bc | + `upper_bound_category` helper (no eval change)              | 159 (noise) |
| f7861c5 | path 2 (flush_dominate) bypasses Hand + LOOKUP_FLUSH direct   | 158 (noise) |
| 8c457b5 | path 1 (no flush) bypasses Hand, direct `RANK_BASES` sum      | 147         |
| 2e88c26 | path 3 (flush + board pair) bypasses Hand, per-combo dispatch | 143-160     |

**Findings**:
- `Hand`-build elimination on path 1 (67% of random hands) was the
  biggest win — paths 2 (20%) and 3 (13%) each gave only marginal
  improvements within their respective slices.
- Holding `OFFSETS`/`LOOKUP` accesses unchanged means we are
  **L2-bound on lookup latency**, not on instruction count. Optimising
  the inner-loop arithmetic gives diminishing returns once `Hand` is out
  of the picture.
- `#[inline(always)]` on `evaluate_rank_only_from_key` was tested and
  **made things slower** (159 → 168 ns), likely due to ICache pressure
  from inlining 60 copies of the lookup chain. Reverted.
- The earlier branch-and-bound prune attempt (`upper_bound_category` +
  sort + skip) also made things slower (143 → 251 ns / +43%). Overhead
  of 6 `upper_bound` calls + 6-element sort + `board_no_straight`
  recomputation dominated savings; on random hands the prune rarely
  fires because category upper bounds cluster at the same value (path 1
  with no board pair: every hole pair has ub=4 or ub=2).

## What other implementations do

### Cactus-Kev / Senzee 5-card kernel (Nerdmaster Go ref)

Source: <https://github.com/Nerdmaster/poker> (`evaluator.go` 252 LOC),
modeled on Cactus-Kev <http://suffe.cool/poker/evaluator.html> with
Senzee's perfect-hash collapse.

Reported throughput: **26.75 M evals/sec / 37 ns/eval** for Omaha PLO4
brute-force. Algorithm is the same naive 60-combo enumeration as ours;
the gap is in the 5-card kernel.

```go
func evalFiveFast(c1, c2, c3, c4, c5 Card) uint16 {
    var q = (c1 | c2 | c3 | c4 | c5) >> 16
    if (c1 & c2 & c3 & c4 & c5 & 0xf000) != 0 { return flushes[q] }
    if unique5[q] != 0 { return unique5[q] }
    var product = (c1&0xff) * (c2&0xff) * (c3&0xff) * (c4&0xff) * (c5&0xff)
    return hashValues[findFast(uint32(product))]
}
```

Card layout (32-bit):
| bits  | meaning                                  | used by                |
|-------|------------------------------------------|------------------------|
| 16-28 | `1 << rank`                              | OR-merge straight/flush detect |
| 12-15 | suit bit (`1 / 2 / 4 / 8` for s/h/d/c)   | AND-merge flush detect |
| 8-11  | rank ordinal (0..12)                     |                        |
| 0-7   | rank prime (`2, 3, 5, ..., 41`)          | non-flush perfect hash |

Tables (all `[]u16`, 2 bytes each):
| name         | entries | size   |
|--------------|--------:|-------:|
| `flushes`    | 7937    | ~16 KB |
| `unique5`    | 7937    | ~16 KB |
| `hashValues` | 8192    | 16 KB  |
| `hashAdjust` | 512     | 1 KB   |
| **total**    |         | **~49 KB** |

**Key vs ours**: 49 KB fits L1d (32 KB on most x86, 48 KB on newer
Intel). Our 144 KB LOOKUP **does not** — every probe is L2 (~12 cycles)
instead of L1 (~4 cycles). Over 60 probes that's ~480 cycles vs ~240
cycles, matching the observed ~4× gap.

The arrays are stable (Cactus-Kev never updated them) and freely
copyable; the upstream `holdem-hand-evaluator` project even ships them
verbatim as `scripts/src/kev/arrays.rs` already in our local clone.

### PHEvaluator quinary perfect-hash (HenryRLee)

Source: <https://github.com/HenryRLee/PokerHandEvaluator>
(`cpp/src/evaluator_plo4.cc`, `hash.c`, table file `tables_plo4.tar.gz`).

Algorithm:
1. Represent the 9 cards by per-rank counts `c[0..12]` (each in 0..4).
2. Encode as a 13-digit base-5 number, **with the constraint "exactly
   2 cards from hole positions, 3 from board positions"** baked in.
3. Build a perfect-hash from valid quinary numbers to the precomputed
   answer.

Result: **one lookup per non-flush Omaha eval**. (Flushes go through a
separate ~16 KB suit-mask table similar to Cactus-Kev's `flushes`.)

Trade-off: the table is **~30 MB** for PLO4. Reported throughput
~33 M evals/s (~3000 ns) per agent research, slower than ours in
absolute numbers — but the **algorithm** is what matters; the C
implementation has table-init and PIC overhead a Rust port can avoid.

Effort to port: 1-2 weeks.

Footnote: the table needs to be regenerated to fit our `(category <<
12) | within_category_index` packed-u16 convention; PHEvaluator stores
its own categorisation. A small post-process step on the table is
sufficient; doesn't change the runtime path.

### OMPEval (zekyll, Hold'em only)

Source: <https://github.com/zekyll/OMPEval>.

Two transferable ideas:
- **Suit isomorphism** at equity-calculator level: when computing
  equity over many runouts, canonicalise (hand, board) by remapping
  suits so suit appearances are sorted. Reported "**~3× speedup**" but
  only in equity loops over many runouts — does **nothing** for a
  single random Omaha eval.
- **Cached partial state in `Hand`**: amortise per-pair / per-board
  precomputation over many evals against the same partial state.
  Already half-applied via our board-partial cache; the OMPEval code
  pushes further by caching across multiple Omaha calls when the board
  is fixed (relevant for solver hot loops, not for random benches).

Hold'em-only single-eval throughput in OMPEval is **~520 M evals/sec /
~2 ns**. That's the cache-friendly L1-resident table ceiling on this
class of perfect-hash.

## Recommendations, ranked

### 1. Switch the 5-card kernel to Cactus-Kev (estimated 2-4 hours, 3-4×)

This is the highest-ROI in-tree change. Steps:

1. Add `KEV_CARDS: [u32; 52]`, `HASH_ADJUST`, `HASH_VALUES`, `FLUSHES`,
   `UNIQUE5` arrays to `phe-omaha` (verbatim from
   `~/ghq/github.com/b-inary/holdem-hand-evaluator/scripts/src/kev/arrays.rs`).
2. Implement `eval_5cards_kev(c1..c5: u32) -> u16` (the 4-line kernel
   above).
3. Replace the per-card RANK_BASES sum + `evaluate_rank_only_from_key`
   chain in path 1 / path 2 / path 3 with `eval_5cards_kev` over the
   same 60 (or fewer in path 2) combinations. Cactus-Kev handles flush
   vs non-flush in one call, so the 3-path dispatch can collapse into
   a single `evaluate_inner_kev` with a flat 60-combo loop.
4. Add `KEV_TO_PACKED: [u16; 7463]` (~15 KB) to convert Cactus-Kev's
   "smaller = stronger" convention (1=Royal SF, 7462=worst HighCard)
   to our `(cat << 12) | idx` packed format. **Convert once at the end
   of each Omaha eval** (after taking the min over 60 Kev ranks), not
   per-combo. Hot working set during the inner loop stays at ~33 KB
   (HASH_ADJUST + HASH_VALUES + UNIQUE5 + KEV_CARDS).
5. Keep all existing tests; the conversion table guarantees identical
   packed-u16 output.

Expected throughput: matching Nerdmaster Go (37 ns/eval ≈ **27 M/s**),
i.e. ~3.5× our current 7 M/s, ~7× the original 5 M/s baseline.

### 2. PHEvaluator quinary hash (estimated 1-2 weeks, 5-10×)

Bigger lift but the only path to PLO4 < 30 ns/eval. Outline:

1. Generator (one-shot): enumerate all valid `(hole_count[13],
   board_count[13])` pairs with hole-sum=4 and board-sum=5. ~10⁵-10⁶
   entries. For each, compute the best Omaha 5-card hand.
2. Compactify into a perfect hash. PHEvaluator uses lex-sort of the
   13-digit base-5 quinary representation; the index into the sorted
   array IS the hash key.
3. Runtime: per Omaha call, build the 13-digit quinary from cards
   (cheap: one `RANK_BASES`-style sum but with base 5), look up.
4. Flush is handled separately as today.

### 3. Smaller wins (not implemented this session)

- **HighCard fast path** for boards satisfying
  `board_has_no_pair && board_no_straight` plus hole with no pocket
  pair and no hole-board rank match. Under those, the answer is "top 2
  hole + top 3 board" and reduces to 1 lookup.
  **Measured frequency: 0.27% of random hands** (1M sample), saving
  ~140 ns each ⇒ ~0.4 ns/eval average. Far below noise; not worth the
  detection-cost overhead on the other 99.7% of calls. Discarded.
- **`evaluate_rank_only_from_key` SIMD** — gather two pair_rk +
  triple_rk pairs in parallel via AVX2 `_mm256_i32gather_epi32`. The
  L2-bound LOOKUP serializes most of the gather, so projected gain
  is small (single-digit %).
- **Reorder hole pairs by structural priority** without sorting (fixed
  iteration order based on cheap hole-card features, prune via
  per-pair upper bound). Tried previously; the per-pair classifier
  cost exceeded the prune savings on random hands. Could be worth
  retrying in **path 3 only** (where category upper bounds vary).

## Reproducing the bench numbers

Each commit has a row in `bench/history.csv`. To re-bench at a
specific commit:

```sh
git checkout <commit>
cargo run --release -p phe-omaha --example exhaustive   # appends one row
```

Variance per machine is ~10% on a quiet system; run 3-10× and look at
the median, not a single number.

## Cross-reference with b-inary's article (`yabaitechtokyo_vol6_holdem_chapter.md`)

The Hold'em-evaluator we depend on is the *same* implementation
described in section 4.7 of b-inary's article (Hold'em high, 7-card
direct eval, 145 KB tables). The article's measured performance:

| Implementation              | Sequential 1.33億   | Random 1億       | Tables  |
|-----------------------------|---------------------|------------------|---------|
| 4.1 naive 5-card brute      | 58.0 s (2.31 M/s)   | 50.0 s (2.0 M/s) | minimal |
| 4.2 naive 7-card direct     | 2.77 s (48 M/s)     | 3.14 s (32 M/s)  | minimal |
| 4.3 hash-table              | 9.11 s (15 M/s)     | 33.0 s (3.0 M/s) | 5.5 GB  |
| 4.4 vec + comb-number       | (n/a)               | 3.86 s (26 M/s)  | 256 MB  |
| 4.5 Two-Plus-Two            | 0.06 s (2.2 G/s)    | 6.91 s (15 M/s)  | 124 MB  |
| 4.6 Cactus-Kev (5-card)     | 10.3 s (13 M/s)     | 10.2 s (9.8 M/s) | 48 KB   |
| **4.7 b-inary final (ours)**| **0.16 s (819 M/s)**| **0.41 s (244 M/s)** | **145 KB** |

Key takeaways relevant to Omaha:

1. **The 4.5 vs 4.6 vs 4.7 contrast nails the cache argument.** Two-Plus-
   Two has the same ~5-10 ns sequential ceiling as 4.7 but its 124 MB
   tables miss cache catastrophically on random access (15 M/s vs the
   244 M/s 4.7 hits). Cactus-Kev's 48 KB tables solve cache but its
   prime-product hash is more expensive per probe than 4.7's
   single-displacement hash. **4.7's win is "smallest tables that
   still preserve a cheap hash."**

2. **Our Hold'em high single-eval is at the 4.7 ceiling** (~4 ns).
   No room to optimise the inner kernel; speedups have to come from
   *fewer kernel calls*.

3. **The article does not extend 4.7 to 9 cards.** That's the gap the
   user wants to close. A 9-card analogue of 4.7 needs:
   - A new rank-base set with unique sums for **9-card** multisets.
     (Quick check this session: `RANK_BASES` from the 5-7 set has
     **219 collisions** across the 270,270 9-card multisets, so the
     existing bases can't be reused as-is.)
   - A bigger LOOKUP table (precise size depends on the bases'
     `MAX_RANK_KEY`, but order-of-magnitude ~1-10 MB).
   - A flush-detection scheme that survives the count-up to 9 cards
     in one suit (the existing `0x3333 + n` nibble-counter trick
     handles 7 cards because `3+7=10=0xa` still fits a nibble; with 9
     cards `3+9=12=0xc` also fits, so the same `0x8888` flush-mask
     bit-3 trick still works — good news).

4. For **Omaha specifically** (not generic 9-card "best 5 of 9"), the
   single-lookup target requires baking the **"exactly 2 hole + 3
   board"** constraint into the table. PHEvaluator's quinary hash
   (separate hole/board base-5 digits) is the right shape; a direct
   "best 5 of 9" lookup gives the wrong answer for hands where the
   global max-of-9 violates the must-use-2 rule (e.g. royal flush all
   on board with no matching hole hearts).

## Empirical: Cactus-Kev kernel switch tested, **net loss**

Implemented the Cactus-Kev kernel switch (recommendation #1) as a
side-by-side path (`evaluate_kev` in `crates/omaha/src/lib.rs`,
backed by `crates/omaha/src/kev.rs` + `kev_tables.rs` lifted verbatim
from upstream). Equivalence: 100,000 random hands plus 11 structural
corner cases all match the production `OmahaHighRule::evaluate`
output. So the implementation is correct.

Bench (criterion `--quick`, 10K random fixtures, same machine):

| variant   | total      | ns/eval | vs optimized |
|-----------|-----------:|--------:|-------------:|
| optimized | 1.37 ms    | 137     | —            |
| naive     | 1.95 ms    | 195     | +43% slower  |
| **kev**   | **2.80 ms**| **280** | **+105% slower** |

**Why**: turns out the cache argument was the wrong frame for Omaha.
Per-combo instruction count is what dominates here, not table size.

| step           | optimized path 1                              | kev kernel |
|----------------|-----------------------------------------------|------------|
| pre-compute    | 4-card hole rk + 5-card board rk              | none       |
| per combo arith | 1 `wrapping_add` (`pair_rk + triple_rk`)      | 4 OR + 4 AND + 1 shift + 5 mask |
| per combo lookups | 2 chained (`OFFSETS` then `LOOKUP`)          | 1-3 (FLUSHES / UNIQUE5 / HASH_VALUES + find_fast hash) |

Our optimized path 1 reduces each of 60 combos to ~3 instructions
(`add` + 2 lookups) plus pre-summed partials hoisted out of the inner.
The Cactus-Kev kernel does ~10 instructions per combo just to compute
`q` and the flush check, before any lookup. Even when its tables fit
L1d, it ends up doing 2-3× the per-combo work.

Lesson: **a smaller table is only a win if the kernel is at least as
cheap as the larger-table kernel.** The b-inary 4.7 perfect-hash
keeps the lookup chain to a single `OFFSETS + LOOKUP` indirection per
hand, which beats Cactus-Kev's prime-product hash even with the
larger 145 KB table partly spilling to L2.

### Decomposing the Kev slowdown

To pin down which part of the Kev kernel costs the most, four
variants were added and bench'd side-by-side. Per-combo numbers (=
total / 60 / 10000-fixture batch size):

| variant       | per combo | what it does                                         |
|---------------|----------:|------------------------------------------------------|
| optimized     | 1.4 ns    | 1 `wrapping_add` + chained `OFFSETS` → `LOOKUP`       |
| naive         | 3.0 ns    | optimized but with `Hand` build per combo            |
| **kev_v2**    | **1.1 ns**| just `OR | AND-check | FLUSHES[q]` (always-flush)    |
| kev_v3        | 2.4 ns    | prime mul + `find_fast` + `HASH_VALUES` (no branches) |
| kev_v1        | 4.5 ns    | full kernel with pre-summed pair/triple partials      |
| kev (full)    | 5.0 ns    | full kernel, no precomp                               |

Three **load-bearing observations** fall out:

**1. The Kev *kernel itself* is not the problem.** `kev_v2` at
1.1 ns/combo is *faster* than optimized's 1.4 ns/combo — Cactus-Kev's
flush-only path (4 ORs + 4 ANDs in parallel + 1 single-step lookup
into 16 KB `FLUSHES`) actually beats our chained `OFFSETS → LOOKUP`
indirection. Modern CPUs absorb the parallel-arithmetic cost via
out-of-order execution, while the perfect-hash chain is
latency-limited (each load waits for the previous to retire).

**2. The non-flush `HASH_VALUES` path is ~70% slower than perfect-hash
LOOKUP.** `kev_v3` (skip both branches; always compute prime + run
`find_fast` + `HASH_VALUES`) costs 2.4 ns/combo vs optimized's
1.4 ns/combo. The chain `mask × mask × mask × mask × mask →
find_fast (5-step bit-mix) → HASH_VALUES[index]` is genuinely longer
than `RANK_BASES sum → OFFSETS → LOOKUP`. **This** is what makes
4.7's perfect-hash design beat Cactus-Kev's 1977-vintage prime hash
on identical hardware.

**3. Branches cost ~1.5-2 ns/combo on random Omaha hands.** Going
from `kev_v3` (no branches, 2.4 ns) to full `kev` (2 branches, 5.0
ns) adds 2.6 ns/combo. That's the **branch-mispredict tax** for two
data-dependent ifs inside the inner loop:

  - `is_flush > 0`: ~5-10% true on random combos. Predictor learns
    "almost always false" but mispredicts on the rare flush.
  - `unique5[q] != 0`: ~20% true (no-pair non-flush 5-card hands).
    Pattern is data-driven (depends on the rank-bit OR pattern q),
    so the predictor *cannot* learn it.

A modern x86 mispredict costs ~10-20 cycles. With ~60 evals × ~25%
mispredict-able branches × ~15 cycles = ~225 cycles ≈ 75 ns wasted
per Omaha call. Empirically the branch cost is ~156 ns (kev_v3 →
kev_full delta × 60), so each branch averages to a couple cycles of
wasted work — consistent with the prediction model.

**Implications**:

- A Cactus-Kev *flush-only* inner could replace path 2's current
  `LOOKUP_FLUSH` direct path; `kev_v2`'s 1.1 ns/combo is faster than
  whatever path 2 currently does. But path 2 is only 20% of random
  hands, so the full-pipeline win is small (single-digit % overall).
  Probably not worth the code split.
- The non-flush path is fundamentally cheaper with the b-inary 4.7
  perfect-hash than with Cactus-Kev's prime hash. Switching kernels
  is a strict regression for the 80% of hands that go non-flush.
- If you wanted Cactus-Kev to win, you'd need either branch-free 5-
  card eval (impossible while keeping correctness — the algorithm
  fundamentally branches on suit / rank duplicate structure) or a
  workload where `unique5` is predictable (not random Omaha).

This is consistent with the article's own measurements (section 4.6
vs 4.7): Cactus-Kev random-access is 9.8 M/s, b-inary 4.7 is
244 M/s — a 25× gap on **identical hardware**, dominated by kernel
instruction count, not by cache.

The `kev` module stays in the tree because:
- It's a clean, tested 5-card kernel that may be useful for other
  variants (5-card draw / 2-7 lowball / Razz, where the eval is on a
  fixed 5-card hand and per-call instruction count of ~10 ops is
  acceptable).
- The `kev_rank_to_packed` conversion is the bridge if anyone wants
  to call it from existing packed-u16 code.
- The 100K random-hand cross-check is a useful regression gate.

## Updated take on the user's target

- "**Hold'em vs Omaha: 6× → 3× slowdown**" calibration:
  - Hold'em direct eval: ~4 ns (article 4.7) ↔ our local measurement
    ~10 ns including loop overhead.
  - "6× Hold'em" ≈ 24-60 ns; "3× Hold'em" ≈ 12-30 ns.
  - **Current Omaha: ~140 ns ≈ 35× Hold'em direct eval.**
- Reaching 3× requires either:
  - **PHEvaluator quinary hash** (1 lookup → ~10-15 ns with flush
    handling). 1-2 weeks to implement, ~30 MB tables (need to verify
    fits cache).
  - **9-card direct eval** with Omaha must-use baked in (analogue of
    4.7 for Omaha). Same budget as PHEvaluator effectively.

The Cactus-Kev kernel switch (recommendation #1 above) gets us **~3-4×
to ~37 ns/eval = ~9× Hold'em**, which is *better* than 6× but still
short of 3×. It's the right intermediate step.

## Final summary of this session

What changed in `main`:

| commit  | change                                                              | ns/eval |
|---------|---------------------------------------------------------------------|--------:|
| 4767ffc | (rebased baseline; bench infrastructure + 60-combo brute force)     | 195     |
| dbf6532 | board-partial cache + flush-impossible fast path                    | 171     |
| 891ad07 | flush-dominates fast path (no-pair board)                           | 160     |
| 9a16dd1 | + `board_no_straight` predicate (no eval change)                    | 161     |
| cb041bc | + `upper_bound_category` helper (no eval change)                    | 159     |
| f7861c5 | path 2 bypasses `Hand` + uses `LOOKUP_FLUSH` directly               | 158     |
| 8c457b5 | path 1 bypasses `Hand`, direct `RANK_BASES` sum                     | 147     |
| 2e88c26 | path 3 bypasses `Hand`, per-combo dispatch                          | 143     |
| 57cdca5 | (Cactus-Kev kernel implemented, **kept as utility** but slower)     | 143     |

Net: **195 → 143 ns/eval** (~1.36× speedup) over the rebased history.
On absolute terms still ~14× our Hold'em high single-eval, vs the
user's 3× target.

What was tried and discarded:

- **Branch-and-bound prune** (with sort): ~43% slowdown. Overhead of
  upper-bound + sort + `board_no_straight` recompute > savings.
- **`#[inline(always)]` on `evaluate_rank_only_from_key`**: ~6%
  slowdown. ICache pressure from inlining 60 copies of the lookup
  chain.
- **Cactus-Kev kernel switch**: ~2× slowdown. Per-combo arithmetic
  cost dominates the L1-fit table benefit.
- **HighCard direct compute**: 0.27% applicable, saving ~0.4 ns/eval
  average. Below noise.

What's left for the user:

1. **9-card direct eval** (analogue of b-inary's article §4.7,
   extended to 9 cards with the Omaha must-use-2-hole constraint
   baked into the perfect-hash table). The article doesn't cover
   this and `RANK_BASES` has 219 collisions over the 270,270 9-card
   multisets, so the rank bases need to be regenerated. PHEvaluator
   does this with a quinary base-5 hash; an in-tree implementation
   following the §4.7 pattern is plausible at ~1-2 weeks of work.
   This is the only path to the 3× Hold'em target.
2. **OMPEval suit isomorphism** for *equity loops* (many evals per
   board). 2-3× in solver hot loops; nothing for single-eval bench.
   Would integrate at the solver level (`poker-cuda-solver`), not in
   `phe-omaha`.

Useful primitives left in tree for downstream:

- `phe_omaha::eval_5cards_kev` + `kev_rank_to_packed` — Cactus-Kev
  5-card kernel with conversion to our packed format. Useful for
  fixed-5-card variants (5-card draw, 2-7 lowball single-draw, Razz)
  where 60-combo amortisation isn't relevant.
- `phe_omaha::upper_bound_category` — per-hole-pair max-category
  bound, public for solver-side custom prune logic.
- `phe_omaha::flush_suit`, `flush_possible`, `board_has_no_pair`,
  `board_no_straight` — structural predicates exposed for downstream.

## Straight-only structural short-circuit (user proposal)

**Question**: precompute, per 3-rank board subset, the 2-rank hole
combos that complete a straight (e.g. `{4,5,6} → [{2,3}, {3,7}, {7,8}]`).
Use the table to detect a straight cheaply before running the
60-combo loop.

**Implementation** (`evaluate_straight_short_circuit` in `crates/omaha/
src/lib.rs`): replaces the 286-key precomputed table with a more
compact 13-bit-mask AND/popcount sweep over the 9 standard 5-rank
windows + the wheel. Same information density, fewer indirections.

Short-circuit fires when:
  - no flush is reachable, AND
  - the board has no pair, AND
  - some 5-rank window has ≥3 board ranks + 2 specific hole ranks
    that fill the missing positions

Under those, Straight is the maximum reachable category (Flush is
gated out by no-flush-eligible; FH / Quads need a board pair). The
short-circuit returns `(4 << 12) | (top - 3)` directly — one packed
u16 with no LOOKUP touched.

**Bench (10K random fixtures, criterion `--quick`)**:

| variant                  | total ms | ns/eval | vs production |
|--------------------------|---------:|--------:|--------------:|
| optimized (production)   | 1.36     | 136     | —             |
| **straight_short_circuit** | **1.46** | **146** | **+10 ns slower** |

**Frequency**: short-circuit fires on **5.72%** of random fixtures.

**Why it's a net loss**: the structural-detection overhead (~25 ns:
suit counts, rank masks, board-pair check, 9-window straight sweep)
is paid on **every hand**. The 60-combo skip saves ~140-180 ns but
only on the ~5.7% of hands that actually hit the fast path.
Expected:

  +25 ns overhead × 100% − 140 ns savings × 5.72% ≈ +17 ns net

The measured +10 ns is consistent with the model (the overhead
estimate was slightly high; LLVM hoists / vectorises some of the
predicate compute).

**Could it become a net win?** If we move the detection *into* path 1
(only run the masks + window sweep when path 1 + no-pair-board is
already chosen by dispatch), the overhead is paid only on ~33% of
hands instead of 100%, dropping the cost to ~8 ns. Savings stay at
~10 ns. Net **+2 ns at best**, which is in the bench-noise floor on
this machine — not worth the code-path split.

**Generalisable lesson** (matches the earlier B&B-prune attempt and
the HighCard fast path): on random Omaha, single-category structural
short-circuits don't pay because their applicability rate (1-6%) is
below the breakeven where overhead × all_hands > savings × hit_rate.
The path-2 (flush-dominates) optimisation works *only* because it
fires on 20% of hands AND the savings per hit are large (~80 ns).

## §4.6 article cross-validation

The article's §4.6 (Cactus-Kev / Senzee perfect-hash 5-card kernel
applied to 7-card via the 21-sub-hand `C(7,5)` enumeration):

  - Sequential 1.33 億 hands : 10.3 s = ~77 ns/7-card hand
  - Random 1 億 hands        : 10.2 s = ~102 ns/7-card hand
  - Tables                   : 48 KB (flushes 16 KB + unique5 16 KB +
                               hash_values 16 KB + hash_adjust 1 KB)

Per-5-card eval (= 7-card / 21):
  - Sequential : ~3.7 ns / 5-card eval
  - Random     : ~4.9 ns / 5-card eval

Our Omaha implementation (60 sub-evals): observed ~5.0 ns/combo (full
kernel), ~4.5 ns/combo (with pre-summed pair/triple partials).
**Matches the article's per-5-card numbers within noise** — the
Rust port is at the same kernel cost as the upstream C++ reference.

Things the article spells out that match our root-cause analysis:

1. **§4.6 is slower than §4.2** (naive 7-card direct eval, 32 ns/hand
   random) by ~3×. The article attributes this to "the 5-card kernel
   times 21 sub-evals exceeds the cost of one 7-card direct eval
   with bit ops". For Omaha at 60 sub-evals the same calculus
   applies — Cactus-Kev × 60 is strictly worse than any 9-card-aware
   direct eval would be.

2. **`find_fast` is 6 ops + 1 `hash_adjust` lookup + 1 final XOR** —
   we use the same code verbatim. Senzee's bit-mix is good enough
   to map all 4,888 unique non-flush prime products into a 13-bit
   range with no collisions, but it's *not* free; the 5-step
   compute chain shows up in the bench as ~70% slower per-combo
   than §4.7's Single-displacement hash.

3. **§4.6 → §4.7 leap = 25× faster random eval**. The article
   identifies the two wins explicitly:
   (a) Single-displacement + First-fit-decreasing perfect hash is
       cheaper to compute than Senzee's prime-product + bit-mix.
   (b) Direct 7-card eval skips the 21-sub-eval loop entirely.
   For our 9-card Omaha problem, win (a) is on the table (replace
   Cactus-Kev's `find_fast` with §4.7-style `OFFSETS + LOOKUP`); win
   (b) requires a 9-card-aware direct evaluator that bakes the
   must-use-2-hole rule into the perfect-hash table generation.
   See "9-card §4.7 analogue" below.

## §4.4 (combinatorial number system) extended to 9 cards: feasibility

A natural question after reading b-inary's article: can we just apply
**§4.4** — sort the cards, compute a combinatorial-number-system
index, look up directly in a precomputed array — to 9 cards?

The math works out unfavorably. Comparison with the article's 7-card
numbers:

| target                                | configs                        | u16 table | per-lookup (random)        |
|---------------------------------------|--------------------------------|----------:|----------------------------|
| §4.4 7-card (article)                 | `C(52, 7) = 133,784,560`       | 256 MB    | 38.6 ns (DRAM-bound)        |
| §4.7 7-card (article — what we use)   | n/a (perfect hash)             | 145 KB    | 4.1 ns (L1d-fit)            |
| **§4.4 9-card "best 5 of 9" generic** | `C(52, 9) = 3,679,075,400`     | **~7.4 GB** | ~50-100 ns (DRAM/SSD-bound) |
| **§4.4 9-card Omaha must-use-2-hole** | `C(52, 4) × C(48, 5) = 463 B`  | **~926 GB** | infeasible                 |
| PHEvaluator quinary (rank-only + flush) | ~15 M                         | **~30 MB** | ~15-20 ns (L3-fit)          |

Three things go wrong with the direct §4.4 extension:

**1. Configuration count grows ~30× from 7 → 9 cards.** `C(52, 7)` is
133M; `C(52, 9)` is 3.68 G. At u16 per entry that's 7.4 GB even for
the "best 5 of 9" generic case. Modern desktops can fit it in RAM,
but it spills out of every level of cache.

**2. Encoding the Omaha must-use-2-hole constraint blows the table
up by ~125×.** Two different (hole, board) splits over the *same* 9
cards can produce different best-5 answers (e.g. royal-flush-all-on-
board with no hearts in hole is unplayable in Omaha but plays as a
royal in "best 5 of 9"). So the table can't be keyed on the 9-card
set alone — it has to encode the (hole, board) split, ballooning to
`C(52, 4) × C(48, 5) ≈ 463 B` entries (~926 GB). Impractical.

**3. Even the 7.4 GB generic version performs worse than what we
already have.** Article §4.4's 256 MB / 38.6 ns ratio is dominated by
DRAM latency, not the index computation. A 7.4 GB table is at least
as DRAM-bound and probably more so (paging, larger working set).
**Per-Omaha throughput**:

  - Current (60 chained perfect-hash lookups, mostly L2):
    ~143 ns/eval.
  - §4.4 9-card (1 DRAM lookup):
    ~50-100 ns/eval = ~1.5-3× speedup. Marginal vs the cost of
    generating + shipping a 7 GB asset, and **doesn't satisfy Omaha
    must-use** so each Omaha call would still need a fall-back path
    for ~5-10% of hands.

**The smarter table layout** is what PHEvaluator does:

- Drop suits except for flush detection: encode the 9-card hand as
  13 rank-counts (each 0..4). Suits go to a separate ~16 KB
  flush-rank table.
- Encode the (hole, board) split in the same key by using two
  separate per-rank counts (e.g. base-5 digit pairs). Only valid
  configurations get table entries.
- Total valid configurations: ~15 million → ~30 MB at u16/entry.
  Fits L3 on modern x86 (8-16 MB *each* core, more shared).
- Per-lookup latency: ~15-20 ns at L3, possibly 5-8 ns if a hot
  subset stays in L2.
- Per-Omaha at 1 lookup + flush check: **~15-25 ns/eval** = 5-10×
  speedup vs current. Hits the user's "3× Hold'em" target.

Bottom line: **§4.4 doesn't extend cleanly to 9-card** (too big,
DRAM-bound, doesn't satisfy must-use). The right shape is the §4.7
perfect-hash *with rank-counts as the key and suits factored out*
— that's PHEvaluator. The §4.4 → 9-card path *is* implementable but
is strictly dominated by PHEvaluator on every metric (size, latency,
correctness), so don't bother.

## References

- Cactus-Kev original: <http://suffe.cool/poker/evaluator.html>
- Senzee's perfect-hash adaptation: <http://senzee.blogspot.com>
- HenryRLee/PokerHandEvaluator (PLO4 quinary hash):
  <https://github.com/HenryRLee/PokerHandEvaluator>
  - Algorithm doc:
    <https://github.com/HenryRLee/PokerHandEvaluator/blob/master/Documentation/Algorithm.md>
- Nerdmaster/poker (Go, tightest brute-force):
  <https://github.com/Nerdmaster/poker>
- zekyll/OMPEval (Hold'em, suit-iso + cached state):
  <https://github.com/zekyll/OMPEval>
- cardrank/cardrank (Go multi-variant): <https://github.com/cardrank/cardrank>
- diditforlulz273/PokerRL-Omaha (Python; readable algorithm doc):
  <https://github.com/diditforlulz273/PokerRL-Omaha>
- Local clone of upstream Cactus-Kev tables (already on this machine):
  `~/ghq/github.com/b-inary/holdem-hand-evaluator/scripts/src/kev/arrays.rs`
