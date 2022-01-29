//! Keyed rate limiter registry and policy manager.

use dashmap::DashMap;
use std::sync::Arc;
use torque_core::error::{Result, TorqueError};

use super::token_bucket::TokenBucket;

/// Rate limiter configuration per key.
#[derive(Debug, Clone, Copy)]
pub struct RateLimitConfig {
    pub rate_per_sec: u64,
    pub burst_capacity: u64,
}

/// Registry of rate limit buckets indexed by request attribute keys (IP, API key, etc.).
pub struct RateLimiterRegistry {
    default_config: RateLimitConfig,
    buckets: DashMap<String, Arc<TokenBucket>>,
}

impl RateLimiterRegistry {
    /// Construct a registry with default rate limit policies.
    pub fn new(default_config: RateLimitConfig) -> Self {
        Self {
            default_config,
            buckets: DashMap::new(),
        }
    }

    /// Check rate limit for a specific key.
    /// Returns Ok(()) if permitted, or Err(TorqueError::RateLimitExceeded).
    pub fn check(&self, key: &str) -> Result<()> {
        let bucket = self.buckets.entry(key.to_string()).or_insert_with(|| {
            Arc::new(TokenBucket::new(
                self.default_config.rate_per_sec,
                self.default_config.burst_capacity,
            ))
        });

        match bucket.try_acquire(1) {
            Ok(_) => Ok(()),
            Err(wait_ms) => Err(TorqueError::RateLimitExceeded {
                key: key.to_string(),
                retry_after_ms: wait_ms,
            }),
        }
    }

    /// Number of active tracked keys.
    pub fn len(&self) -> usize {
        self.buckets.len()
    }

    /// Whether registry has zero active keys.
    pub fn is_empty(&self) -> bool {
        self.buckets.is_empty()
    }
}
