//! Lightweight gossip synchronization protocol for distributed rate coordinate aggregation.

use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;

/// Compact binary gossip frame representing local token consumption deltas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GossipSyncMessage {
    pub node_id: u32,
    pub sequence: u64,
    pub deltas: Vec<(u64, u32)>, // (key_hash, consumed_count)
}

impl GossipSyncMessage {
    /// Serialize message into a compact binary byte vector.
    pub fn encode(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(16 + (self.deltas.len() * 12));
        buf.extend_from_slice(&self.node_id.to_be_bytes());
        buf.extend_from_slice(&self.sequence.to_be_bytes());
        buf.extend_from_slice(&(self.deltas.len() as u32).to_be_bytes());
        for (hash, count) in &self.deltas {
            buf.extend_from_slice(&hash.to_be_bytes());
            buf.extend_from_slice(&count.to_be_bytes());
        }
        buf
    }

    /// Deserialize message from a binary slice.
    pub fn decode(src: &[u8]) -> Option<Self> {
        if src.len() < 16 {
            return None;
        }
        let node_id = u32::from_be_bytes(src[..4].try_into().unwrap());
        let sequence = u64::from_be_bytes(src[4..12].try_into().unwrap());
        let count = u32::from_be_bytes(src[12..16].try_into().unwrap()) as usize;

        let mut cursor = 16;
        let mut deltas = Vec::with_capacity(count);
        for _ in 0..count {
            if src.len() < cursor + 12 {
                return None;
            }
            let hash = u64::from_be_bytes(src[cursor..cursor + 8].try_into().unwrap());
            let delta = u32::from_be_bytes(src[cursor + 8..cursor + 12].try_into().unwrap());
            cursor += 12;
            deltas.push((hash, delta));
        }

        Some(Self {
            node_id,
            sequence,
            deltas,
        })
    }
}

/// Gossip state aggregator tracking cluster-wide remote consumption.
pub struct GossipAggregator {
    remote_deltas: RwLock<HashMap<u64, u64>>,
}

impl GossipAggregator {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            remote_deltas: RwLock::new(HashMap::new()),
        })
    }

    /// Ingest a gossip message received from a remote cluster peer.
    pub fn ingest(&self, msg: &GossipSyncMessage) {
        let mut map = self.remote_deltas.write();
        for &(hash, count) in &msg.deltas {
            *map.entry(hash).or_insert(0) += count as u64;
        }
    }

    /// Retrieve aggregated remote consumption for a given key hash.
    pub fn get_remote_consumption(&self, key_hash: u64) -> u64 {
        let map = self.remote_deltas.read();
        map.get(&key_hash).copied().unwrap_or(0)
    }

    /// Reset window deltas.
    pub fn clear(&self) {
        self.remote_deltas.write().clear();
    }
}
