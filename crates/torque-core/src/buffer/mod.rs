//! Buffer and memory management primitives.

pub mod pool;
pub mod segmented;

pub use pool::{BytePool, PooledBuf, DEFAULT_CHUNK_SIZE};
pub use segmented::SegmentedBuf;
