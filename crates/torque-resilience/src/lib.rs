//! # Torque Resilience
//!
//! Multi-tier resilience engine providing 4-state circuit breakers, high-precision
//! rate limiters, gossip synchronization, retry budgeting, and dynamic concurrency limiters.

pub mod circuit;
pub mod concurrency;
pub mod ratelimit;
pub mod retry;

pub use circuit::{CircuitBreaker, CircuitBreakerConfig, CircuitState, SlidingWindow};
pub use concurrency::ConcurrencyLimiter;
pub use ratelimit::{
    GossipAggregator, GossipSyncMessage, LeakyBucket, RateLimitConfig, RateLimiterRegistry,
    SlidingWindowLimiter, TokenBucket,
};
pub use retry::{Backoff, JitterStrategy, RetryBudget};

/// Resilience prelude.
pub mod prelude {
    pub use crate::circuit::{CircuitBreaker, CircuitBreakerConfig, CircuitState};
    pub use crate::concurrency::ConcurrencyLimiter;
    pub use crate::ratelimit::{RateLimitConfig, RateLimiterRegistry, TokenBucket};
    pub use crate::retry::{Backoff, JitterStrategy, RetryBudget};
}
