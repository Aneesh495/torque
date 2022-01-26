//! Leaky bucket rate limiter with smooth constant outflow drain.

use std::sync::atomic::{AtomicU64, Ordering};
use torque_core::time::fast_now_millis;

/// Leaky bucket rate limiter.
pub struct LeakyBucket {
    capacity: u64,
    drain_rate_per_sec: u64,
    water_level: AtomicU64,
    last_drain_millis: AtomicU64,
}

impl LeakyBucket {
    pub fn new(capacity: u64, drain_rate_per_sec: u64) -> Self {
        Self {
            capacity,
            drain_rate_per_sec: drain_rate_per_sec.max(1),
            water_level: AtomicU64::new(0),
            last_drain_millis: AtomicU64::new(fast_now_millis()),
        }
    }

    /// Try to pour `amount` water units into the bucket. Returns Ok(new_level) or Err(overflow_amount).
    pub fn try_acquire(&self, amount: u64) -> Result<u64, u64> {
        let now = fast_now_millis();
        let last = self.last_drain_millis.swap(now, Ordering::Relaxed);
        let elapsed_ms = now.saturating_sub(last);

        let leaked = (elapsed_ms * self.drain_rate_per_sec) / 1000;

        let mut cur = self.water_level.load(Ordering::Relaxed);
        loop {
            let drained = cur.saturating_sub(leaked);
            let next = drained + amount;
            if next <= self.capacity {
                match self.water_level.compare_exchange_weak(
                    cur,
                    next,
                    Ordering::Relaxed,
                    Ordering::Relaxed,
                ) {
                    Ok(_) => return Ok(next),
                    Err(actual) => cur = actual,
                }
            } else {
                let overflow = next - self.capacity;
                let wait_ms = ((overflow * 1000) / self.drain_rate_per_sec).max(1);
                return Err(wait_ms);
            }
        }
    }
}
