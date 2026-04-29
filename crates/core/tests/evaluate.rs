//! Integration test for `evaluate_via_lookup`.
//!
//! We don't have real lookup tables in `phe-core` (those ship in the
//! variant assets crates). To pin down the dispatch contract — flush
//! branch vs. rank branch, perfect-hash offset arithmetic — we build a
//! synthetic table just large enough to exercise both branches.

use phe_core::{evaluate_via_lookup, Hand, FLUSH_MASK, OFFSETS, OFFSET_SHIFT};

/// For a non-flush 5-card hand, the dispatcher must:
///   1. take rank_key = hand.key as u32 as usize
///   2. read OFFSETS[rank_key >> OFFSET_SHIFT] as the perfect-hash offset
///   3. return lookup[rank_key + offset]
#[test]
fn rank_branch_uses_perfect_hash_offset() {
    // Five distinct ranks → no flush bit set in the suit nibble.
    // Cards 0,4,8,12,16 are 2c,3c,4c,5c,6c → all clubs → THIS IS a flush, skip.
    // Use 0,5,10,15,20 → 2c,3d,4h,5s,6c → mixed suits.
    let h = Hand::from_slice(&[0, 5, 10, 15, 20]);
    assert_eq!(
        h.get_key() & FLUSH_MASK,
        0,
        "this fixture must not be flagged as flush"
    );

    let rank_key = h.get_key() as u32 as usize;
    let bucket = rank_key >> OFFSET_SHIFT;
    let offset = OFFSETS[bucket] as usize;
    let hash_idx = rank_key.wrapping_add(offset);

    // Build a sparse lookup that returns 0xBEEF at the expected slot.
    let mut lookup = vec![0u16; hash_idx + 1];
    lookup[hash_idx] = 0xBEEF;
    let lookup_flush = vec![0u16; 1]; // unused on this branch

    let r = evaluate_via_lookup(&h, &lookup, &lookup_flush);
    assert_eq!(r, 0xBEEF);
}

/// For a 5-card flush, the dispatcher must:
///   1. detect the flush via key & FLUSH_MASK
///   2. compute flush_key = (mask >> (4 * leading_zeros(flush_bit))) as u16
///   3. return lookup_flush[flush_key]
#[test]
fn flush_branch_uses_lookup_flush() {
    // 5 clubs: 0,4,8,12,16 = 2c,3c,4c,5c,6c
    let h = Hand::from_slice(&[0, 4, 8, 12, 16]);
    let is_flush = h.get_key() & FLUSH_MASK;
    assert!(is_flush > 0, "this fixture must be detected as a flush");

    let flush_key = (h.get_mask() >> (4 * is_flush.leading_zeros())) as u16;

    let lookup = vec![0u16; 1]; // unused on this branch
    let mut lookup_flush = vec![0u16; flush_key as usize + 1];
    lookup_flush[flush_key as usize] = 0xCAFE;

    let r = evaluate_via_lookup(&h, &lookup, &lookup_flush);
    assert_eq!(r, 0xCAFE);
}
