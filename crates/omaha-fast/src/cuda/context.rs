//! Host-side `PloEvalContext` — owns the CUDA context, compiled
//! kernel module, and device-resident lookup tables.

use std::sync::Arc;

use cudarc::driver::{
    CudaContext, CudaFunction, CudaSlice, CudaStream, DriverError, LaunchConfig, PushKernelArg,
};
use cudarc::nvrtc::{compile_ptx_with_opts, CompileError, CompileOptions};

use crate::dp::{BIT_OF_DIV_4, CHOOSE, DP};
use crate::flush_5card::FLUSH;
use phe_omaha_fast_assets::{FLUSH_PLO4, NOFLUSH_PLO4};

use super::kernel::KERNEL_SRC;

const KERNEL_NAME: &str = "evaluate_plo4_batch_kernel";
const BLOCK_DIM: u32 = 256;

/// Errors produced by the CUDA backend.
#[derive(Debug, thiserror::Error)]
pub enum CudaEvalError {
    #[error("CUDA driver error: {0}")]
    Driver(#[from] DriverError),
    #[error("NVRTC compile error: {0}")]
    Compile(#[from] CompileError),
    #[error("input length mismatch: holes={holes} boards={boards} (expected equal)")]
    LengthMismatch { holes: usize, boards: usize },
    #[error("output buffer too small: got {got}, need {need}")]
    OutputTooSmall { got: usize, need: usize },
}

/// GPU-resident PLO4 evaluator.
///
/// Compiling the kernel and uploading the ~30 MB of FLUSH/NOFLUSH
/// tables takes ~1 s the first time the context is created. After
/// that every `evaluate_batch*` call reuses the same tables; only
/// the per-hand input/output crosses PCIe, and not even that for
/// the device-resident path.
///
/// # Solver integration
///
/// To integrate into a larger CUDA app (e.g., `poker-cuda-solver`):
///
/// 1. Construct via [`PloEvalContext::from_context`], passing your
///    existing `Arc<CudaContext>`. This shares the context (no
///    duplicate driver-side state) and keeps the eval tables in
///    the same memory pool.
/// 2. Launch with [`PloEvalContext::evaluate_batch_on_stream`],
///    passing your solver's stream. Useful for CUDA graph capture
///    (the launch is a regular kernel launch, recordable like any
///    other) and stream-pipelined workflows.
///
/// The kernel writes **i32 Cactus-Kev rank** in `[1, 7462]`, lower
/// = stronger — matching the CPU `evaluate_plo4_cards` convention.
/// Solvers that need the workspace-standard "u16 higher better"
/// strength can apply `7463 - rank` in a downstream fused kernel.
pub struct PloEvalContext {
    _ctx: Arc<CudaContext>,
    /// Stream used for the one-time table upload only. Subsequent
    /// kernel launches use the caller-supplied stream when going
    /// through the `_on_stream` API; the legacy `_device` API
    /// continues to use this stream.
    stream: Arc<CudaStream>,
    kernel: CudaFunction,
    d_dp: CudaSlice<u32>,
    d_choose: CudaSlice<u32>,
    d_bit_of_div4: CudaSlice<u16>,
    d_flush: CudaSlice<u16>,
    d_flush_plo4: CudaSlice<u16>,
    d_noflush_plo4: CudaSlice<u16>,
}

impl PloEvalContext {
    /// Initialise on device 0 with a fresh CudaContext. Convenient
    /// for stand-alone CPU-side callers; solver integration should
    /// prefer [`PloEvalContext::from_context`].
    pub fn new() -> Result<Self, CudaEvalError> {
        Self::with_device(0)
    }

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

        // Flatten DP[5][14][10] and CHOOSE[53][10] to 1-D for device.
        let dp_flat: Vec<u32> = {
            let mut v = Vec::with_capacity(5 * 14 * 10);
            for qi in 0..5 {
                for off in 0..14 {
                    for k in 0..10 {
                        v.push(DP[qi][off][k]);
                    }
                }
            }
            v
        };
        let choose_flat: Vec<u32> = {
            let mut v = Vec::with_capacity(53 * 10);
            for n in 0..53 {
                for k in 0..10 {
                    v.push(CHOOSE[n][k]);
                }
            }
            v
        };

        let bit_of_div4_vec: Vec<u16> = BIT_OF_DIV_4.to_vec();
        let flush_vec: Vec<u16> = FLUSH.to_vec();
        let flush_plo4_vec: Vec<u16> = FLUSH_PLO4.to_vec();
        let noflush_plo4_vec: Vec<u16> = NOFLUSH_PLO4.to_vec();

        let d_dp = stream.clone_htod(&dp_flat)?;
        let d_choose = stream.clone_htod(&choose_flat)?;
        let d_bit_of_div4 = stream.clone_htod(&bit_of_div4_vec)?;
        let d_flush = stream.clone_htod(&flush_vec)?;
        let d_flush_plo4 = stream.clone_htod(&flush_plo4_vec)?;
        let d_noflush_plo4 = stream.clone_htod(&noflush_plo4_vec)?;

        stream.synchronize()?;

        Ok(Self {
            _ctx: ctx,
            stream,
            kernel,
            d_dp,
            d_choose,
            d_bit_of_div4,
            d_flush,
            d_flush_plo4,
            d_noflush_plo4,
        })
    }

    /// Borrow the underlying default stream (the one tables were
    /// uploaded on). Useful when the caller wants to share streams
    /// for synchronization.
    pub fn default_stream(&self) -> &Arc<CudaStream> {
        &self.stream
    }

    /// Evaluate a batch of hands held on the host. Uploads
    /// `holes`/`boards` to the device, runs the kernel, downloads
    /// ranks. Synchronous — synchronizes the default stream before
    /// returning.
    pub fn evaluate_batch(&self, hands: &[([u8; 4], [u8; 5])]) -> Result<Vec<i32>, CudaEvalError> {
        let n = hands.len();
        let mut holes_flat = Vec::with_capacity(n * 4);
        let mut boards_flat = Vec::with_capacity(n * 5);
        for (hole, board) in hands {
            holes_flat.extend_from_slice(hole);
            boards_flat.extend_from_slice(board);
        }

        let d_holes = self.stream.clone_htod(&holes_flat)?;
        let d_boards = self.stream.clone_htod(&boards_flat)?;
        let mut d_out = unsafe { self.stream.alloc::<i32>(n) }?;

        self.launch_on(&self.stream, &d_holes, &d_boards, &mut d_out, n)?;
        self.stream.synchronize()?;

        let out = self.stream.clone_dtoh(&d_out)?;
        Ok(out)
    }

    /// Evaluate a batch where the caller already holds device
    /// buffers. Launches on the context's default stream.
    /// Does **not** synchronize — the caller is responsible.
    ///
    /// For solver integration where you want to launch on your own
    /// stream (e.g., for CUDA graph capture), prefer
    /// [`evaluate_batch_on_stream`](Self::evaluate_batch_on_stream).
    pub fn evaluate_batch_device(
        &self,
        d_holes: &CudaSlice<u8>,
        d_boards: &CudaSlice<u8>,
        d_out: &mut CudaSlice<i32>,
        n: usize,
    ) -> Result<(), CudaEvalError> {
        self.evaluate_batch_on_stream(&self.stream, d_holes, d_boards, d_out, n)
    }

    /// Evaluate on a caller-supplied stream. Buffers must already
    /// live on the device the stream belongs to. The launch is
    /// asynchronous and capturable into a CUDA graph by the caller.
    ///
    /// This is the primary entry point for solver integration:
    /// `poker-cuda-solver`-style callers should pass their per-pass
    /// stream so that the eval kernel orders correctly with the
    /// surrounding CFR forward / backward / showdown kernels.
    pub fn evaluate_batch_on_stream(
        &self,
        stream: &CudaStream,
        d_holes: &CudaSlice<u8>,
        d_boards: &CudaSlice<u8>,
        d_out: &mut CudaSlice<i32>,
        n: usize,
    ) -> Result<(), CudaEvalError> {
        if d_holes.len() < n * 4 {
            return Err(CudaEvalError::OutputTooSmall {
                got: d_holes.len(),
                need: n * 4,
            });
        }
        if d_boards.len() < n * 5 {
            return Err(CudaEvalError::OutputTooSmall {
                got: d_boards.len(),
                need: n * 5,
            });
        }
        if d_out.len() < n {
            return Err(CudaEvalError::OutputTooSmall {
                got: d_out.len(),
                need: n,
            });
        }
        self.launch_on(stream, d_holes, d_boards, d_out, n)
    }

    fn launch_on(
        &self,
        stream: &CudaStream,
        d_holes: &CudaSlice<u8>,
        d_boards: &CudaSlice<u8>,
        d_out: &mut CudaSlice<i32>,
        n: usize,
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
        b.arg(d_holes);
        b.arg(d_boards);
        b.arg(d_out);
        b.arg(&self.d_dp);
        b.arg(&self.d_choose);
        b.arg(&self.d_bit_of_div4);
        b.arg(&self.d_flush);
        b.arg(&self.d_flush_plo4);
        b.arg(&self.d_noflush_plo4);
        unsafe { b.launch(cfg) }?;
        Ok(())
    }
}
