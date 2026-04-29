//! Shared logic for asset generators.
//!
//! Currently exposes a parameterized naive 5/6/7-card high-hand evaluator
//! used by `gen-deuce-seven-lookup` (and reusable for any future variant
//! that differs only in wheel handling).

pub mod naive_high;
