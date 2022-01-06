//! Power of Two Random Choices (P2C) with Peak-EWMA latency weighting.
//!
//! Evaluates two candidate endpoints chosen at random and routes traffic to the
//! host with lower latency-inflight product:
//! `score = ewma_latency_ms * (active_requests + 1) / weight`.

use rand::seq::SliceRandom;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use torque_core::error::{Result, TorqueError};

use crate::endpoint::Endpoint;

/// P2C Peak-EWMA load balancer.
pub struct PeakEwma {
    endpoints: Vec<Arc<Endpoint>>,
}

impl PeakEwma {
    pub fn new(endpoints: Vec<Arc<Endpoint>>) -> Self {
        Self { endpoints }
    }

    /// Select the best endpoint between two random candidates.
    pub fn select(&self) -> Result<Arc<Endpoint>> {
        let available: Vec<&Arc<Endpoint>> =
            self.endpoints.iter().filter(|e| e.is_available()).collect();

        if available.is_empty() {
            return Err(TorqueError::UpstreamUnreachable {
                cluster: "cluster".to_string(),
                endpoint: "none".to_string(),
                detail: "All upstream endpoints are marked unhealthy".to_string(),
            });
        }

        if available.len() == 1 {
            return Ok((*available[0]).clone());
        }

        let mut rng = rand::thread_rng();
        let candidates = available.choose_multiple(&mut rng, 2).collect::<Vec<_>>();
        let ep1 = *candidates[0];
        let ep2 = *candidates[1];

        let score1 = calc_score(ep1);
        let score2 = calc_score(ep2);

        if score1 <= score2 {
            Ok(ep1.clone())
        } else {
            Ok(ep2.clone())
        }
    }
}

fn calc_score(ep: &Endpoint) -> f64 {
    let ewma = ep.ewma_latency_ms.get().max(0.1);
    let active = ep.active_requests.load(Ordering::Relaxed) as f64;
    let weight = ep.weight.max(1) as f64;
    (ewma * (active + 1.0)) / weight
}
