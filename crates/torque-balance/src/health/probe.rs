//! Active HTTP and TCP synthetic health probing.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crate::endpoint::Endpoint;

/// Protocol type for active health checks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProbeProtocol {
    Http { path: String, expected_status: u16 },
    Tcp,
}

/// Active health check configuration parameters.
#[derive(Debug, Clone)]
pub struct HealthCheckConfig {
    pub protocol: ProbeProtocol,
    pub interval: Duration,
    pub timeout: Duration,
    pub healthy_threshold: usize,
    pub unhealthy_threshold: usize,
}

impl Default for HealthCheckConfig {
    fn default() -> Self {
        Self {
            protocol: ProbeProtocol::Http {
                path: "/healthz".to_string(),
                expected_status: 200,
            },
            interval: Duration::from_millis(2000),
            timeout: Duration::from_millis(500),
            healthy_threshold: 2,
            unhealthy_threshold: 3,
        }
    }
}

/// State machine tracking consecutive successes and failures for an endpoint.
#[derive(Debug)]
pub struct EndpointHealthTracker {
    pub endpoint: Arc<Endpoint>,
    successes: AtomicUsize,
    failures: AtomicUsize,
}

impl EndpointHealthTracker {
    pub fn new(endpoint: Arc<Endpoint>) -> Self {
        Self {
            endpoint,
            successes: AtomicUsize::new(0),
            failures: AtomicUsize::new(0),
        }
    }

    /// Record a probe success and update healthy status if threshold reached.
    pub fn on_success(&self, threshold: usize) {
        self.failures.store(0, Ordering::Relaxed);
        let count = self.successes.fetch_add(1, Ordering::Relaxed) + 1;
        if count >= threshold && !self.endpoint.is_available() {
            self.endpoint.set_healthy(true);
        }
    }

    /// Record a probe failure and update unhealthy status if threshold reached.
    pub fn on_failure(&self, threshold: usize) {
        self.successes.store(0, Ordering::Relaxed);
        let count = self.failures.fetch_add(1, Ordering::Relaxed) + 1;
        if count >= threshold && self.endpoint.is_available() {
            self.endpoint.set_healthy(false);
        }
    }
}
