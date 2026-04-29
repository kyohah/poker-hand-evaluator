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
