//! Torque core error types and diagnostics.
//!
//! Provides a structured error hierarchy for the entire proxy substrate,
//! including diagnostic error codes, protocol violations, routing failures,
//! resilience trips, and upstream communication errors.

use std::fmt;
use thiserror::Error;

/// Root result type alias for Torque operations.
pub type Result<T> = std::result::Result<T, TorqueError>;

/// Standard error code categorization for runtime diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorCode {
    /// Generic I/O failure (E1001)
    IoError,
    /// Protocol frame or decoding violation (E2001)
    ProtocolViolation,
    /// HPACK compression or decompression error (E2002)
    HpackError,
    /// HTTP/2 stream state violation (E2003)
    Http2StreamError,
    /// WebSocket handshake or frame error (E2004)
    WebSocketError,
    /// Route not matched (E3001)
    RouteNotFound,
    /// Predicate evaluation failure (E3002)
    PredicateMismatch,
    /// Upstream host unreachable or connection refused (E4001)
    UpstreamUnreachable,
    /// Upstream response timed out (E4002)
    UpstreamTimeout,
    /// Upstream connection pool exhausted (E4003)
    PoolExhausted,
    /// Circuit breaker open, rejecting call (E5001)
    CircuitBreakerOpen,
    /// Rate limit exceeded (E5002)
    RateLimitExceeded,
    /// Concurrency limit reached, shed load (E5003)
    ConcurrencyLimitReached,
    /// Cache storage or invalidation failure (E6001)
    CacheError,
    /// Configuration parsing or validation error (E7001)
    ConfigurationError,
    /// Authentication or token verification failure (E8001)
    AuthenticationFailed,
    /// Internal unexpected failure (E9001)
    InternalError,
}

impl ErrorCode {
    /// Return the standardized alphanumeric error code string.
    pub fn as_str(&self) -> &'static str {
        match self {
            ErrorCode::IoError => "TORQUE-E1001",
            ErrorCode::ProtocolViolation => "TORQUE-E2001",
            ErrorCode::HpackError => "TORQUE-E2002",
            ErrorCode::Http2StreamError => "TORQUE-E2003",
            ErrorCode::WebSocketError => "TORQUE-E2004",
            ErrorCode::RouteNotFound => "TORQUE-E3001",
            ErrorCode::PredicateMismatch => "TORQUE-E3002",
            ErrorCode::UpstreamUnreachable => "TORQUE-E4001",
            ErrorCode::UpstreamTimeout => "TORQUE-E4002",
            ErrorCode::PoolExhausted => "TORQUE-E4003",
            ErrorCode::CircuitBreakerOpen => "TORQUE-E5001",
            ErrorCode::RateLimitExceeded => "TORQUE-E5002",
            ErrorCode::ConcurrencyLimitReached => "TORQUE-E5003",
            ErrorCode::CacheError => "TORQUE-E6001",
            ErrorCode::ConfigurationError => "TORQUE-E7001",
            ErrorCode::AuthenticationFailed => "TORQUE-E8001",
            ErrorCode::InternalError => "TORQUE-E9001",
        }
    }
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Unified error enumeration across all Torque subsystems.
#[derive(Debug, Error)]
pub enum TorqueError {
    /// Underlying low-level I/O error.
    #[error("[{code}] I/O failure: {source}", code = ErrorCode::IoError)]
    Io {
        #[from]
        source: std::io::Error,
    },

    /// Protocol syntax, framing, or decoding violation.
    #[error("[{code}] Protocol error: {detail}", code = ErrorCode::ProtocolViolation)]
    Protocol {
        detail: String,
        source: Option<Box<dyn std::error::Error + Send + Sync + 'static>>,
    },

    /// HPACK table compression or decompression failure.
    #[error("[{code}] HPACK error: {detail}", code = ErrorCode::HpackError)]
    Hpack { detail: String },

    /// HTTP/2 stream state machine violation.
    #[error("[{code}] HTTP/2 stream error (stream_id={stream_id}): {detail}", code = ErrorCode::Http2StreamError)]
    Http2Stream {
        stream_id: u32,
        error_code: u32,
        detail: String,
    },

    /// WebSocket frame format or handshake error.
    #[error("[{code}] WebSocket error: {detail}", code = ErrorCode::WebSocketError)]
    WebSocket { detail: String },

    /// Requested route does not match any prefix or pattern.
    #[error("[{code}] Route not found for path: {path}", code = ErrorCode::RouteNotFound)]
    RouteNotFound { path: String },

    /// Upstream host communication error.
    #[error("[{code}] Upstream failure ({cluster}/{endpoint}): {detail}", code = ErrorCode::UpstreamUnreachable)]
    UpstreamUnreachable {
        cluster: String,
        endpoint: String,
        detail: String,
    },

    /// Upstream response timed out.
    #[error("[{code}] Upstream request timed out after {timeout_ms}ms ({cluster})", code = ErrorCode::UpstreamTimeout)]
    UpstreamTimeout { cluster: String, timeout_ms: u64 },

    /// Upstream connection pool is at capacity.
    #[error("[{code}] Upstream connection pool exhausted for {cluster}", code = ErrorCode::PoolExhausted)]
    PoolExhausted { cluster: String },

    /// Circuit breaker is in Open state, blocking requests.
    #[error("[{code}] Circuit breaker open for cluster: {cluster}", code = ErrorCode::CircuitBreakerOpen)]
    CircuitBreakerOpen { cluster: String },

    /// Rate limit bucket exhausted.
    #[error("[{code}] Rate limit exceeded for key: {key} (retry after {retry_after_ms}ms)", code = ErrorCode::RateLimitExceeded)]
    RateLimitExceeded { key: String, retry_after_ms: u64 },

    /// Active concurrency threshold exceeded, request shed.
    #[error("[{code}] Concurrency limit reached ({current}/{max}) for cluster {cluster}", code = ErrorCode::ConcurrencyLimitReached)]
    ConcurrencyLimitReached {
        cluster: String,
        current: usize,
        max: usize,
    },

    /// Cache operation error.
    #[error("[{code}] Cache failure: {detail}", code = ErrorCode::CacheError)]
    Cache { detail: String },

    /// Configuration parsing, validation, or schema failure.
    #[error("[{code}] Configuration error in {section}: {detail}", code = ErrorCode::ConfigurationError)]
    Config { section: String, detail: String },

    /// Authentication, JWT, or signature verification failure.
    #[error("[{code}] Authentication error: {detail}", code = ErrorCode::AuthenticationFailed)]
    Auth { detail: String },

    /// Unspecified internal or invariant failure.
    #[error("[{code}] Internal invariant violation: {detail}", code = ErrorCode::InternalError)]
    Internal { detail: String },
}

impl TorqueError {
    /// Return the corresponding diagnostic error code.
    pub fn error_code(&self) -> ErrorCode {
        match self {
            TorqueError::Io { .. } => ErrorCode::IoError,
            TorqueError::Protocol { .. } => ErrorCode::ProtocolViolation,
            TorqueError::Hpack { .. } => ErrorCode::HpackError,
            TorqueError::Http2Stream { .. } => ErrorCode::Http2StreamError,
            TorqueError::WebSocket { .. } => ErrorCode::WebSocketError,
            TorqueError::RouteNotFound { .. } => ErrorCode::RouteNotFound,
            TorqueError::UpstreamUnreachable { .. } => ErrorCode::UpstreamUnreachable,
            TorqueError::UpstreamTimeout { .. } => ErrorCode::UpstreamTimeout,
            TorqueError::PoolExhausted { .. } => ErrorCode::PoolExhausted,
            TorqueError::CircuitBreakerOpen { .. } => ErrorCode::CircuitBreakerOpen,
            TorqueError::RateLimitExceeded { .. } => ErrorCode::RateLimitExceeded,
            TorqueError::ConcurrencyLimitReached { .. } => ErrorCode::ConcurrencyLimitReached,
            TorqueError::Cache { .. } => ErrorCode::CacheError,
            TorqueError::Config { .. } => ErrorCode::ConfigurationError,
            TorqueError::Auth { .. } => ErrorCode::AuthenticationFailed,
            TorqueError::Internal { .. } => ErrorCode::InternalError,
        }
    }

    /// Construct a new I/O error.
    pub fn io(err: std::io::Error) -> Self {
        TorqueError::Io { source: err }
    }

    /// Construct a new protocol error with message detail.
    pub fn protocol(msg: impl Into<String>) -> Self {
        TorqueError::Protocol {
            detail: msg.into(),
            source: None,
        }
    }

    /// Construct a new HPACK error with message detail.
    pub fn hpack(msg: impl Into<String>) -> Self {
        TorqueError::Hpack { detail: msg.into() }
    }

    /// Construct a new WebSocket error with message detail.
    pub fn websocket(msg: impl Into<String>) -> Self {
        TorqueError::WebSocket { detail: msg.into() }
    }

    /// Construct a new protocol error wrapping an underlying error source.
    pub fn protocol_with_source<E>(msg: impl Into<String>, source: E) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        TorqueError::Protocol {
            detail: msg.into(),
            source: Some(Box::new(source)),
        }
    }

    /// Construct a new configuration error.
    pub fn config(section: impl Into<String>, detail: impl Into<String>) -> Self {
        TorqueError::Config {
            section: section.into(),
            detail: detail.into(),
        }
    }

    /// Construct a new internal error.
    pub fn internal(detail: impl Into<String>) -> Self {
        TorqueError::Internal {
            detail: detail.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_code_mapping() {
        let err = TorqueError::protocol("bad framing");
        assert_eq!(err.error_code(), ErrorCode::ProtocolViolation);
        assert_eq!(err.error_code().as_str(), "TORQUE-E2001");

        let cb_err = TorqueError::CircuitBreakerOpen {
            cluster: "auth_service".to_string(),
        };
        assert_eq!(cb_err.error_code(), ErrorCode::CircuitBreakerOpen);
        assert_eq!(cb_err.error_code().as_str(), "TORQUE-E5001");
    }

    #[test]
    fn test_error_display_formatting() {
        let err = TorqueError::RateLimitExceeded {
            key: "192.168.1.1".to_string(),
            retry_after_ms: 500,
        };
        let formatted = format!("{err}");
        assert!(formatted.contains("TORQUE-E5002"));
        assert!(formatted.contains("192.168.1.1"));
        assert!(formatted.contains("500ms"));
    }
}
