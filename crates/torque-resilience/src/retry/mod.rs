//! Retry management and backoff strategies.

pub mod backoff;
pub mod budget;

pub use backoff::{Backoff, JitterStrategy};
pub use budget::RetryBudget;
