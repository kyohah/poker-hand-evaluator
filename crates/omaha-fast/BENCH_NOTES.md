# `phe-omaha-fast` performance notes

Direct comparison against the upstream HenryRLee/PokerHandEvaluator C
reference, run on the **same host** (no cross-host noise).

## Numbers

10 000 random PLO4 hands, deterministic xorshift64 seed
`0xDEAD_BEEF_CAFE_BABE`. Measured 2026-05-01.

| build                                  | speed (ns / eval) |
|----------------------------------------|-------------------|
| Rust `phe-omaha-fast` (this crate)     | **~37 ns**        |
| C MSVC `cl /O2 /GL /LTCG` (LTO on)     | 45.8 – 47.3 ns    |
| C MSVC `cl /O2` only                   | 52 – 62 ns        |
| HenryRLee published (their host, gcc)  | 30.5 ns           |

The Rust port is **~20% faster than the C reference compiled with MSVC
on this host**. Both implementations execute the identical algorithm
(verbatim port + bit-exact correctness on 1000+ hands and on the Royal
SF spot-check), so the gap is compiler / codegen, not algorithmic.

The 30.5 ns published by HenryRLee was on a different machine
(2.6 GHz Linux box, gcc / clang). On this host (Windows 11 + a
laptop-class CPU + MSVC) we cannot reproduce that absolute number from
their C build, but our Rust LLVM port lands close to where a
`clang-cl` build would.

## How to reproduce the C number

```sh
# In ~/ghq/github.com/HenryRLee/PokerHandEvaluator
# (1) Save the snippet below as bench_plo4.c
# (2) From a "x64 Native Tools Command Prompt for VS 2022":

mkdir obj
cl /O2 /GL /nologo /I cpp\include bench_plo4.c \
  cpp\src\evaluator_plo4.c cpp\src\dptables.c cpp\src\tables_bitwise.c \
  cpp\src\tables_plo4.c cpp\src\hash.c cpp\src\hashtable.c \
  cpp\src\rank.c cpp\src\7462.c \
  /link /LTCG /OUT:bench_plo4.exe

bench_plo4.exe   # prints `PLO4 C reference: XX.XX ns/eval ...`
```

```c
// bench_plo4.c
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
    printf("PLO4 C reference: %.2f ns/eval over %lld evals (sink=%d, %.3fs total)\n",
           sec * 1e9 / ((double)N * (double)NF),
           (long long)N * (long long)NF, sink, sec);
    return 0;
}
```

## Implication

The "13× slower than HenryRLee" worry that motivated this port turned
out to be unfounded once we measured on the same host. With the
algorithm faithfully ported and Rust's LLVM backend, we end up
**faster than the upstream C** out of the box. No further hot-path
hand-tuning is needed to remain competitive — the next meaningful
improvement would be a cache-friendlier table layout (phase 2 in the
roadmap), not micro-optimisation of the existing kernel.
