//! Asynchronous connection pool for upstream HTTP/1.1 keepalive reuse.

use parking_lot::Mutex;
use std::collections::{HashMap, VecDeque};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use torque_core::error::Result;

use super::connection::UpstreamConnection;

/// Connection pool configuration parameters.
#[derive(Debug, Clone)]
pub struct PoolConfig {
    pub max_idle_per_host: usize,
    pub idle_timeout: Duration,
    pub connect_timeout: Duration,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            max_idle_per_host: 32,
            idle_timeout: Duration::from_secs(60),
            connect_timeout: Duration::from_millis(1000),
        }
    }
}

/// Thread-safe upstream connection pool.
pub struct ConnectionPool {
    config: PoolConfig,
    idle_conns: Mutex<HashMap<SocketAddr, VecDeque<UpstreamConnection>>>,
}

impl ConnectionPool {
    /// Construct a new connection pool.
    pub fn new(config: PoolConfig) -> Arc<Self> {
        Arc::new(Self {
            config,
            idle_conns: Mutex::new(HashMap::new()),
        })
    }

    /// Acquire a warm connection from the idle pool or establish a new TCP socket.
    pub async fn acquire(&self, addr: SocketAddr) -> Result<UpstreamConnection> {
        // Try pop warm idle connection
        {
            let mut guard = self.idle_conns.lock();
            if let Some(queue) = guard.get_mut(&addr) {
                while let Some(conn) = queue.pop_front() {
                    if !conn.is_expired(self.config.idle_timeout) {
                        return Ok(conn);
                    }
                    // Connection expired; dropped here
                }
            }
        }

        // Establish new connection
        UpstreamConnection::connect(addr, self.config.connect_timeout).await
    }

    /// Return an active connection to the idle pool for reuse.
    pub fn release(&self, conn: UpstreamConnection) {
        if conn.is_expired(self.config.idle_timeout) {
            return;
        }

        let mut guard = self.idle_conns.lock();
        let queue = guard.entry(conn.addr).or_default();
        if queue.len() < self.config.max_idle_per_host {
            queue.push_back(conn);
        }
    }

    /// Prune dead or expired connections across all host pools.
    pub fn prune_idle(&self) -> usize {
        let mut guard = self.idle_conns.lock();
        let mut pruned = 0;
        let timeout = self.config.idle_timeout;

        for queue in guard.values_mut() {
            let before = queue.len();
            queue.retain(|c| !c.is_expired(timeout));
            pruned += before - queue.len();
        }

        pruned
    }
}
