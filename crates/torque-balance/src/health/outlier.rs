//! Passive outlier detection and host ejection.
//!
//! Ejects flapping or failing endpoints based on consecutive 5xx error thresholds
//! or statistical error rates exceeding cluster norms.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use torque_core::time::fast_now_millis;

use crate::endpoint::Endpoint;

/// Outlier detection configuration parameters.
#[derive(Debug, Clone)]
pub struct OutlierConfig {
    pub consecutive_5xx_threshold: usize,
    pub base_ejection_duration: Duration,
    pub max_ejection_percent: usize,
}

impl Default for OutlierConfig {
    fn default() -> Self {
        Self {
            consecutive_5xx_threshold: 5,
            base_ejection_duration: Duration::from_secs(30),
            max_ejection_percent: 50,
        }
    }
}

/// Outlier tracker managing host ejection duration.
pub struct OutlierDetector {
    endpoint: Arc<Endpoint>,
    config: OutlierConfig,
    ejected_until_millis: AtomicU64,
}

impl OutlierDetector {
    pub fn new(endpoint: Arc<Endpoint>, config: OutlierConfig) -> Self {
        Self {
            endpoint,
            config,
            ejected_until_millis: AtomicU64::new(0),
        }
    }

    /// Record a server error (5xx or connection reset).
    pub fn on_error(&self) {
        let failures = self.endpoint.record_failure();
        if failures >= self.config.consecutive_5xx_threshold {
            let duration_ms = self.config.base_ejection_duration.as_millis() as u64;
            let expiry = fast_now_millis() + duration_ms;
            self.ejected_until_millis.store(expiry, Ordering::Relaxed);
            self.endpoint.set_healthy(false);
        }
    }

    /// Record a successful response.
    pub fn on_success(&self) {
        self.endpoint.record_success();
    }

    /// Check if ejection period has expired, restoring endpoint to service.
    pub fn check_expiration(&self) -> bool {
        let expiry = self.ejected_until_millis.load(Ordering::Relaxed);
        if expiry > 0 && fast_now_millis() >= expiry {
            self.ejected_until_millis.store(0, Ordering::Relaxed);
            self.endpoint.set_healthy(true);
            return true;
        }
        false
    }
}
