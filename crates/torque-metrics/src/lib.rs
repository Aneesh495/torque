//! # Torque Metrics
//!
//! Zero-allocation Prometheus metrics registry, HDR logarithmic latency histograms,
//! and structured JSON access logs.

pub mod access_log;
pub mod histogram;
pub mod registry;

pub use access_log::AccessLogRecord;
pub use histogram::{HdrHistogram, BUCKET_BOUNDS_MS};
pub use registry::MetricsRegistry;

/// Metrics prelude.
pub mod prelude {
    pub use crate::access_log::AccessLogRecord;
    pub use crate::histogram::HdrHistogram;
    pub use crate::registry::MetricsRegistry;
}
