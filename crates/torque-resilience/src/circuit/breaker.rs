//! Production-grade 4-state circuit breaker with sliding window metrics.

use parking_lot::RwLock;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use torque_core::error::{Result, TorqueError};
use torque_core::time::fast_now_millis;

use super::state::CircuitState;
use super::window::SlidingWindow;

/// Circuit breaker policy configuration.
#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    pub failure_rate_threshold: f64,
    pub slow_call_rate_threshold: f64,
    pub slow_call_duration_ms: u64,
    pub min_request_volume: usize,
    pub window_duration_ms: u64,
    pub recovery_timeout_ms: u64,
    pub half_open_probes: usize,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_rate_threshold: 0.50,
            slow_call_rate_threshold: 0.80,
            slow_call_duration_ms: 1000,
            min_request_volume: 20,
            window_duration_ms: 10_000,
            recovery_timeout_ms: 5_000,
            half_open_probes: 5,
        }
    }
}

/// Dynamic circuit breaker guarding upstream service interactions.
pub struct CircuitBreaker {
    name: String,
    config: CircuitBreakerConfig,
    state: RwLock<CircuitState>,
    window: SlidingWindow,
    opened_at_millis: AtomicU64,
    half_open_active_probes: AtomicUsize,
    half_open_success_count: AtomicUsize,
    half_open_failure_count: AtomicUsize,
}

impl CircuitBreaker {
    /// Construct a new circuit breaker with given configuration.
    pub fn new(name: impl Into<String>, config: CircuitBreakerConfig) -> Self {
        let window_duration = config.window_duration_ms;
        Self {
            name: name.into(),
            config,
            state: RwLock::new(CircuitState::Closed),
            window: SlidingWindow::new(window_duration),
            opened_at_millis: AtomicU64::new(0),
            half_open_active_probes: AtomicUsize::new(0),
            half_open_success_count: AtomicUsize::new(0),
            half_open_failure_count: AtomicUsize::new(0),
        }
    }

    /// Query whether a new request is permitted to proceed.
    pub fn allow_request(&self) -> Result<()> {
        let state = *self.state.read();
        match state {
            CircuitState::Closed => Ok(()),
            CircuitState::ForcedOpen => Err(TorqueError::CircuitBreakerOpen {
                cluster: self.name.clone(),
            }),
            CircuitState::Open => {
                let opened_at = self.opened_at_millis.load(Ordering::Relaxed);
                let now = fast_now_millis();
                if now >= opened_at + self.config.recovery_timeout_ms {
                    // Transition to HalfOpen
                    let mut lock = self.state.write();
                    if *lock == CircuitState::Open {
                        *lock = CircuitState::HalfOpen;
                        self.half_open_active_probes.store(1, Ordering::Relaxed);
                        self.half_open_success_count.store(0, Ordering::Relaxed);
                        self.half_open_failure_count.store(0, Ordering::Relaxed);
                        return Ok(());
                    }
                }
                Err(TorqueError::CircuitBreakerOpen {
                    cluster: self.name.clone(),
                })
            }
            CircuitState::HalfOpen => {
                let active = self.half_open_active_probes.fetch_add(1, Ordering::Relaxed);
                if active < self.config.half_open_probes {
                    Ok(())
                } else {
                    self.half_open_active_probes.fetch_sub(1, Ordering::Relaxed);
                    Err(TorqueError::CircuitBreakerOpen {
                        cluster: self.name.clone(),
                    })
                }
            }
        }
    }

    /// Record a successful request completion.
    pub fn on_success(&self, duration_ms: u64) {
        let is_slow = duration_ms >= self.config.slow_call_duration_ms;
        self.window.record(false, is_slow);

        let state = *self.state.read();
        if state == CircuitState::HalfOpen {
            self.half_open_active_probes.fetch_sub(1, Ordering::Relaxed);
            let successes = self.half_open_success_count.fetch_add(1, Ordering::Relaxed) + 1;
            if successes >= self.config.half_open_probes {
                // Trial succeeded: reset to Closed
                let mut lock = self.state.write();
                if *lock == CircuitState::HalfOpen {
                    *lock = CircuitState::Closed;
                    self.window.clear();
                }
            }
        }
    }

    /// Record a failed request completion (e.g. 5xx or connection timeout).
    pub fn on_failure(&self, duration_ms: u64) {
        let is_slow = duration_ms >= self.config.slow_call_duration_ms;
        self.window.record(true, is_slow);

        let state = *self.state.read();
        if state == CircuitState::HalfOpen {
            // Any failure during HalfOpen trips breaker back to Open
            self.trip_open();
            return;
        }

        if state == CircuitState::Closed {
            let snap = self.window.snapshot();
            if snap.total >= self.config.min_request_volume {
                if snap.failure_rate >= self.config.failure_rate_threshold
                    || snap.slow_rate >= self.config.slow_call_rate_threshold
                {
                    self.trip_open();
                }
            }
        }
    }

    /// Transition to Open state.
    fn trip_open(&self) {
        let mut lock = self.state.write();
        *lock = CircuitState::Open;
        self.opened_at_millis
            .store(fast_now_millis(), Ordering::Relaxed);
        self.half_open_active_probes.store(0, Ordering::Relaxed);
    }

    /// Retrieve current state of the circuit breaker.
    pub fn state(&self) -> CircuitState {
        *self.state.read()
    }

    /// Administratively force circuit breaker into Closed state.
    pub fn force_close(&self) {
        let mut lock = self.state.write();
        *lock = CircuitState::Closed;
        self.window.clear();
    }

    /// Administratively force circuit breaker into ForcedOpen state.
    pub fn force_open(&self) {
        let mut lock = self.state.write();
        *lock = CircuitState::ForcedOpen;
    }
}
