//! Biased Least Connections / Least Outstanding Requests load balancer.

use std::sync::atomic::Ordering;
use std::sync::Arc;
use torque_core::error::{Result, TorqueError};

use crate::endpoint::Endpoint;

/// Biased Least Connections load balancer.
pub struct LeastConnections {
    endpoints: Vec<Arc<Endpoint>>,
}

impl LeastConnections {
    pub fn new(endpoints: Vec<Arc<Endpoint>>) -> Self {
        Self { endpoints }
    }

    /// Select the healthy endpoint with the lowest ratio of active in-flight requests to weight.
    pub fn select(&self) -> Result<Arc<Endpoint>> {
        if self.endpoints.is_empty() {
            return Err(TorqueError::internal("No endpoints configured in cluster"));
        }

        let mut best: Option<&Arc<Endpoint>> = None;
        let mut best_score = f64::MAX;

        for ep in &self.endpoints {
            if !ep.is_available() {
                continue;
            }

            let active = ep.active_requests.load(Ordering::Relaxed) as f64;
            let weight = ep.weight.max(1) as f64;
            let score = active / weight;

            if score < best_score {
                best_score = score;
                best = Some(ep);
            }
        }

        match best {
            Some(ep) => Ok(ep.clone()),
            None => Err(TorqueError::UpstreamUnreachable {
                cluster: "cluster".to_string(),
                endpoint: "none".to_string(),
                detail: "All upstream endpoints are marked unhealthy".to_string(),
            }),
        }
    }
}
