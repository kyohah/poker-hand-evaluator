# PLO4 CPU 最適化技法の適用可能性分析

**日付**: 2026-05-01
**動機**: `phe-omaha-fast` → `phe-omaha` 統合後、HenryRLee/PokerHandEvaluator
の C++ 実装と同一ホスト LLVM ビルドで **PLO4 が parity (~35 ns/eval)** に
ある。CPU side で更に削れるか、b-inary の Hold'em 高速化技法を流用できるかを
評価する。
**参考資料**: yabaitech.tokyo vol.6 「テキサスホールデムの役判定に見る
高速化テクニック」 (著者: ばいなり)。Hold'em で 145 KB を L2 cache に
収めて random access **2.4 億 eval/s** を達成した実績の解説。

## 結論サマリ

| | 期待効果 | 実装コスト | 推奨度 |
|---|---|---|---|
| **A. Sort-by-index batch** (本書の主推奨) | DRAM hit → L2/L3 hit、equity-table 系で 1.5-3× | 中 | ⭐⭐⭐ |
| **B. Suit padding bit-trick** (`scb >= 3 && sch >= 2` を 1 AND 化) | ~2-3 ns/hand | 小 | ⭐⭐ |
| **C. SIMD quinary increments** (`_mm_add_epi64` 系) | ~3-5 ns/hand | 中 | ⭐⭐ |
| **D. Algorithmic 圧縮** (22 MB → L3 fit) | DRAM floor 突破 | 研究レベル | ⭐ |
| **E. AVX2 8-wide hash_quinary** (試行済) | 0 (scalar early-exit に tie) | — | ❌ |

CPU side の総 ceiling は **~5-8 ns/hand** (現 35 → 28-30 ns)。
**DRAM floor (~30 ns NOFLUSH lookup)** は CPU 最適化では破れない。
algorithmic 圧縮なしに HenryRLee に「明確に勝つ」ことはできない。

## 背景: なぜ Hold'em 流の最適化が PLO4 に効きにくいか

### Hold'em の勝因 (b-inary 145 KB)

phe-holdem は 5/6/7-card eval を **multiset の関数** として扱える:
`sum(RANK_BASES[c])` 一つの key で perfect-hash → 1 LOOKUP 読みで答え。
key 空間 (49,205 multiset for 7-card) を 145 KB に圧縮でき **L2 fit**。
DRAM へ降りないので、CPU 工夫 (`__m128i` add_card / OMPEval bases /
single-displacement) すべてが効く。

### PLO4 の構造的違い

PLO4 のルール「2 from hole + 3 from board」のため、**best-of-60 は 9-card
multiset の関数ではない**。同じ 9-card rank multiset でも (hole, board) の
partition が違えば答えが違う。例:

| Hand | rank multiset | best 2+3 |
|---|---|---|
| hole=`AAKQ`, board=`AKJ T 2` | (A:3, K:2, Q:1, J:1, T:1, 2:1) | three aces |
| hole=`KKQJ`, board=`AAAA T` | (A:4, K:2, Q:1, J:1, T:1) | full house |

→ key には **(board_quinary, hole_quinary) の組** が必要。

| | エントリ数 | バイト数 |
|---|---|---|
| board_quinary (`hash_quinary` 5 cards, ≤4/rank) | 6,175 | — |
| hole_quinary (`hash_quinary` 4 cards, ≤4/rank) | 1,820 | — |
| 表 (`board_hash * 1820 + hole_hash`) | 11,238,500 | × 13 bit = **18.3 MB** |

これが PLO4 NOFLUSH の **情報理論的下限**。実装は u16 で 22.5 MB、4 MB 余裕あり。
どんなアルゴリズムも (board_q, hole_q) を区別する限りこの 18 MB を割れない。

L3 cache:
| CPU | L3 |
|---|---|
| Intel Skylake-X / 3rd-gen Xeon | 8-16 MB |
| AMD Ryzen 9 5950X | 64 MB (3D V-cache: 96 MB+) |
| Apple M3 | 16 MB |

→ **大半のホストで 18 MB は L3 を超える**。ランダム access では DRAM bound (~30 ns/hit)。

## 各技法の評価

### 1. Cactus-Kev 32-bit card encoding `xxxAKQJT 98765432 cdhsrrrr xxpppppp`

**Hold'em**: 5-card 用、48 KB perfect hash で完結。

**PLO4 適用**: HenryRLee PLO4 が `BIT_OF_DIV_4[52] u16` で実質同等の役割を
実現済 (per-card の rank-bit、suit は `c & 3` で分離)。**新規価値なし**。

### 2. `__m128i` union + `_mm_add_epi64` で incremental `add_card`

**Hold'em**: sequential 探索 (133M hand 辞書順走査) で incremental に hash
state を保持 → 1 命令で `add_card`。0.16 s で 133M hand。

**PLO4 適用**: PLO4 は random access 用途しかない (1 hand 9 枚を一気に評価)。
incremental の利益は薄い。ただし **batch path で 9-card hash 構築を SIMD 化**
する余地はある (#3 参照)。**間接的に C で活きる**。

### 3. Suit カウンタ +N padding + `& 0x8888...` で flush 判定

**Hold'em**: 4-bit suit カウンタに +3 を初期値、accumulate 後 `& 0x88880000`
で「5 枚以上ある suit」を 1 AND で検出。

**PLO4 適用**: ✅ **適用できる**。PLO4 の flush 判定は
`scb >= 3 AND sch >= 2` (board 3 枚以上 AND hole 2 枚以上の同 suit)。

```rust
// 提案実装 (擬似コード)
const INIT_BOARD: u16 = 0x5555;  // 各 4-bit slot = 5 (5+3=8 で bit3 立つ)
const INIT_HOLE:  u16 = 0x6666;  // 各 4-bit slot = 6 (6+2=8 で bit3 立つ)

let mut scb_packed: u16 = INIT_BOARD;
let mut sch_packed: u16 = INIT_HOLE;
// 9 枚分: scb_packed += SUIT_INC[c]; sch_packed += SUIT_INC[c]; (board/hole 別)
//   SUIT_INC[c] = 1 << (4 * (c & 3))

let flush_board = scb_packed & 0x8888;
let flush_hole  = sch_packed & 0x8888;
let both = flush_board & flush_hole;
if both != 0 {
    let suit = both.trailing_zeros() / 4;
    // flush path
}
```

→ 現状の 4 iter loop が 1 AND + branch に置換。**~2-3 ns/hand 削減**。

**実装注意点**:
- `SUIT_INC[52]` テーブルは 52 × 2 = 104 byte で L1 楽勝
- branch predictor 的にも 1 branch (flush 入る or 入らない) で frigndly
- AVX2/AVX-512 不要、scalar `u16` 操作のみで OK (cross-platform で効く)

### 4. SIMD `_mm_add_epi64` で quinary increments の並列化

**Hold'em**: card-encoded `__m128i` を 1 命令で merge。

**PLO4 適用**: △ batch path の `noflush_index_scalar` で:

```rust
// 現状: scalar 9 increments
*quinary_board.get_unchecked_mut((c1 >> 2) as usize) += 1;
*quinary_board.get_unchecked_mut((c2 >> 2) as usize) += 1;
// ... 5 board + 4 hole
```

これを 13-byte quinary に対し SIMD で:

```rust
// 提案: 事前計算 CARD_QUINARY[52] = u8x16 (rank slot に 1、他 0)
let mut acc_board = u8x16::splat(0);
acc_board += CARD_QUINARY[c1 as usize];
// ... 5 board cards
// hole も同様
```

`u8x16` (128-bit) で 13 byte slot を carry なしで add。
**~3-5 ns/hand 削減**期待 (scalar の 5+4=9 increments → SIMD の 5+4=9 adds、
ただし memory store/load が消える)。

実装注意点:
- `wide` crate or `std::simd` (nightly) で portable
- AVX2 ある host なら u8x16 が無料、ない host でも SSE2 は baseline
- 現存の `phe-omaha-fast` AVX2 8-wide hash_quinary 実験 (BENCH_NOTES.md の
  negative result) とは別物 — あれは「8 hand 並列に hash_quinary」、これは
  「1 hand 内で quinary increment を SIMD 化」

### 5. OMPEval 非自明な rank 基底 (25-bit 圧縮)

**Hold'em**: 7-card rank key を 25 bit に圧縮、49K LOOKUP を密に詰める。

**PLO4 適用**: ✗ PLO4 は既に `(board_hash, hole_hash) → 0..6175 × 0..1820`
で密 (gap なし)。OMPEval-style 圧縮は **することがない**。

### 6. Single-displacement perfect hash `offset[k/t] + (k%t)`

**Hold'em**: sparse な 33M key 空間を 49K LOOKUP に圧縮。First-fit-decreasing
でビルド。

**PLO4 適用**: ✗ 同上。address は既に密。displacement で詰めるべき gap がない。

### 7. L2 fit (145 KB)

**Hold'em**: 145 KB を L2 (256 KB-1 MB) に収めて random access 最速。

**PLO4 適用**: ✗ **不可能**。情報理論的下限 18 MB > L3。
これが PLO4 で b-inary 流の勝ち方ができない核心。

## 同一ホスト勝ちの現実的経路

### A. Sort-by-index batch (主推奨)

DRAM floor を破る唯一の現実的手段。**equity-table** や **multi-board
enumerate** のように index 局所性が高い workload で:

1. Pass 1: `noflush_index_scalar` で全 hand の index を計算
2. **Sort indices** (radix sort, ~10-15 ns/hand)
3. Pass 2: index 昇順で NOFLUSH_PLO4[idx] を読む → 隣接 hand が同じ cache
   line を共有しやすい

→ 多くの hit が DRAM (30 ns) → L3 (10 ns) → L2 (3 ns) に降格。
sort cost を加算しても **net で 1.5-3× 速い** (workload 次第)。

random hand では index 一様分布なので効果弱い。**index 局所性のある
workload 限定**。

### B + C. Suit padding bit-trick + SIMD quinary increments

合計 **~5-8 ns/hand 削減** (35 → 28-30 ns)。

DRAM floor (30 ns) に到達して頭打ち。HenryRLee 公称 30.5 ns との差は
~0 ns、parity 維持。

**ただし**: solver context で **数百億 eval** (CFR の 100 iter × 数億 node)
規模なら 5 ns × 10^10 = **50 秒の時間節約**。実装する価値はある。

### D. Algorithmic 圧縮 (research-level)

NOFLUSH_PLO4 の値は rank ordering なので:
- 共通する `board_hash` 範囲内で hole_hash → rank の単調性を圧縮可能?
- partition by board pattern で per-board mini-table?
- delta encoding?

成功すれば 18 MB → < 8 MB で L3 fit、DRAM floor 突破。
論文レベルの研究、本リポジトリ scope 外。

### E. AVX2 8-wide hash_quinary (試行済、negative)

`phe-omaha-fast` (現 `phe-omaha`) で実験。AVX2 gather で 8 hand 並列に
hash_quinary を計算。SoA scatter histogram + 13-iter forced loop の
overhead が scalar early-exit と相殺。**0 ns 改善**。

詳細: `crates/omaha/BENCH_NOTES.md` の "AVX2 8-wide pass-1" negative result。

## 実装優先順位の提案

solver consumer (poker-cuda-solver) を主想定すると:

1. **B (suit padding)** — 数百行、低リスク、~2-3 ns gain。**先にやる**
2. **C (SIMD quinary)** — 数百行、中リスク (SIMD 不利なホスト要 fallback)、
   ~3-5 ns gain。**B の後**
3. **A (sort-by-index batch)** — 新 API 追加。equity-table workload で
   大きく効く。solver 統合タイミングで実装

合計 CPU 改善後の見積: **35 → 28-30 ns/hand single-call**。
HenryRLee と parity 維持、DRAM floor で頭打ち。

数百億 eval scale の solver では合計 50 秒 + α の節約、実装価値あり。

## 関連ドキュメント

- `crates/omaha/BENCH_NOTES.md` — PLO4 ベンチ詳細、AVX2 negative result、
  same-host C-vs-Rust parity、CUDA backend integration recipe
- yabaitech.tokyo vol.6 「テキサスホールデムの役判定に見る高速化テクニック」
  (ばいなり) — 本書の元ネタ、b-inary Hold'em 145 KB 設計の解説
