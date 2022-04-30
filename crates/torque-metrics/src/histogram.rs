//! High Dynamic Range (HDR) latency histogram with logarithmic bucket intervals.

use std::sync::atomic::{AtomicU64, Ordering};

/// Number of logarithmic latency buckets.
const NUM_BUCKETS: usize = 16;

/// Upper boundaries in milliseconds for each bucket:
/// 0.5ms, 1ms, 2ms, 5ms, 10ms, 25ms, 50ms, 100ms, 250ms, 500ms, 1s, 2.5s, 5s, 10s, 30s, +inf
pub static BUCKET_BOUNDS_MS: [f64; NUM_BUCKETS] = [
    0.5,
    1.0,
    2.0,
    5.0,
    10.0,
    25.0,
    50.0,
    100.0,
    250.0,
    500.0,
    1000.0,
    2500.0,
    5000.0,
    10000.0,
    30000.0,
    f64::INFINITY,
];

/// Lock-free latency histogram tracking response duration distribution.
pub struct HdrHistogram {
    buckets: [AtomicU64; NUM_BUCKETS],
    total_count: AtomicU64,
    sum_ms: torque_core::atomic::AtomicF64,
}

impl HdrHistogram {
    /// Construct a new empty latency histogram.
    pub fn new() -> Self {
        Self {
            buckets: std::array::from_fn(|_| AtomicU64::new(0)),
            total_count: AtomicU64::new(0),
            sum_ms: torque_core::atomic::AtomicF64::new(0.0),
        }
    }

    /// Record an observed latency sample in milliseconds.
    pub fn record(&self, latency_ms: f64) {
        let sample = latency_ms.max(0.0);
        self.total_count.fetch_add(1, Ordering::Relaxed);
        self.sum_ms.fetch_add(sample);

        for (i, &bound) in BUCKET_BOUNDS_MS.iter().enumerate() {
            if sample <= bound {
                self.buckets[i].fetch_add(1, Ordering::Relaxed);
                break;
            }
        }
    }

    /// Calculate estimated quantile latency (e.g. 0.50, 0.90, 0.99).
    pub fn quantile(&self, q: f64) -> f64 {
        assert!(q >= 0.0 && q <= 1.0, "Quantile must be in range [0.0, 1.0]");
        let total = self.total_count.load(Ordering::Relaxed);
        if total == 0 {
            return 0.0;
        }

        let target = (total as f64 * q).ceil() as u64;
        let mut accumulated = 0;

        for (i, &bound) in BUCKET_BOUNDS_MS.iter().enumerate() {
            accumulated += self.buckets[i].load(Ordering::Relaxed);
            if accumulated >= target {
                return if bound.is_infinite() { 30000.0 } else { bound };
            }
        }

        30000.0
    }

    /// Total count of samples recorded.
    pub fn count(&self) -> u64 {
        self.total_count.load(Ordering::Relaxed)
    }

    /// Mean latency across all samples in milliseconds.
    pub fn mean(&self) -> f64 {
        let count = self.total_count.load(Ordering::Relaxed);
        if count == 0 {
            0.0
        } else {
            self.sum_ms.load(Ordering::Relaxed) / count as f64
        }
    }

    /// Raw bucket counts.
    pub fn bucket_counts(&self) -> [u64; NUM_BUCKETS] {
        std::array::from_fn(|i| self.buckets[i].load(Ordering::Relaxed))
    }
}

impl Default for HdrHistogram {
    fn default() -> Self {
        Self::new()
    }
}
