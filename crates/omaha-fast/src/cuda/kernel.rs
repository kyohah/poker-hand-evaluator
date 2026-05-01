//! NVRTC-compiled CUDA kernel for PLO4 evaluation.
//!
//! Direct port of `evaluate_plo4_cards` from
//! HenryRLee/PokerHandEvaluator (`cpp/src/evaluator_plo4.c`),
//! adapted for 1-thread-per-hand parallel execution. The kernel
//! takes device pointers to all 6 lookup tables plus the per-hand
//! input/output buffers; tables stay resident across calls.
//!
//! Implementation notes:
//!
//! * Table indexing matches the host-side flat layout:
//!   `DP[qi][offset][kk]` → `dp[qi*140 + offset*10 + kk]`
//!   `CHOOSE[n][k]` → `choose[n*10 + k]`
//! * `hash_quinary` keeps the early-exit form (the CPU port
//!   measured ~38 ns/hand vs ~45 ns for the branchless variant).
//!   On GPU each thread runs independently so divergence here is
//!   per-hand; warps will tail-end together at the slowest lane,
//!   acceptable since the 13-iter ceiling is small.
//! * `__fmad=false` is passed to NVRTC (matching `poker-cuda-solver`
//!   convention). PLO4 is integer-only so it doesn't matter for
//!   correctness, but staying consistent across the workspace's
//!   CUDA kernels avoids surprises.

pub const KERNEL_SRC: &str = r#"
// PLO4 evaluator — port of HenryRLee/PokerHandEvaluator
// `cpp/src/evaluator_plo4.c`. 1 thread = 1 hand.

__device__ __forceinline__ unsigned int hash_quinary_dev(
    const unsigned char* q,
    int k_init,
    const unsigned int* dp)
{
    unsigned int sum = 0;
    int k = k_init;
    #pragma unroll
    for (int i = 0; i < 13; ++i) {
        unsigned int qi = (unsigned int)q[i];
        // dp[qi][12-i][k]
        sum += dp[qi * 140 + (12 - i) * 10 + k];
        k -= (int)qi;
        if (k <= 0) break;
    }
    return sum;
}

__device__ __forceinline__ unsigned int hash_binary_dev(
    int binary,
    int k,
    const unsigned int* choose)
{
    unsigned int sum = 0;
    const int len = 15;
    for (int i = 0; i < len; ++i) {
        if ((binary & (1 << i)) != 0) {
            int n = len - i - 1;
            if (n >= k) {
                // CHOOSE[n][k]
                sum += choose[n * 10 + k];
            }
            k -= 1;
            if (k == 0) break;
        }
    }
    return sum;
}

extern "C" __global__ void evaluate_plo4_batch_kernel(
    unsigned int n,
    const unsigned char* __restrict__ holes,           // n * 4
    const unsigned char* __restrict__ boards,          // n * 5
    int* __restrict__ out,                             // n
    const unsigned int*   __restrict__ dp,             // [5*14*10]
    const unsigned int*   __restrict__ choose,         // [53*10]
    const unsigned short* __restrict__ bit_of_div4,    // [52]
    const unsigned short* __restrict__ flush_5card,    // [8192]
    const unsigned short* __restrict__ flush_plo4,     // FLUSH_PLO4
    const unsigned short* __restrict__ noflush_plo4)   // NOFLUSH_PLO4
{
    unsigned int gid = blockIdx.x * blockDim.x + threadIdx.x;
    if (gid >= n) return;

    int c1 = (int)boards[gid * 5 + 0];
    int c2 = (int)boards[gid * 5 + 1];
    int c3 = (int)boards[gid * 5 + 2];
    int c4 = (int)boards[gid * 5 + 3];
    int c5 = (int)boards[gid * 5 + 4];
    int h1 = (int)holes[gid * 4 + 0];
    int h2 = (int)holes[gid * 4 + 1];
    int h3 = (int)holes[gid * 4 + 2];
    int h4 = (int)holes[gid * 4 + 3];

    int value_flush = 10000;

    // Per-suit counts (board / hole)
    int scb[4] = {0, 0, 0, 0};
    int sch[4] = {0, 0, 0, 0};
    scb[c1 & 3]++; scb[c2 & 3]++; scb[c3 & 3]++; scb[c4 & 3]++; scb[c5 & 3]++;
    sch[h1 & 3]++; sch[h2 & 3]++; sch[h3 & 3]++; sch[h4 & 3]++;

    const int padding[3] = { 0x0000, 0x2000, 0x6000 };

    for (int i = 0; i < 4; ++i) {
        if (scb[i] >= 3 && sch[i] >= 2) {
            int sbb[4] = {0, 0, 0, 0};
            int sbh[4] = {0, 0, 0, 0};
            sbb[c1 & 3] |= (int)bit_of_div4[c1];
            sbb[c2 & 3] |= (int)bit_of_div4[c2];
            sbb[c3 & 3] |= (int)bit_of_div4[c3];
            sbb[c4 & 3] |= (int)bit_of_div4[c4];
            sbb[c5 & 3] |= (int)bit_of_div4[c5];
            sbh[h1 & 3] |= (int)bit_of_div4[h1];
            sbh[h2 & 3] |= (int)bit_of_div4[h2];
            sbh[h3 & 3] |= (int)bit_of_div4[h3];
            sbh[h4 & 3] |= (int)bit_of_div4[h4];
            int sb_b = sbb[i];
            int sb_h = sbh[i];
            if (scb[i] == 3 && sch[i] == 2) {
                value_flush = (int)flush_5card[sb_b | sb_h];
            } else {
                int board_padded = sb_b | padding[5 - scb[i]];
                int hole_padded  = sb_h | padding[4 - sch[i]];
                unsigned int board_hash = hash_binary_dev(board_padded, 5, choose);
                unsigned int hole_hash  = hash_binary_dev(hole_padded, 4, choose);
                value_flush = (int)flush_plo4[board_hash * 1365 + hole_hash];
            }
            break;
        }
    }

    // Quinary histograms (13 ranks)
    unsigned char qb[13] = {0,0,0,0,0,0,0,0,0,0,0,0,0};
    unsigned char qh[13] = {0,0,0,0,0,0,0,0,0,0,0,0,0};
    qb[c1 >> 2]++; qb[c2 >> 2]++; qb[c3 >> 2]++; qb[c4 >> 2]++; qb[c5 >> 2]++;
    qh[h1 >> 2]++; qh[h2 >> 2]++; qh[h3 >> 2]++; qh[h4 >> 2]++;

    unsigned int board_hash = hash_quinary_dev(qb, 5, dp);
    unsigned int hole_hash  = hash_quinary_dev(qh, 4, dp);
    int value_noflush = (int)noflush_plo4[board_hash * 1820 + hole_hash];

    out[gid] = value_flush < value_noflush ? value_flush : value_noflush;
}
"#;
