//! Health probing and outlier ejection.

pub mod outlier;
pub mod probe;

pub use outlier::{OutlierConfig, OutlierDetector};
pub use probe::{EndpointHealthTracker, HealthCheckConfig, ProbeProtocol};
