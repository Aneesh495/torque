//! # Torque Upstream
//!
//! Upstream persistent connection pooling, keepalive lifecycle management,
//! and latency-measuring HTTP forwarder client.

pub mod client;
pub mod connection;
pub mod pool;

pub use client::UpstreamClient;
pub use connection::UpstreamConnection;
pub use pool::{ConnectionPool, PoolConfig};

/// Upstream prelude.
pub mod prelude {
    pub use crate::client::UpstreamClient;
    pub use crate::pool::{ConnectionPool, PoolConfig};
}
