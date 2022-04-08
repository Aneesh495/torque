//! # Torque Server
//!
//! Production-grade Layer-7 reverse proxy and service mesh data plane daemon.
//! Integrates non-blocking I/O event loops, zero-copy protocol translation,
//! route resolution, adaptive replacement caching, circuit breaking,
//! rate limiting, and zero-downtime hot reloading.

pub mod dispatcher;
pub mod reactor;
pub mod server;

pub use dispatcher::RequestDispatcher;
pub use reactor::TcpReactor;
pub use server::TorqueServer;

/// Server prelude.
pub mod prelude {
    pub use crate::dispatcher::RequestDispatcher;
    pub use crate::reactor::TcpReactor;
    pub use crate::server::TorqueServer;
}
