//! # Torque Core
//!
//! Foundational memory, concurrency, time, and diagnostic primitives for the Torque
//! proxy and service mesh substrate.

pub mod atomic;
pub mod buffer;
pub mod error;
pub mod ring;
pub mod time;

pub use atomic::{AtomicBitSet, AtomicEwma, AtomicF64};
pub use buffer::{BytePool, PooledBuf, SegmentedBuf, DEFAULT_CHUNK_SIZE};
pub use error::{ErrorCode, Result, TorqueError};
pub use ring::{spsc_channel, RingBuffer, SpscConsumer, SpscProducer};
pub use time::{
    fast_now_millis, fast_now_nanos, init_clock, unix_timestamp_millis, unix_timestamp_secs,
    update_cached_time, FastInstant,
};

/// Prelude re-exporting common core abstractions.
pub mod prelude {
    pub use crate::atomic::{AtomicBitSet, AtomicEwma, AtomicF64};
    pub use crate::buffer::{BytePool, PooledBuf, SegmentedBuf};
    pub use crate::error::{ErrorCode, Result, TorqueError};
    pub use crate::ring::{spsc_channel, RingBuffer};
    pub use crate::time::{fast_now_millis, fast_now_nanos, FastInstant};
}
