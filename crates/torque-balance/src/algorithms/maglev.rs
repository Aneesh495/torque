//! Google Maglev consistent hashing algorithm.
//!
//! Generates prime-modulo permutation sequences for deterministic O(1) host lookup
//! with minimal disruption during endpoint membership churn.

use std::sync::Arc;
use torque_core::error::{Result, TorqueError};

use crate::endpoint::Endpoint;

/// Default prime table size for Maglev distribution table.
const DEFAULT_MAGLEV_M: usize = 65537;

/// Maglev consistent hash lookup table.
pub struct MaglevTable {
    endpoints: Vec<Arc<Endpoint>>,
    lookup_table: Vec<usize>,
    m: usize,
}

impl MaglevTable {
    /// Construct Maglev lookup table from endpoint list.
    pub fn new(endpoints: Vec<Arc<Endpoint>>) -> Self {
        let m = DEFAULT_MAGLEV_M;
        if endpoints.is_empty() {
            return Self {
                endpoints,
                lookup_table: Vec::new(),
                m,
            };
        }

        let n = endpoints.len();
        let mut permutations = Vec::with_capacity(n);

        // Generate offset and skip for each endpoint
        for ep in &endpoints {
            let addr_str = ep.addr.to_string();
            let h1 = hash1(addr_str.as_bytes()) % m;
            let h2 = (hash2(addr_str.as_bytes()) % (m - 1)) + 1; // skip must be > 0
            permutations.push((h1, h2));
        }

        let mut lookup_table = vec![usize::MAX; m];
        let mut next = vec![0usize; n];
        let mut filled = 0;

        // Fill lookup table round-robin across permutations
        while filled < m {
            for i in 0..n {
                let (offset, skip) = permutations[i];
                let mut cur = (offset + next[i] * skip) % m;
                while lookup_table[cur] != usize::MAX {
                    next[i] += 1;
                    cur = (offset + next[i] * skip) % m;
                }
                lookup_table[cur] = i;
                next[i] += 1;
                filled += 1;
                if filled == m {
                    break;
                }
            }
        }

        Self {
            endpoints,
            lookup_table,
            m,
        }
    }

    /// Select endpoint in O(1) time using hash key.
    pub fn select(&self, key: &[u8]) -> Result<Arc<Endpoint>> {
        if self.endpoints.is_empty() {
            return Err(TorqueError::internal("Maglev table has zero endpoints"));
        }

        let hash = hash1(key);
        let start_idx = hash % self.m;

        // Linear probe if selected endpoint is unhealthy
        for i in 0..self.endpoints.len() {
            let idx = (start_idx + i) % self.m;
            let ep_idx = self.lookup_table[idx];
            let ep = &self.endpoints[ep_idx];
            if ep.is_available() {
                return Ok(ep.clone());
            }
        }

        Err(TorqueError::UpstreamUnreachable {
            cluster: "cluster".to_string(),
            endpoint: "none".to_string(),
            detail: "All upstream endpoints in Maglev table are marked unhealthy".to_string(),
        })
    }
}

fn hash1(bytes: &[u8]) -> usize {
    let mut hash: usize = 0xcbf29ce484222325;
    for &b in bytes {
        hash ^= b as usize;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn hash2(bytes: &[u8]) -> usize {
    let mut hash: usize = 0x84222325cbf29ce4;
    for &b in bytes {
        hash ^= (b as usize).rotate_left(3);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}
