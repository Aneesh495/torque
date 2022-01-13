//! # Torque Balance
//!
//! High-throughput load balancing algorithms (Smooth Weighted Round Robin,
//! Least Connections, Peak-EWMA P2C, Ketama, Maglev) and health checking.

pub mod algorithms;
pub mod balancer;
pub mod endpoint;
pub mod health;

pub use algorithms::{
    KetamaRing, LeastConnections, MaglevTable, PeakEwma, SmoothWeightedRoundRobin,
};
pub use balancer::{ClusterBalancer, LbPolicy};
pub use endpoint::Endpoint;
pub use health::{
    EndpointHealthTracker, HealthCheckConfig, OutlierConfig, OutlierDetector, ProbeProtocol,
};

/// Balancer prelude.
pub mod prelude {
    pub use crate::balancer::{ClusterBalancer, LbPolicy};
    pub use crate::endpoint::Endpoint;
    pub use crate::health::{HealthCheckConfig, OutlierConfig};
}
