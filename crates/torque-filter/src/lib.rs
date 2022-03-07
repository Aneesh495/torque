//! # Torque Filter Pipeline
//!
//! Pluggable filter architecture, JWT verification, API key authentication,
//! CORS preflight management, W3C distributed traceparent propagation,
//! security headers, and response compression.

pub mod auth;
pub mod compression;
pub mod pipeline;
pub mod security;
pub mod tracing;

pub use auth::{ApiKeyFilter, JwtAuthFilter};
pub use compression::CompressionFilter;
pub use pipeline::{Filter, FilterAction, FilterChain, FilterContext, FilterStage};
pub use security::{CorsConfig, CorsFilter, SecurityHeadersFilter};
pub use tracing::TraceparentFilter;

/// Filter prelude.
pub mod prelude {
    pub use crate::auth::{ApiKeyFilter, JwtAuthFilter};
    pub use crate::compression::CompressionFilter;
    pub use crate::pipeline::{Filter, FilterAction, FilterChain, FilterContext, FilterStage};
    pub use crate::security::{CorsConfig, CorsFilter, SecurityHeadersFilter};
    pub use crate::tracing::TraceparentFilter;
}
