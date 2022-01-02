//! Smooth Weighted Round Robin (SWRR) load balancing algorithm.
//!
//! Based on Nginx's smooth interleaving algorithm:
//! 1. For each endpoint: `current_weight += effective_weight`
//! 2. Select endpoint with largest `current_weight`
//! 3. Selected endpoint: `current_weight -= total_weight`

use parking_lot::Mutex;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use torque_core::error::{Result, TorqueError};

use crate::endpoint::Endpoint;

/// Smooth Weighted Round Robin load balancer.
pub struct SmoothWeightedRoundRobin {
    endpoints: Vec<Arc<Endpoint>>,
    total_weight: isize,
    lock: Mutex<()>,
}

impl SmoothWeightedRoundRobin {
    pub fn new(endpoints: Vec<Arc<Endpoint>>) -> Self {
        let total_weight: isize = endpoints.iter().map(|e| e.weight as isize).sum();
        Self {
            endpoints,
            total_weight,
            lock: Mutex::new(()),
        }
    }

    /// Select the next endpoint in the smooth weighted schedule.
    pub fn select(&self) -> Result<Arc<Endpoint>> {
        if self.endpoints.is_empty() {
            return Err(TorqueError::internal("No endpoints configured in cluster"));
        }

        let _guard = self.lock.lock();

        let mut best: Option<&Arc<Endpoint>> = None;
        let mut best_weight = isize::MIN;
        let mut available_count = 0;

        for ep in &self.endpoints {
            if !ep.is_available() {
                continue;
            }
            available_count += 1;

            let cur = ep
                .current_swrr_weight
                .fetch_add(ep.weight as isize, Ordering::Relaxed)
                + (ep.weight as isize);

            if cur > best_weight {
                best_weight = cur;
                best = Some(ep);
            }
        }

        if available_count == 0 {
            return Err(TorqueError::UpstreamUnreachable {
                cluster: "cluster".to_string(),
                endpoint: "none".to_string(),
                detail: "All upstream endpoints are marked unhealthy".to_string(),
            });
        }

        let selected = best.unwrap();
        selected
            .current_swrr_weight
            .fetch_sub(self.total_weight, Ordering::Relaxed);

        Ok(selected.clone())
    }
}
