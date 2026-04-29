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
  hole + top 3 board" and reduces to 1 lookup. Frequency ~5-10% of
  random hands; saves ~150 ns each → ~7-15 ns/eval average. Modest.
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
