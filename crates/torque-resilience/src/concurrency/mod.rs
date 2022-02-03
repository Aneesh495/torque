//! Concurrency control and dynamic load shedding.

pub mod limiter;

pub use limiter::ConcurrencyLimiter;
