//! Circuit breaker state machine enumeration.

/// Operational states of a circuit breaker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    /// Normal operation: all requests are forwarded to upstreams.
    Closed,
    /// Recovery trial: a limited number of probe requests are permitted.
    HalfOpen,
    /// Fault protection: calls are rejected immediately without hitting upstreams.
    Open,
    /// Administratively forced open: overrides automatic recovery.
    ForcedOpen,
}

impl CircuitState {
    pub fn as_str(&self) -> &'static str {
        match self {
            CircuitState::Closed => "CLOSED",
            CircuitState::HalfOpen => "HALF_OPEN",
            CircuitState::Open => "OPEN",
            CircuitState::ForcedOpen => "FORCED_OPEN",
        }
    }
}
