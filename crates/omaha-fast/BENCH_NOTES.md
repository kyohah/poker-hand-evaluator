# `phe-omaha-fast` performance notes

Direct comparison against the upstream HenryRLee/PokerHandEvaluator C
reference, run on the **same Windows host** (no cross-host noise).

## Numbers

10 000 random PLO4 hands, deterministic xorshift64 seed
`0xDEAD_BEEF_CAFE_BABE`. Measured 2026-05-01.

| build                                  | speed (ns / eval) |
|----------------------------------------|-------------------|
| **Rust `phe-omaha-fast` (LLVM)**       | **~35 ns** (best 34.9) |
| **C clang-cl `/O2 -flto -fuse-ld=lld`**| **~35 ns** (best 34.7) |
| C MSVC `cl /O2 /GL /LTCG`              | ~46 ns            |
| C MSVC `cl /O2` (no LTO)               | ~52 – 62 ns       |
| HenryRLee published (their host, gcc)  | 30.5 ns           |

When both are compiled with LLVM at `-O2 +LTO`, the Rust port and the
C reference are **at parity on this host** (within ~0.2 ns,
indistinguishable from noise). The 30.5 ns HenryRLee published was on
a different machine; we cannot reproduce that absolute number here,
but Rust matches what an LLVM-built C does.

## Why MSVC is slower

Cross-translation-unit inlining: with `cl /O2` alone, MSVC compiles
each `.c` file separately and the linker doesn't inline across
boundaries, so every `evaluate_plo4_cards` call is a real function
call. `/GL /LTCG` enables whole-program optimisation, which recovers
~6 ns but still trails LLVM by ~10 ns on this kernel.

## How to reproduce

### Rust

```sh
cd ~/ghq/github.com/kyohah/poker-hand-evaluator
cargo bench -p phe-omaha-fast --bench eval -- --quick
```

### C (clang-cl + LTO)

```sh
# Install LLVM if needed:
scoop install llvm        # or: winget install LLVM.LLVM

# In ~/ghq/github.com/HenryRLee/PokerHandEvaluator:
# (1) Save bench_plo4.c (snippet below)
# (2) From an x64 Native Tools Command Prompt:

mkdir obj
"C:\Users\kyoha\scoop\apps\llvm\current\bin\clang-cl.exe" \
    /O2 -flto -fuse-ld=lld /nologo /I cpp\include \
    bench_plo4.c \
    cpp\src\evaluator_plo4.c cpp\src\dptables.c cpp\src\tables_bitwise.c \
    cpp\src\tables_plo4.c cpp\src\hash.c cpp\src\hashtable.c \
    cpp\src\rank.c cpp\src\7462.c \
    /Fe:bench_clang.exe /Fo:obj\

bench_clang.exe   # prints `PLO4 C: XX.XX ns/eval ...`
```

### C (MSVC LTCG, for comparison)

```sh
mkdir obj
cl /O2 /GL /nologo /I cpp\include bench_plo4.c \
   cpp\src\evaluator_plo4.c cpp\src\dptables.c cpp\src\tables_bitwise.c \
   cpp\src\tables_plo4.c cpp\src\hash.c cpp\src\hashtable.c \
   cpp\src\rank.c cpp\src\7462.c \
   /link /LTCG /OUT:bench_msvc.exe
```

```c
// bench_plo4.c — same fixture-generation as our criterion bench
// so the two numbers are directly comparable.
#include <stdio.h>
#include <stdint.h>
#include <windows.h>
#include "phevaluator/phevaluator.h"

static uint64_t xs(uint64_t* s) {
    uint64_t x = *s;
    x ^= x << 13; x ^= x >> 7; x ^= x << 17;
    *s = x;
    return x;
}

#define NF 10000
static int holes[NF][4], boards[NF][5];

int main(void) {
    uint64_t state = 0xDEADBEEFCAFEBABEULL;
    for (int i = 0; i < NF; ++i) {
        int deck[52];
        for (int j = 0; j < 52; ++j) deck[j] = j;
        for (int j = 0; j < 9; ++j) {
            int p = j + (int)(xs(&state) % (uint64_t)(52 - j));
            int t = deck[j]; deck[j] = deck[p]; deck[p] = t;
        }
        for (int j = 0; j < 4; ++j) holes[i][j] = deck[j];
        for (int j = 0; j < 5; ++j) boards[i][j] = deck[4 + j];
    }
    LARGE_INTEGER freq, t0, t1;
    QueryPerformanceFrequency(&freq);
    int sink = 0;
    for (int it = 0; it < 10; ++it)        // warmup
        for (int i = 0; i < NF; ++i)
            sink ^= evaluate_plo4_cards(
                boards[i][0], boards[i][1], boards[i][2], boards[i][3], boards[i][4],
                holes[i][0], holes[i][1], holes[i][2], holes[i][3]);
    int N = 1000;
    QueryPerformanceCounter(&t0);
    for (int it = 0; it < N; ++it)
        for (int i = 0; i < NF; ++i)
            sink ^= evaluate_plo4_cards(
                boards[i][0], boards[i][1], boards[i][2], boards[i][3], boards[i][4],
                holes[i][0], holes[i][1], holes[i][2], holes[i][3]);
    QueryPerformanceCounter(&t1);
    double sec = (double)(t1.QuadPart - t0.QuadPart) / (double)freq.QuadPart;
    printf("PLO4 C: %.2f ns/eval (%lld evals, sink=%d, %.3fs)\n",
           sec * 1e9 / ((double)N * (double)NF),
           (long long)N * (long long)NF, sink, sec);
    return 0;
}
```

## Implication

`phe-omaha-fast` is at parity with `clang-cl`-built C on this host.
The "13× slower than HenryRLee" worry that motivated this port was
eliminated entirely once we measured on the same host with the same
LLVM backend — both implementations land at ~35 ns / eval, and the
gap to HenryRLee's published 30.5 ns is microarchitectural (CPU clock
and L3 hit-rate on a different machine), not algorithmic or
language-level.

The next meaningful improvement is a cache-friendlier table layout
(phase 2 in the omaha-fast roadmap), not micro-optimisation of the
existing kernel — both LLVM C and LLVM Rust have already squeezed
this algorithm dry.

## Negative results recorded here so we don't retry them

### Packed bitmap for suit counting (batch path, 2026-05-01)

Tried replacing the 9 scattered `suit_count_*[c & 3] += 1` /
`suit_binary_*[c & 3] |= BIT_OF_DIV_4[c]` writes with a single OR of
9 `1u64 << (16 * suit + rank)` values — same idea as
`phe-core::Hand` for 7-card hold'em. Per suit, the count is
`popcount` of the 16-bit window and the rank-bitmap falls out for
free as the same window's low 13 bits.

Even with tree-reduced ORs, this regressed batch by ~5 ns / hand
(3.15 → 4.20 ms / 100K). Why: the OR chain has a 5-cycle dependency
chain (5 cards) that the CPU can't break, while the scattered store
form lets store-buffer parallelism issue 9 stores out-of-order. The
wider issue width wins on Skylake-class hosts.

If revisiting on a microarchitecture with narrower stores or wider
ORs, re-bench from the original `evaluate_with_noflush_idx` form
before switching.
