//! GPU vs CPU bit-exact parity for `HoldemEvalContext::evaluate_*`.
//!
//! Run with:
//! ```text
//! cargo test -p phe-holdem --release --features cuda -- --ignored
//! ```

#![cfg(feature = "cuda")]

use phe_core::Hand;
use phe_holdem::cuda::HoldemEvalContext;
use phe_holdem::HighRule;

fn deal_hands(seed: u64, n: usize, cards_per_hand: usize) -> Vec<u8> {
    let mut s = seed;
    let mut out = Vec::with_capacity(n * cards_per_hand);
    for _ in 0..n {
        let mut deck: [u8; 52] = [0; 52];
        for i in 0..52 {
            deck[i] = i as u8;
        }
        for i in 0..cards_per_hand {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            let p = i + (s as usize) % (52 - i);
            deck.swap(i, p);
        }
        for i in 0..cards_per_hand {
            out.push(deck[i]);
        }
    }
    out
}

fn cpu_eval(cards: &[u8]) -> u16 {
    let mut h = Hand::new();
    for &c in cards {
        h = h.add_card(c as usize);
    }
    HighRule::evaluate(&h)
}

#[test]
#[ignore = "requires CUDA-capable GPU at runtime"]
fn cuda_evaluate_batch_5card_matches_cpu() {
    let ctx = HoldemEvalContext::new().expect("CUDA init");
    let cards = deal_hands(0xCAFE_F00D_5555, 1000, 5);
    let gpu_out = ctx.evaluate_batch(&cards, 5).expect("kernel");

    let mut mismatches = 0;
    for i in 0..1000 {
        let cpu = cpu_eval(&cards[i * 5..(i + 1) * 5]);
        if cpu != gpu_out[i] {
            if mismatches < 5 {
                eprintln!("5-card mismatch at {i}: cpu={cpu} gpu={}", gpu_out[i]);
            }
            mismatches += 1;
        }
    }
    assert_eq!(mismatches, 0);
}

#[test]
#[ignore = "requires CUDA-capable GPU at runtime"]
fn cuda_evaluate_batch_6card_matches_cpu() {
    let ctx = HoldemEvalContext::new().expect("CUDA init");
    let cards = deal_hands(0xCAFE_F00D_6666, 1000, 6);
    let gpu_out = ctx.evaluate_batch(&cards, 6).expect("kernel");

    for i in 0..1000 {
        let cpu = cpu_eval(&cards[i * 6..(i + 1) * 6]);
        assert_eq!(cpu, gpu_out[i], "6-card mismatch at {i}");
    }
}

#[test]
#[ignore = "requires CUDA-capable GPU at runtime"]
fn cuda_evaluate_batch_7card_matches_cpu() {
    let ctx = HoldemEvalContext::new().expect("CUDA init");
    let cards = deal_hands(0xCAFE_F00D_7777, 1000, 7);
    let gpu_out = ctx.evaluate_batch(&cards, 7).expect("kernel");

    for i in 0..1000 {
        let cpu = cpu_eval(&cards[i * 7..(i + 1) * 7]);
        assert_eq!(cpu, gpu_out[i], "7-card mismatch at {i}");
    }
}

#[test]
#[ignore = "requires CUDA-capable GPU at runtime"]
fn cuda_uneven_grid_sizes() {
    let ctx = HoldemEvalContext::new().expect("CUDA init");
    for &n in &[1usize, 7, 255, 256, 257, 511, 512, 1023, 1024, 5000] {
        let cards = deal_hands(0xBEEF + n as u64, n, 7);
        let gpu_out = ctx.evaluate_batch(&cards, 7).expect("kernel");
        for i in 0..n {
            let cpu = cpu_eval(&cards[i * 7..(i + 1) * 7]);
            assert_eq!(cpu, gpu_out[i], "n={n} i={i}");
        }
    }
}

#[test]
#[ignore = "requires CUDA-capable GPU at runtime"]
fn cuda_solver_integration_path() {
    // Mirrors phe-omaha-fast's solver-integration test:
    // shared CudaContext + caller stream + caller buffers.
    use cudarc::driver::CudaContext;

    let ctx = CudaContext::new(0).expect("CudaContext");
    let stream = ctx.default_stream();
    let holdem = HoldemEvalContext::from_context(ctx.clone()).expect("from_context");

    let n = 1024usize;
    let cards = deal_hands(0xBABA_FACE_4242, n, 7);
    let d_cards = stream.clone_htod(&cards).unwrap();
    let mut d_out = unsafe { stream.alloc::<u16>(n) }.unwrap();

    holdem
        .evaluate_batch_on_stream(&stream, &d_cards, &mut d_out, n, 7)
        .expect("launch");
    stream.synchronize().expect("sync");

    let gpu_out = stream.clone_dtoh(&d_out).unwrap();
    for i in 0..n {
        let cpu = cpu_eval(&cards[i * 7..(i + 1) * 7]);
        assert_eq!(cpu, gpu_out[i], "i={i}");
    }
}

#[test]
#[ignore = "requires CUDA-capable GPU at runtime"]
fn cuda_invalid_cards_per_hand_rejected() {
    let ctx = HoldemEvalContext::new().expect("CUDA init");
    // 4-card hand is invalid for the LOOKUP scheme (needs 5-7).
    let cards = vec![0u8; 4];
    let r = ctx.evaluate_batch(&cards, 4);
    assert!(r.is_err(), "4-card eval must error");
}
