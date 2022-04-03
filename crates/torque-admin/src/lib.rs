//! # Torque Admin
//!
//! Administrative REST endpoints, operational health checks, Prometheus telemetry
//! exposition, live configuration hot-reloading, and embedded web dashboard.

pub mod dashboard;
pub mod server;

pub use dashboard::DASHBOARD_HTML;
pub use server::AdminServer;

/// Admin prelude.
pub mod prelude {
    pub use crate::dashboard::DASHBOARD_HTML;
    pub use crate::server::AdminServer;
}
