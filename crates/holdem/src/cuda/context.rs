//! Host-side `HoldemEvalContext` — owns the CUDA context, compiled
//! kernel module, and device-resident lookup tables.

use std::sync::Arc;

use cudarc::driver::{
    CudaContext, CudaFunction, CudaSlice, CudaStream, DriverError, LaunchConfig, PushKernelArg,
};
use cudarc::nvrtc::{compile_ptx_with_opts, CompileError, CompileOptions};

use phe_core::{CARDS, OFFSETS};
use phe_holdem_assets::{LOOKUP, LOOKUP_FLUSH};

use super::kernel::KERNEL_SRC;

const KERNEL_NAME: &str = "evaluate_holdem_batch_kernel";
const BLOCK_DIM: u32 = 256;

/// Errors produced by the CUDA backend.
#[derive(Debug, thiserror::Error)]
pub enum CudaEvalError {
    /// Failure from the CUDA driver itself. Wraps `cudarc`'s
    /// `DriverError`.
    #[error("CUDA driver error: {0}")]
    Driver(#[from] DriverError),
    /// Failure to compile `KERNEL_SRC` via NVRTC.
    #[error("NVRTC compile error: {0}")]
    Compile(#[from] CompileError),
    /// Input slice length didn't match `n * cards_per_hand`.
    #[error("input length: got {got} bytes, need {need} ({n} hands × {cards_per_hand} cards)")]
    InputLength {
        /// Actual input length supplied.
        got: usize,
        /// Required length (`n * cards_per_hand`).
        need: usize,
        /// Number of hands the caller asked to evaluate.
        n: usize,
        /// Cards per hand passed to the launch (must be 5, 6, or 7).
        cards_per_hand: u32,
    },
    /// Output buffer too small to hold one rank per input hand.
    #[error("output buffer too small: got {got}, need {need}")]
    OutputTooSmall {
        /// Length of the supplied output buffer.
        got: usize,
        /// Required length (one entry per input hand).
        need: usize,
    },
    /// `cards_per_hand` was outside the supported `5..=7` range.
    #[error("cards_per_hand must be 5, 6, or 7; got {0}")]
    InvalidCardsPerHand(u32),
}

/// GPU-resident Hold'em (5-7 card) evaluator.
///
/// Compiling the kernel and uploading the ~212 KB of LOOKUP /
/// LOOKUP_FLUSH / OFFSETS / CARDS tables takes ~1 s the first time
/// the context is created. Subsequent `evaluate_batch*` calls reuse
/// the device buffers; the tables stay L2-resident on the GPU
/// through normal access patterns, since their working set is much
/// smaller than typical L2 (~3-40 MB).
///
/// # Solver integration
///
/// Construct via [`HoldemEvalContext::from_context`] passing the
/// caller's existing `Arc<CudaContext>`, and launch with
/// [`HoldemEvalContext::evaluate_batch_on_stream`] passing the
/// solver's per-pass stream — same pattern as
/// `phe-omaha::cuda::PloEvalContext`.
///
/// Output is u16 with the workspace's "higher = stronger"
/// convention (matches `HandRule::evaluate`), so no Cactus-Kev
/// inversion step is needed when the solver feeds these into a
/// showdown comparison.
pub struct HoldemEvalContext {
    _ctx: Arc<CudaContext>,
    /// Stream used for the one-time table upload only. The
    /// `_on_stream` API takes a caller-supplied stream; the legacy
    /// `_device` API continues to use this stream.
    stream: Arc<CudaStream>,
    kernel: CudaFunction,
    d_card_keys: CudaSlice<u64>,
    d_card_masks: CudaSlice<u64>,
    d_offsets: CudaSlice<i32>,
    d_lookup: CudaSlice<u16>,
    d_lookup_flush: CudaSlice<u16>,
}

impl HoldemEvalContext {
    /// Initialise on device 0 with a fresh CudaContext. Convenient
    /// for stand-alone CPU-side callers; solver integration should
    /// prefer [`HoldemEvalContext::from_context`].
    pub fn new() -> Result<Self, CudaEvalError> {
        Self::with_device(0)
    }

    /// Initialise on the CUDA device with the given ordinal,
    /// constructing a fresh `CudaContext`. Use
    /// [`HoldemEvalContext::from_context`] instead when integrating
    /// into a host that already owns one.
    pub fn with_device(ordinal: usize) -> Result<Self, CudaEvalError> {
        Self::from_context(CudaContext::new(ordinal)?)
    }

    /// Construct from a caller-owned `Arc<CudaContext>`. The
    /// context is shared (cloned `Arc`); table uploads happen on
    /// the context's default stream and are synchronized before
    /// returning.
    pub fn from_context(ctx: Arc<CudaContext>) -> Result<Self, CudaEvalError> {
        let stream = ctx.default_stream();

        let ptx = compile_ptx_with_opts(
            KERNEL_SRC,
            CompileOptions {
                options: vec!["--fmad=false".into()],
                ..Default::default()
            },
        )?;
        let module = ctx.load_module(ptx)?;
        let kernel = module.load_function(KERNEL_NAME)?;

        // Split phe-core's CARDS = [(u64 key, u64 mask); 52] into
        // two device buffers (the kernel takes them separately to
        // avoid struct-padding ambiguity in NVRTC C).
        let card_keys: Vec<u64> = CARDS.iter().map(|&(k, _)| k).collect();
        let card_masks: Vec<u64> = CARDS.iter().map(|&(_, m)| m).collect();
        let offsets_vec: Vec<i32> = OFFSETS.to_vec();
        let lookup_vec: Vec<u16> = LOOKUP.to_vec();
        let lookup_flush_vec: Vec<u16> = LOOKUP_FLUSH.to_vec();

        let d_card_keys = stream.clone_htod(&card_keys)?;
        let d_card_masks = stream.clone_htod(&card_masks)?;
        let d_offsets = stream.clone_htod(&offsets_vec)?;
        let d_lookup = stream.clone_htod(&lookup_vec)?;
        let d_lookup_flush = stream.clone_htod(&lookup_flush_vec)?;

        stream.synchronize()?;

        Ok(Self {
            _ctx: ctx,
            stream,
            kernel,
            d_card_keys,
            d_card_masks,
            d_offsets,
            d_lookup,
            d_lookup_flush,
        })
    }

    /// Borrow the underlying default stream.
    pub fn default_stream(&self) -> &Arc<CudaStream> {
        &self.stream
    }

    /// Evaluate a batch of 5/6/7-card hands held on the host.
    /// Uploads `cards`, runs the kernel, downloads strengths.
    /// Synchronizes the default stream before returning.
    ///
    /// `cards` is `n * cards_per_hand` bytes laid out contiguously.
    pub fn evaluate_batch(
        &self,
        cards: &[u8],
        cards_per_hand: u32,
    ) -> Result<Vec<u16>, CudaEvalError> {
        Self::check_cards_per_hand(cards_per_hand)?;
        let n = cards.len() / cards_per_hand as usize;
        if n * cards_per_hand as usize != cards.len() {
            return Err(CudaEvalError::InputLength {
                got: cards.len(),
                need: n * cards_per_hand as usize,
                n,
                cards_per_hand,
            });
        }

        let d_cards = self.stream.clone_htod(cards)?;
        let mut d_out = unsafe { self.stream.alloc::<u16>(n) }?;

        self.launch_on(&self.stream, &d_cards, &mut d_out, n, cards_per_hand)?;
        self.stream.synchronize()?;

        let out = self.stream.clone_dtoh(&d_out)?;
        Ok(out)
    }

    /// Evaluate a batch where the caller already holds device
    /// buffers. Launches on the context's default stream. Does
    /// **not** synchronize — the caller is responsible.
    ///
    /// For solver integration where you want to launch on your own
    /// stream (e.g., for CUDA graph capture), prefer
    /// [`evaluate_batch_on_stream`](Self::evaluate_batch_on_stream).
    pub fn evaluate_batch_device(
        &self,
        d_cards: &CudaSlice<u8>,
        d_out: &mut CudaSlice<u16>,
        n: usize,
        cards_per_hand: u32,
    ) -> Result<(), CudaEvalError> {
        self.evaluate_batch_on_stream(&self.stream, d_cards, d_out, n, cards_per_hand)
    }

    /// Evaluate on a caller-supplied stream. Buffers must already
    /// live on the device the stream belongs to. The launch is
    /// asynchronous and capturable into a CUDA graph.
    ///
    /// This is the primary entry point for solver integration:
    /// `poker-cuda-solver`-style callers should pass their per-pass
    /// stream so the eval kernel orders correctly with surrounding
    /// CFR forward / backward / showdown kernels.
    pub fn evaluate_batch_on_stream(
        &self,
        stream: &CudaStream,
        d_cards: &CudaSlice<u8>,
        d_out: &mut CudaSlice<u16>,
        n: usize,
        cards_per_hand: u32,
    ) -> Result<(), CudaEvalError> {
        Self::check_cards_per_hand(cards_per_hand)?;
        let need_cards = n * cards_per_hand as usize;
        if d_cards.len() < need_cards {
            return Err(CudaEvalError::InputLength {
                got: d_cards.len(),
                need: need_cards,
                n,
                cards_per_hand,
            });
        }
        if d_out.len() < n {
            return Err(CudaEvalError::OutputTooSmall {
                got: d_out.len(),
                need: n,
            });
        }
        self.launch_on(stream, d_cards, d_out, n, cards_per_hand)
    }

    fn check_cards_per_hand(c: u32) -> Result<(), CudaEvalError> {
        match c {
            5 | 6 | 7 => Ok(()),
            other => Err(CudaEvalError::InvalidCardsPerHand(other)),
        }
    }

    fn launch_on(
        &self,
        stream: &CudaStream,
        d_cards: &CudaSlice<u8>,
        d_out: &mut CudaSlice<u16>,
        n: usize,
        cards_per_hand: u32,
    ) -> Result<(), CudaEvalError> {
        let n_u32 = n as u32;
        let grid = (n_u32 + BLOCK_DIM - 1) / BLOCK_DIM;
        let cfg = LaunchConfig {
            grid_dim: (grid, 1, 1),
            block_dim: (BLOCK_DIM, 1, 1),
            shared_mem_bytes: 0,
        };

        let mut b = stream.launch_builder(&self.kernel);
        b.arg(&n_u32);
        b.arg(&cards_per_hand);
        b.arg(d_cards);
        b.arg(d_out);
        b.arg(&self.d_card_keys);
        b.arg(&self.d_card_masks);
        b.arg(&self.d_offsets);
        b.arg(&self.d_lookup);
        b.arg(&self.d_lookup_flush);
        unsafe { b.launch(cfg) }?;
        Ok(())
    }
}
