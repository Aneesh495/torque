//! Upstream endpoint representation and live runtime metrics.

use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use torque_core::atomic::AtomicEwma;

/// An individual upstream server destination.
#[derive(Debug)]
pub struct Endpoint {
    pub addr: SocketAddr,
    pub weight: u32,
    pub active_requests: AtomicUsize,
    pub ewma_latency_ms: AtomicEwma,
    pub is_healthy: AtomicBool,
    pub consecutive_failures: AtomicUsize,
    pub total_requests: AtomicU64,
    pub current_swrr_weight: AtomicIsize,
}

impl Endpoint {
    /// Construct a new endpoint with static weight and default 10ms EWMA latency.
    pub fn new(addr: SocketAddr, weight: u32) -> Arc<Self> {
        let weight = weight.max(1);
        Arc::new(Self {
            addr,
            weight,
            active_requests: AtomicUsize::new(0),
            ewma_latency_ms: AtomicEwma::new(0.3, 10.0),
            is_healthy: AtomicBool::new(true),
            consecutive_failures: AtomicUsize::new(0),
            total_requests: AtomicU64::new(0),
            current_swrr_weight: AtomicIsize::new(0),
        })
    }

    /// Increment active in-flight request counter.
    pub fn acquire(&self) {
        self.active_requests.fetch_add(1, Ordering::Relaxed);
        self.total_requests.fetch_add(1, Ordering::Relaxed);
    }

    /// Decrement active in-flight request counter and record observed round-trip latency.
    pub fn release(&self, latency_ms: f64) {
        self.active_requests.fetch_sub(1, Ordering::Relaxed);
        self.ewma_latency_ms.update(latency_ms);
    }

    /// Mark endpoint as healthy or unhealthy.
    pub fn set_healthy(&self, healthy: bool) {
        self.is_healthy.store(healthy, Ordering::Relaxed);
        if healthy {
            self.consecutive_failures.store(0, Ordering::Relaxed);
        }
    }

    /// Check if endpoint is eligible to receive traffic.
    pub fn is_available(&self) -> bool {
        self.is_healthy.load(Ordering::Relaxed)
    }

    /// Record a failure occurrence.
    pub fn record_failure(&self) -> usize {
        self.consecutive_failures.fetch_add(1, Ordering::Relaxed) + 1
    }

    /// Record a success occurrence, resetting consecutive failure counter.
    pub fn record_success(&self) {
        self.consecutive_failures.store(0, Ordering::Relaxed);
    }
}
