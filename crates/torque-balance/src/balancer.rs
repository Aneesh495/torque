//! High-level cluster balancer interface and policy selection.

use std::sync::Arc;
use torque_core::error::Result;

use crate::algorithms::{
    KetamaRing, LeastConnections, MaglevTable, PeakEwma, SmoothWeightedRoundRobin,
};
use crate::endpoint::Endpoint;

/// Load balancing policy strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LbPolicy {
    RoundRobin,
    LeastConn,
    #[default]
    PeakEwma,
    Ketama,
    Maglev,
    Random,
}

/// Unified load balancer managing traffic dispatch across a cluster of endpoints.
pub struct ClusterBalancer {
    policy: LbPolicy,
    endpoints: Vec<Arc<Endpoint>>,
    swrr: SmoothWeightedRoundRobin,
    least_conn: LeastConnections,
    peak_ewma: PeakEwma,
    ketama: KetamaRing,
    maglev: MaglevTable,
}

impl ClusterBalancer {
    /// Construct a new ClusterBalancer for a set of endpoints under the given policy.
    pub fn new(endpoints: Vec<Arc<Endpoint>>, policy: LbPolicy) -> Self {
        Self {
            policy,
            endpoints: endpoints.clone(),
            swrr: SmoothWeightedRoundRobin::new(endpoints.clone()),
            least_conn: LeastConnections::new(endpoints.clone()),
            peak_ewma: PeakEwma::new(endpoints.clone()),
            ketama: KetamaRing::new(endpoints.clone()),
            maglev: MaglevTable::new(endpoints),
        }
    }

    /// Select an upstream endpoint based on configured policy and optional affinity key.
    pub fn select(&self, key: Option<&[u8]>) -> Result<Arc<Endpoint>> {
        match self.policy {
            LbPolicy::RoundRobin => self.swrr.select(),
            LbPolicy::LeastConn => self.least_conn.select(),
            LbPolicy::PeakEwma => self.peak_ewma.select(),
            LbPolicy::Ketama => {
                let default_key = b"default-routing-key";
                self.ketama.select(key.unwrap_or(default_key))
            }
            LbPolicy::Maglev => {
                let default_key = b"default-routing-key";
                self.maglev.select(key.unwrap_or(default_key))
            }
            LbPolicy::Random => self.peak_ewma.select(),
        }
    }

    /// Retrieve all endpoints in the cluster.
    pub fn endpoints(&self) -> &[Arc<Endpoint>] {
        &self.endpoints
    }
}
