//! Nanosecond-precision token bucket rate limiter.

use std::sync::atomic::{AtomicU64, Ordering};
use torque_core::time::fast_now_nanos;

/// Token bucket tracking rate allowance and burst capacity.
pub struct TokenBucket {
    rate_per_sec: u64,
    burst_capacity: u64,
    tokens: AtomicU64,
    last_update_nanos: AtomicU64,
}

impl TokenBucket {
    /// Create a new TokenBucket with steady refill rate per second and burst ceiling.
    pub fn new(rate_per_sec: u64, burst_capacity: u64) -> Self {
        let rate_per_sec = rate_per_sec.max(1);
        let burst_capacity = burst_capacity.max(1);
        let initial_tokens = burst_capacity * 1_000_000_000; // scaled by 1e9 for precision

        Self {
            rate_per_sec,
            burst_capacity,
            tokens: AtomicU64::new(initial_tokens),
            last_update_nanos: AtomicU64::new(fast_now_nanos()),
        }
    }

    /// Attempt to acquire `cost` tokens (default 1). Returns Ok(remaining) or Err(wait_duration_ms).
    pub fn try_acquire(&self, cost: u64) -> Result<u64, u64> {
        let cost_scaled = cost * 1_000_000_000;
        let max_scaled = self.burst_capacity * 1_000_000_000;
        let fill_rate_per_nano = self.rate_per_sec; // tokens per second = (rate * 1e9) / 1e9 nanos

        let now = fast_now_nanos();
        let last = self.last_update_nanos.swap(now, Ordering::Relaxed);
        let elapsed_nanos = now.saturating_sub(last);

        let delta_tokens = elapsed_nanos.saturating_mul(fill_rate_per_nano);

        // Update tokens atomically
        let mut cur = self.tokens.load(Ordering::Relaxed);
        loop {
            let refilled = cur.saturating_add(delta_tokens).min(max_scaled);
            if refilled >= cost_scaled {
                let next = refilled - cost_scaled;
                match self.tokens.compare_exchange_weak(
                    cur,
                    next,
                    Ordering::Relaxed,
                    Ordering::Relaxed,
                ) {
                    Ok(_) => return Ok(next / 1_000_000_000),
                    Err(actual) => cur = actual,
                }
            } else {
                // Not enough tokens
                let deficit = cost_scaled - refilled;
                let wait_nanos = deficit / fill_rate_per_nano.max(1);
                let wait_ms = (wait_nanos / 1_000_000).max(1);
                return Err(wait_ms);
            }
        }
    }
}
