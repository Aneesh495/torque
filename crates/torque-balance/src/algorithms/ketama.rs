//! Ketama consistent hashing with virtual node distribution ring.

use std::collections::BTreeMap;
use std::sync::Arc;
use torque_core::error::{Result, TorqueError};

use crate::endpoint::Endpoint;

/// Number of virtual nodes mapped per endpoint weight unit.
const VNODES_PER_WEIGHT: usize = 160;

/// Ketama consistent hash ring.
pub struct KetamaRing {
    endpoints: Vec<Arc<Endpoint>>,
    ring: BTreeMap<u32, Arc<Endpoint>>,
}

impl KetamaRing {
    /// Construct a Ketama hash ring from a list of endpoints.
    pub fn new(endpoints: Vec<Arc<Endpoint>>) -> Self {
        let mut ring = BTreeMap::new();

        for ep in &endpoints {
            let vnode_count = (ep.weight as usize) * VNODES_PER_WEIGHT;
            for i in 0..vnode_count {
                let token = format!("{}-vnode-{}", ep.addr, i);
                let hash = hash_key(token.as_bytes());
                ring.insert(hash, ep.clone());
            }
        }

        Self { endpoints, ring }
    }

    /// Return reference to configured endpoints.
    pub fn endpoints(&self) -> &[Arc<Endpoint>] {
        &self.endpoints
    }

    /// Select endpoint based on hash key.
    pub fn select(&self, key: &[u8]) -> Result<Arc<Endpoint>> {
        if self.ring.is_empty() {
            return Err(TorqueError::internal("Ketama ring is empty"));
        }

        let hash = hash_key(key);

        // Find the first endpoint whose ring point is >= hash (clockwise traversal)
        let matched = self
            .ring
            .range(hash..)
            .find(|(_, ep)| ep.is_available())
            .or_else(|| self.ring.iter().find(|(_, ep)| ep.is_available()));

        match matched {
            Some((_, ep)) => Ok(ep.clone()),
            None => Err(TorqueError::UpstreamUnreachable {
                cluster: "cluster".to_string(),
                endpoint: "none".to_string(),
                detail: "All upstream endpoints in hash ring are marked unhealthy".to_string(),
            }),
        }
    }
}

/// FNV-1a 32-bit hash function.
fn hash_key(bytes: &[u8]) -> u32 {
    let mut hash: u32 = 0x811c9dc5;
    for &b in bytes {
        hash ^= b as u32;
        hash = hash.wrapping_mul(0x01000193);
    }
    hash
}
