//! GPU vs CPU bit-exact parity for `evaluate_plo4_*`.
//!
//! Run with:
//! ```text
//! cargo test -p phe-omaha-fast --release --features cuda -- --ignored
//! ```
//!
//! The `#[ignore]` is so unit-test runs without a GPU (e.g., CI's CPU
//! lane) don't fail. Whenever the kernel or table layout changes,
//! re-run this manually.

#![cfg(feature = "cuda")]

use phe_omaha_fast::cuda::PloEvalContext;
use phe_omaha_fast::evaluate_plo4_cards;

fn deal_hands(seed: u64, n: usize) -> Vec<([u8; 4], [u8; 5])> {
    let mut s = seed;
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        let mut deck = [0u8; 52];
        for i in 0..52 {
            deck[i] = i as u8;
        }
        for i in 0..9 {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            let p = i + (s as usize) % (52 - i);
            deck.swap(i, p);
        }
        out.push((
            [deck[0], deck[1], deck[2], deck[3]],
            [deck[4], deck[5], deck[6], deck[7], deck[8]],
        ));
    }
    out
}

#[test]
#[ignore = "requires CUDA-capable GPU at runtime"]
fn cuda_evaluate_batch_matches_cpu_1000() {
    let ctx = PloEvalContext::new().expect("CUDA context init");
    let hands = deal_hands(0xCAFE_BABE_DEAD_BEEF, 1000);

    let gpu_out = ctx.evaluate_batch(&hands).expect("kernel launch");

    let mut mismatches = 0;
    for (i, (hole, board)) in hands.iter().enumerate() {
        let cpu = evaluate_plo4_cards(
            board[0] as i32,
            board[1] as i32,
            board[2] as i32,
            board[3] as i32,
            board[4] as i32,
            hole[0] as i32,
            hole[1] as i32,
            hole[2] as i32,
            hole[3] as i32,
        );
        if cpu != gpu_out[i] {
            if mismatches < 5 {
                eprintln!(
                    "mismatch at {i}: cpu={cpu} gpu={} hand={:?}",
                    gpu_out[i], hands[i]
                );
            }
            mismatches += 1;
        }
    }
    assert_eq!(mismatches, 0, "{} CPU/GPU mismatches", mismatches);
}

#[test]
#[ignore = "requires CUDA-capable GPU at runtime"]
fn cuda_solver_integration_path() {
    // Mimics how `poker-cuda-solver` would integrate:
    // 1. Caller owns the CudaContext (singleton).
    // 2. Caller owns the stream and uploads its own buffers.
    // 3. PloEvalContext shares the context via from_context().
    // 4. Launch on caller's stream via evaluate_batch_on_stream().
    use cudarc::driver::CudaContext;

    let ctx = CudaContext::new(0).expect("CudaContext");
    let stream = ctx.default_stream();
    let plo = PloEvalContext::from_context(ctx.clone()).expect("from_context");

    let hands = deal_hands(0xBEEF_FACE_C0FF_EE55, 1024);
    let n = hands.len();
    let mut holes_flat = Vec::with_capacity(n * 4);
    let mut boards_flat = Vec::with_capacity(n * 5);
    for (h, b) in &hands {
        holes_flat.extend_from_slice(h);
        boards_flat.extend_from_slice(b);
    }
    let d_holes = stream.clone_htod(&holes_flat).unwrap();
    let d_boards = stream.clone_htod(&boards_flat).unwrap();
    let mut d_out = unsafe { stream.alloc::<i32>(n) }.unwrap();

    plo.evaluate_batch_on_stream(&stream, &d_holes, &d_boards, &mut d_out, n)
        .expect("launch");
    stream.synchronize().expect("sync");

    let gpu_out = stream.clone_dtoh(&d_out).unwrap();

    for (i, (hole, board)) in hands.iter().enumerate() {
        let cpu = evaluate_plo4_cards(
            board[0] as i32, board[1] as i32, board[2] as i32, board[3] as i32, board[4] as i32,
            hole[0] as i32, hole[1] as i32, hole[2] as i32, hole[3] as i32,
        );
        assert_eq!(cpu, gpu_out[i], "mismatch at {i}");
    }
}

#[test]
#[ignore = "requires CUDA-capable GPU at runtime"]
fn cuda_evaluate_batch_handles_uneven_grid() {
    // Test sizes that aren't multiples of BLOCK_DIM (256).
    let ctx = PloEvalContext::new().expect("CUDA context init");
    for &n in &[1usize, 7, 255, 256, 257, 511, 512, 1023, 1024, 5000] {
        let hands = deal_hands(0x1111_2222_3333_4444 + n as u64, n);
        let gpu_out = ctx.evaluate_batch(&hands).expect("kernel launch");
        for (i, (hole, board)) in hands.iter().enumerate() {
            let cpu = evaluate_plo4_cards(
                board[0] as i32, board[1] as i32, board[2] as i32, board[3] as i32, board[4] as i32,
                hole[0] as i32, hole[1] as i32, hole[2] as i32, hole[3] as i32,
            );
            assert_eq!(cpu, gpu_out[i], "n={n} i={i}");
        }
    }
}
