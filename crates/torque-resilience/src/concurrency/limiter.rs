//! Dynamic concurrency limiters based on Gradient and Vegas latency algorithms.
//!
//! Automatically sizes allowed in-flight requests based on observed RTT relative
//! to minimum baseline latency (Little's Law feedback control).

use std::sync::atomic::{AtomicUsize, Ordering};
use torque_core::atomic::AtomicF64;
use torque_core::error::{Result, TorqueError};

/// Dynamic concurrency limiter tracking gradient latency feedback.
pub struct ConcurrencyLimiter {
    cluster: String,
    current_limit: AtomicF64,
    min_limit: f64,
    max_limit: f64,
    active_in_flight: AtomicUsize,
    min_rtt_ms: AtomicF64,
    smoothed_rtt_ms: AtomicF64,
}

impl ConcurrencyLimiter {
    /// Construct a new concurrency limiter for a cluster.
    pub fn new(cluster: impl Into<String>, initial_limit: f64, min_limit: f64, max_limit: f64) -> Self {
        Self {
            cluster: cluster.into(),
            current_limit: AtomicF64::new(initial_limit),
            min_limit,
            max_limit,
            active_in_flight: AtomicUsize::new(0),
            min_rtt_ms: AtomicF64::new(1000.0), // high initial anchor
            smoothed_rtt_ms: AtomicF64::new(10.0),
        }
    }

    /// Try to acquire an execution slot.
    /// Returns Ok(()) if permitted, or Err(TorqueError::ConcurrencyLimitReached) if shed.
    pub fn acquire(&self) -> Result<()> {
        let cur = self.active_in_flight.fetch_add(1, Ordering::Relaxed);
        let limit = self.current_limit.load(Ordering::Relaxed).ceil() as usize;

        if cur >= limit {
            self.active_in_flight.fetch_sub(1, Ordering::Relaxed);
            Err(TorqueError::ConcurrencyLimitReached {
                cluster: self.cluster.clone(),
                current: cur,
                max: limit,
            })
        } else {
            Ok(())
        }
    }

    /// Release an execution slot and update the dynamic limit based on observed RTT.
    pub fn release(&self, rtt_ms: f64) {
        self.active_in_flight.fetch_sub(1, Ordering::Relaxed);

        let rtt = rtt_ms.max(0.1);

        // Update minimum RTT anchor
        let mut min_rtt = self.min_rtt_ms.load(Ordering::Relaxed);
        if rtt < min_rtt {
            self.min_rtt_ms.store(rtt, Ordering::Relaxed);
            min_rtt = rtt;
        }

        // Update smoothed RTT (EWMA alpha = 0.1)
        let smoothed = self.smoothed_rtt_ms.load(Ordering::Relaxed);
        let next_smoothed = (0.1 * rtt) + (0.9 * smoothed);
        self.smoothed_rtt_ms.store(next_smoothed, Ordering::Relaxed);

        // Gradient feedback: g = min_rtt / smoothed_rtt
        let gradient = (min_rtt / next_smoothed).clamp(0.5, 1.0);
        let current_limit = self.current_limit.load(Ordering::Relaxed);

        let new_limit = if gradient >= 0.95 {
            // Latency stable: probe upward (+1)
            (current_limit + 1.0).min(self.max_limit)
        } else {
            // Latency inflating: shed back (limit * gradient)
            (current_limit * gradient).max(self.min_limit)
        };

        self.current_limit.store(new_limit, Ordering::Relaxed);
    }

    /// Current allowed in-flight limit.
    pub fn limit(&self) -> usize {
        self.current_limit.load(Ordering::Relaxed).ceil() as usize
    }

    /// Current active in-flight count.
    pub fn in_flight(&self) -> usize {
        self.active_in_flight.load(Ordering::Relaxed)
    }
}
