//! NVRTC-compiled CUDA kernel for Hold'em (5/6/7-card) evaluation.
//!
//! Uses the same `phe-core` perfect-hash scheme that the CPU path
//! does (`evaluate_via_lookup`):
//! * `key` and `mask` are accumulated by adding `CARDS[c].0` /
//!   `CARDS[c].1` per card, with `key` initialised to
//!   `0x3333 << 48` (so 4-of-a-suit overflows into the top nibble
//!   block, signalling a flush).
//! * On flush, the rank bits are extracted from `mask` shifted by
//!   the leading-zero count of the flush bit and looked up in
//!   `LOOKUP_FLUSH`.
//! * Otherwise the rank-key indexes `OFFSETS` to get a per-bucket
//!   offset, and `LOOKUP[rank_key + offset]` gives the strength.
//!
//! 1 thread = 1 hand. Output is u16 with the workspace's standard
//! "higher = stronger" convention (LOOKUP / LOOKUP_FLUSH already
//! store values that way).

pub const KERNEL_SRC: &str = r#"
// phe-core / phe-holdem perfect-hash constants.
#define SUIT_SHIFT      48
#define OFFSET_SHIFT    11
#define FLUSH_MASK_HI   0x8888ULL
#define INIT_KEY_HI     0x3333ULL

__device__ __forceinline__ unsigned short evaluate_hand_dev(
    const unsigned char* cards,
    int n,
    const unsigned long long* card_keys,
    const unsigned long long* card_masks,
    const int* offsets,
    const unsigned short* lookup,
    const unsigned short* lookup_flush)
{
    unsigned long long key  = INIT_KEY_HI << SUIT_SHIFT;
    unsigned long long mask = 0ULL;
    for (int i = 0; i < n; ++i) {
        unsigned int c = (unsigned int)cards[i];
        key  += card_keys[c];
        mask += card_masks[c];
    }
    unsigned long long is_flush = key & (FLUSH_MASK_HI << SUIT_SHIFT);
    if (is_flush != 0ULL) {
        int lz = __clzll(is_flush);
        unsigned short flush_key = (unsigned short)(mask >> (4 * lz));
        return lookup_flush[flush_key];
    }
    unsigned int rank_key = (unsigned int)key;
    int offset = offsets[rank_key >> OFFSET_SHIFT];
    unsigned int hash_key = rank_key + (unsigned int)offset;
    return lookup[hash_key];
}

// 1 thread = 1 hand. `cards_per_hand` is 5, 6, or 7; hands are
// laid out contiguously as `n * cards_per_hand` bytes.
extern "C" __global__ void evaluate_holdem_batch_kernel(
    unsigned int n,
    unsigned int cards_per_hand,
    const unsigned char* __restrict__ cards,
    unsigned short* __restrict__ out,
    const unsigned long long* __restrict__ card_keys,
    const unsigned long long* __restrict__ card_masks,
    const int* __restrict__ offsets,
    const unsigned short* __restrict__ lookup,
    const unsigned short* __restrict__ lookup_flush)
{
    unsigned int gid = blockIdx.x * blockDim.x + threadIdx.x;
    if (gid >= n) return;
    out[gid] = evaluate_hand_dev(
        &cards[gid * cards_per_hand],
        (int)cards_per_hand,
        card_keys, card_masks, offsets, lookup, lookup_flush);
}
"#;
