//! Circular time-bucketed sliding window for error rate accounting.

use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use torque_core::time::fast_now_millis;

/// Granularity of an individual time bucket (100 milliseconds).
const BUCKET_DURATION_MS: u64 = 100;

/// Statistical counter bucket for a single 100ms interval.
#[derive(Debug, Default)]
struct Bucket {
    timestamp_bucket: AtomicU64,
    total: AtomicUsize,
    failed: AtomicUsize,
    slow: AtomicUsize,
}

impl Bucket {
    fn reset(&self, bucket_id: u64) {
        self.timestamp_bucket.store(bucket_id, Ordering::Relaxed);
        self.total.store(0, Ordering::Relaxed);
        self.failed.store(0, Ordering::Relaxed);
        self.slow.store(0, Ordering::Relaxed);
    }
}

/// Circular sliding window aggregating failure and latency statistics.
pub struct SlidingWindow {
    buckets: Vec<Bucket>,
    num_buckets: usize,
    window_duration_ms: u64,
}

/// Aggregated metrics across the entire sliding window.
#[derive(Debug, Clone, Copy, Default)]
pub struct WindowSnapshot {
    pub total: usize,
    pub failed: usize,
    pub slow: usize,
    pub failure_rate: f64,
    pub slow_rate: f64,
}

impl SlidingWindow {
    /// Construct a sliding window with total duration (e.g. 10_000 ms = 100 buckets).
    pub fn new(window_duration_ms: u64) -> Self {
        let num_buckets = (window_duration_ms / BUCKET_DURATION_MS).max(10) as usize;
        let mut buckets = Vec::with_capacity(num_buckets);
        for _ in 0..num_buckets {
            buckets.push(Bucket::default());
        }
        Self {
            buckets,
            num_buckets,
            window_duration_ms,
        }
    }

    /// Record an execution outcome.
    pub fn record(&self, is_failure: bool, is_slow: bool) {
        let now_ms = fast_now_millis();
        let bucket_id = now_ms / BUCKET_DURATION_MS;
        let idx = (bucket_id as usize) % self.num_buckets;
        let bucket = &self.buckets[idx];

        if bucket.timestamp_bucket.load(Ordering::Relaxed) != bucket_id {
            bucket.reset(bucket_id);
        }

        bucket.total.fetch_add(1, Ordering::Relaxed);
        if is_failure {
            bucket.failed.fetch_add(1, Ordering::Relaxed);
        }
        if is_slow {
            bucket.slow.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Aggregate all active buckets within the window horizon.
    pub fn snapshot(&self) -> WindowSnapshot {
        let now_ms = fast_now_millis();
        let current_bucket_id = now_ms / BUCKET_DURATION_MS;
        let oldest_valid_bucket = current_bucket_id.saturating_sub(self.num_buckets as u64);

        let mut total = 0;
        let mut failed = 0;
        let mut slow = 0;

        for bucket in &self.buckets {
            let b_id = bucket.timestamp_bucket.load(Ordering::Relaxed);
            if b_id >= oldest_valid_bucket && b_id <= current_bucket_id {
                total += bucket.total.load(Ordering::Relaxed);
                failed += bucket.failed.load(Ordering::Relaxed);
                slow += bucket.slow.load(Ordering::Relaxed);
            }
        }

        let failure_rate = if total > 0 {
            failed as f64 / total as f64
        } else {
            0.0
        };

        let slow_rate = if total > 0 {
            slow as f64 / total as f64
        } else {
            0.0
        };

        WindowSnapshot {
            total,
            failed,
            slow,
            failure_rate,
            slow_rate,
        }
    }

    /// Clear all statistics.
    pub fn clear(&self) {
        for bucket in &self.buckets {
            bucket.reset(0);
        }
    }

    /// Configured window duration in milliseconds.
    pub fn window_duration_ms(&self) -> u64 {
        self.window_duration_ms
    }
}
