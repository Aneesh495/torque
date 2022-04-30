//! Token-based retry budgets preventing cascading retry storms.

use std::sync::atomic::{AtomicI64, Ordering};

/// Capped retry budget tracking ratio of allowable retries to total traffic.
pub struct RetryBudget {
    tokens: AtomicI64,
    max_tokens: i64,
    retry_ratio: f64,
}

impl RetryBudget {
    /// Construct a retry budget with max token capacity and retry allowance ratio (e.g. 0.20 = 20%).
    pub fn new(max_tokens: u32, retry_ratio: f64) -> Self {
        assert!(retry_ratio > 0.0 && retry_ratio <= 1.0);
        let max_tokens = max_tokens as i64 * 1000;
        Self {
            tokens: AtomicI64::new(max_tokens),
            max_tokens,
            retry_ratio,
        }
    }

    /// Record a regular client request, depositing fractional retry credit.
    pub fn record_request(&self) {
        let deposit = (self.retry_ratio * 1000.0) as i64;
        let mut cur = self.tokens.load(Ordering::Relaxed);
        loop {
            let next = (cur + deposit).min(self.max_tokens);
            match self
                .tokens
                .compare_exchange_weak(cur, next, Ordering::Relaxed, Ordering::Relaxed)
            {
                Ok(_) => break,
                Err(actual) => cur = actual,
            }
        }
    }

    /// Attempt to acquire permission for a retry attempt.
    /// Consumes 1 full retry token (1000 units). Returns true if allowed, false if budget empty.
    pub fn try_acquire_retry(&self) -> bool {
        let cost = 1000;
        let mut cur = self.tokens.load(Ordering::Relaxed);
        loop {
            if cur >= cost {
                let next = cur - cost;
                match self.tokens.compare_exchange_weak(
                    cur,
                    next,
                    Ordering::Relaxed,
                    Ordering::Relaxed,
                ) {
                    Ok(_) => return true,
                    Err(actual) => cur = actual,
                }
            } else {
                return false;
            }
        }
    }

    /// Current available retry tokens.
    pub fn available_tokens(&self) -> u32 {
        let val = self.tokens.load(Ordering::Relaxed);
        (val.max(0) / 1000) as u32
    }
}

impl Default for RetryBudget {
    fn default() -> Self {
        Self::new(100, 0.20)
    }
}
