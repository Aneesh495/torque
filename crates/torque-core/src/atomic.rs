//! Lock-free atomic structures for real-time telemetry and state tracking.
//!
//! Provides atomic floating point types (`AtomicF64`), atomic Exponentially
//! Weighted Moving Average (`AtomicEwma`), and concurrent bitsets.

use std::sync::atomic::{AtomicU64, Ordering};

/// Atomic IEEE 754 64-bit floating point number.
#[derive(Debug)]
pub struct AtomicF64 {
    bits: AtomicU64,
}

impl AtomicF64 {
    /// Construct a new AtomicF64 with initial value.
    pub const fn new(val: f64) -> Self {
        Self {
            bits: AtomicU64::new(val.to_bits()),
        }
    }

    /// Load the current floating point value.
    pub fn load(&self, order: Ordering) -> f64 {
        f64::from_bits(self.bits.load(order))
    }

    /// Store a new floating point value.
    pub fn store(&self, val: f64, order: Ordering) {
        self.bits.store(val.to_bits(), order);
    }

    /// Atomically swap the value, returning previous float.
    pub fn swap(&self, val: f64, order: Ordering) -> f64 {
        f64::from_bits(self.bits.swap(val.to_bits(), order))
    }

    /// Compare-and-swap float value using IEEE representation.
    pub fn compare_exchange(
        &self,
        current: f64,
        new: f64,
        success: Ordering,
        failure: Ordering,
    ) -> Result<f64, f64> {
        self.bits
            .compare_exchange(current.to_bits(), new.to_bits(), success, failure)
            .map(f64::from_bits)
            .map_err(f64::from_bits)
    }

    /// Atomically add a delta to the current value using a CAS loop.
    pub fn fetch_add(&self, delta: f64) -> f64 {
        let mut current_bits = self.bits.load(Ordering::Relaxed);
        loop {
            let current_val = f64::from_bits(current_bits);
            let new_val = current_val + delta;
            match self.bits.compare_exchange_weak(
                current_bits,
                new_val.to_bits(),
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => return current_val,
                Err(actual) => current_bits = actual,
            }
        }
    }
}

impl Default for AtomicF64 {
    fn default() -> Self {
        Self::new(0.0)
    }
}

/// Atomic Exponentially Weighted Moving Average (EWMA) tracker.
///
/// Computes `EWMA_new = alpha * sample + (1 - alpha) * EWMA_old` lock-free.
#[derive(Debug)]
pub struct AtomicEwma {
    value: AtomicF64,
    alpha: f64,
}

impl AtomicEwma {
    /// Construct a new AtomicEwma with smoothing factor alpha (between 0.0 and 1.0).
    pub fn new(alpha: f64, initial: f64) -> Self {
        assert!(
            alpha > 0.0 && alpha <= 1.0,
            "Alpha must be in range (0.0, 1.0]"
        );
        Self {
            value: AtomicF64::new(initial),
            alpha,
        }
    }

    /// Record a new observation sample and update the EWMA estimate atomically.
    pub fn update(&self, sample: f64) -> f64 {
        let mut current = self.value.load(Ordering::Relaxed);
        loop {
            let next = (self.alpha * sample) + ((1.0 - self.alpha) * current);
            match self.value.compare_exchange(
                current,
                next,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => return next,
                Err(actual) => current = actual,
            }
        }
    }

    /// Retrieve the current EWMA estimate.
    pub fn get(&self) -> f64 {
        self.value.load(Ordering::Relaxed)
    }
}

/// 64-bit concurrent atomic bitset for active health/state flags.
#[derive(Debug, Default)]
pub struct AtomicBitSet {
    mask: AtomicU64,
}

impl AtomicBitSet {
    /// Create a new bitset with all bits cleared.
    pub const fn new() -> Self {
        Self {
            mask: AtomicU64::new(0),
        }
    }

    /// Set bit at index `idx` (0..63) to true.
    pub fn set(&self, idx: usize) {
        assert!(idx < 64, "Bit index out of bounds");
        let bit = 1u64 << idx;
        self.mask.fetch_or(bit, Ordering::Relaxed);
    }

    /// Clear bit at index `idx` (0..63) to false.
    pub fn clear(&self, idx: usize) {
        assert!(idx < 64, "Bit index out of bounds");
        let bit = !(1u64 << idx);
        self.mask.fetch_and(bit, Ordering::Relaxed);
    }

    /// Test if bit at index `idx` is set.
    pub fn test(&self, idx: usize) -> bool {
        assert!(idx < 64, "Bit index out of bounds");
        let bit = 1u64 << idx;
        (self.mask.load(Ordering::Relaxed) & bit) != 0
    }

    /// Count number of set bits (popcount).
    pub fn count_ones(&self) -> u32 {
        self.mask.load(Ordering::Relaxed).count_ones()
    }

    /// Return raw 64-bit mask.
    pub fn raw(&self) -> u64 {
        self.mask.load(Ordering::Relaxed)
    }
}
