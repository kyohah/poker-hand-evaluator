//! CUDA backend for Hold'em (5-7 card) hand evaluation.
//!
//! Enabled via the `cuda` feature. Provides a `HoldemEvalContext`
//! that uploads the LOOKUP / LOOKUP_FLUSH / OFFSETS / CARDS tables
//! to the GPU once on creation (~212 KB total, comfortably L2-
//! resident on every CUDA-capable GPU) and exposes a
//! 1-thread-per-hand kernel for batch evaluation.
//!
//! Mirrors the `phe-omaha-fast::cuda` API surface intentionally:
//!
//! * [`HoldemEvalContext::new`] / [`HoldemEvalContext::from_context`]
//!   for stand-alone vs solver-integrated initialisation.
//! * [`HoldemEvalContext::evaluate_batch`] — host slice in, host
//!   Vec out, includes upload + download.
//! * [`HoldemEvalContext::evaluate_batch_device`] — caller-owned
//!   device buffers, default stream, no PCIe.
//! * [`HoldemEvalContext::evaluate_batch_on_stream`] — explicit
//!   stream parameter, capturable into the caller's CUDA graph.
//!
//! The kernel handles 5, 6, and 7-card hands via a single
//! `cards_per_hand: u32` parameter; the loop unrolls in NVRTC's
//! generated PTX for each call site.
//!
//! Output is the workspace-standard u16 strength (higher =
//! stronger), identical in encoding to `HighRule::evaluate` on the
//! CPU.

pub mod kernel;

mod context;

pub use context::{CudaEvalError, HoldemEvalContext};
