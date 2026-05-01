//! CUDA backend for PLO4 evaluation.
//!
//! Enabled via the `cuda` feature. Provides a `PloEvalContext` that
//! uploads the FLUSH/NOFLUSH/DP tables to the GPU once on creation
//! and exposes a 1-thread-per-hand kernel for batch evaluation.
//!
//! Two entry points:
//!
//! * [`PloEvalContext::evaluate_batch`] — host slice in, host Vec out.
//!   Uploads / downloads each call. Suited for CPU-resident callers
//!   that just want GPU throughput.
//! * [`PloEvalContext::evaluate_batch_device`] — caller-provided
//!   device buffers. Suited for solver integration where hand and
//!   board data already live on the GPU; this path crosses zero
//!   PCIe bytes per call.
//!
//! Both share the same kernel; only the data movement differs.

pub mod kernel;

mod context;

pub use context::{PloEvalContext, CudaEvalError};
