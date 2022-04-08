//! # Torque Bench
//!
//! Ultra high-throughput load generator, pipelined HTTP benchmark runner,
//! and logarithmic latency distribution analyzer.

pub mod engine;

pub use engine::{BenchConfig, BenchEngine, BenchResult};

/// Bench prelude.
pub mod prelude {
    pub use crate::engine::{BenchConfig, BenchEngine, BenchResult};
}
