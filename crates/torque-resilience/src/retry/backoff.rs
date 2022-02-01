//! Jittered exponential backoff algorithms for fault tolerance.
//!
//! Implements AWS Architecture backoff strategies:
//! - Full Jitter: `t = rand(0, min(cap, base * 2^attempt))`
//! - Equal Jitter: `t = half/2 + rand(0, half/2)`
//! - Decorrelated Jitter: `t = min(cap, rand(base, prev * 3))`

use rand::Rng;
use std::time::Duration;

/// Jitter backoff strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JitterStrategy {
    Full,
    Equal,
    Decorrelated,
}

/// Backoff generator for retrying transient upstream failures.
#[derive(Debug, Clone)]
pub struct Backoff {
    base_ms: u64,
    cap_ms: u64,
    strategy: JitterStrategy,
}

impl Backoff {
    pub fn new(base_ms: u64, cap_ms: u64, strategy: JitterStrategy) -> Self {
        Self {
            base_ms,
            cap_ms,
            strategy,
        }
    }

    /// Calculate the sleep duration for attempt number `attempt` (0-indexed).
    pub fn calculate(&self, attempt: u32, prev_sleep: Duration) -> Duration {
        let mut rng = rand::thread_rng();
        let exp = 2u64.saturating_pow(attempt);
        let temp = (self.base_ms.saturating_mul(exp)).min(self.cap_ms);

        let sleep_ms = match self.strategy {
            JitterStrategy::Full => {
                if temp == 0 {
                    0
                } else {
                    rng.gen_range(0..=temp)
                }
            }
            JitterStrategy::Equal => {
                let half = temp / 2;
                if half == 0 {
                    0
                } else {
                    half + rng.gen_range(0..=half)
                }
            }
            JitterStrategy::Decorrelated => {
                let prev_ms = prev_sleep.as_millis() as u64;
                let upper = (prev_ms.saturating_mul(3)).max(self.base_ms);
                rng.gen_range(self.base_ms..=upper).min(self.cap_ms)
            }
        };

        Duration::from_millis(sleep_ms)
    }
}
