//! Cross-check: `evaluate_batch` must produce identical outputs to
//! a single-call loop for the same inputs.
//!
//! `evaluate_batch` reorders work into two phases (dispatch + key
//! stash, then prefetched path-1 lookup loop) for better memory-latency
//! overlap. The reordering is purely a perf optimization — every
//! `out[i]` must match `OmahaHighRule::evaluate(&inputs[i].0,
//! &inputs[i].1)`.

use phe_omaha::OmahaHighRule;

/// PCG-style RNG (same constants as the bench).
struct Rng(u64);
impl Rng {
    fn next_u64(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0
    }
}

fn random_fixtures(seed: u64, n: usize) -> Vec<([usize; 4], [usize; 5])> {
    let mut rng = Rng(seed);
    let mut fixtures = Vec::with_capacity(n);
    for _ in 0..n {
        let mut deck: [usize; 52] = std::array::from_fn(|i| i);
        for i in 0..9 {
            let j = i + (rng.next_u64() as usize) % (52 - i);
            deck.swap(i, j);
        }
        fixtures.push((
            [deck[0], deck[1], deck[2], deck[3]],
            [deck[4], deck[5], deck[6], deck[7], deck[8]],
        ));
    }
    fixtures
}

#[test]
fn batch_matches_single_call_on_10k_random() {
    let fixtures = random_fixtures(0xBAD_C0FFEE_BEEF, 10_000);

    let single: Vec<u16> = fixtures
        .iter()
        .map(|(h, b)| OmahaHighRule::evaluate(h, b))
        .collect();

    let mut batch = vec![0u16; fixtures.len()];
    OmahaHighRule::evaluate_batch(&fixtures, &mut batch);

    for (i, (s, b)) in single.iter().zip(batch.iter()).enumerate() {
        assert_eq!(
            s, b,
            "batch/single mismatch at index {}: hole={:?} board={:?} single={} batch={}",
            i, fixtures[i].0, fixtures[i].1, s, b
        );
    }
}

#[test]
fn batch_handles_short_input_below_prefetch_distance() {
    // Smaller than PREFETCH_AHEAD: the batch impl must skip the
    // prefetch path and still produce correct outputs.
    let fixtures = random_fixtures(0x1234_5678, 3);

    let single: Vec<u16> = fixtures
        .iter()
        .map(|(h, b)| OmahaHighRule::evaluate(h, b))
        .collect();

    let mut batch = vec![0u16; fixtures.len()];
    OmahaHighRule::evaluate_batch(&fixtures, &mut batch);

    assert_eq!(single, batch);
}

#[test]
fn batch_empty_inputs_is_no_op() {
    let mut out: Vec<u16> = Vec::new();
    OmahaHighRule::evaluate_batch(&[], &mut out);
    assert!(out.is_empty());
}
