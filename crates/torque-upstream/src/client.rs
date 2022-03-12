//! High-level upstream HTTP client and round-trip dispatcher.

use std::sync::Arc;
use std::time::{Duration, Instant};
use torque_balance::endpoint::Endpoint;
use torque_core::error::Result;
use torque_proto::http1::{Request, Response};

use super::pool::{ConnectionPool, PoolConfig};

/// High-level client for dispatching requests to upstream endpoints.
pub struct UpstreamClient {
    pool: Arc<ConnectionPool>,
}

impl UpstreamClient {
    pub fn new(pool_config: PoolConfig) -> Self {
        Self {
            pool: ConnectionPool::new(pool_config),
        }
    }

    /// Forward an HTTP request to the selected upstream endpoint.
    pub async fn forward(
        &self,
        endpoint: &Arc<Endpoint>,
        req: &Request,
        timeout_dur: Duration,
    ) -> Result<Response> {
        endpoint.acquire();
        let start = Instant::now();

        let mut conn = match self.pool.acquire(endpoint.addr).await {
            Ok(c) => c,
            Err(e) => {
                endpoint.release(start.elapsed().as_secs_f64() * 1000.0);
                endpoint.record_failure();
                return Err(e);
            }
        };

        match conn.exchange(req, timeout_dur).await {
            Ok(resp) => {
                let elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;
                endpoint.release(elapsed_ms);
                endpoint.record_success();
                self.pool.release(conn);
                Ok(resp)
            }
            Err(e) => {
                let elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;
                endpoint.release(elapsed_ms);
                endpoint.record_failure();
                Err(e)
            }
        }
    }

    /// Access the underlying connection pool.
    pub fn pool(&self) -> &Arc<ConnectionPool> {
        &self.pool
    }
}
