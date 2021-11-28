//! High-performance monotonic clock primitives and fast cached timestamps.
//!
//! Provides cached timestamp facilities for microsecond-scale request pipelines,
//! eliminating kernel clock syscall overhead across high-frequency event loops.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Global cached nanosecond monotonic offset from process start.
static START_INSTANT: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
static CACHED_MILLIS: AtomicU64 = AtomicU64::new(0);
static CACHED_NANOS: AtomicU64 = AtomicU64::new(0);

/// Initialize the monotonic clock anchor.
pub fn init_clock() {
    START_INSTANT.get_or_init(Instant::now);
    update_cached_time();
}

/// Update the cached timestamp. Typically called once per reactor event loop tick.
pub fn update_cached_time() {
    let anchor = START_INSTANT.get_or_init(Instant::now);
    let elapsed = anchor.elapsed();
    CACHED_NANOS.store(elapsed.as_nanos() as u64, Ordering::Relaxed);
    CACHED_MILLIS.store(elapsed.as_millis() as u64, Ordering::Relaxed);
}

/// Retrieve the cached elapsed time in nanoseconds since process initialization.
#[inline(always)]
pub fn fast_now_nanos() -> u64 {
    let nanos = CACHED_NANOS.load(Ordering::Relaxed);
    if nanos == 0 {
        let anchor = START_INSTANT.get_or_init(Instant::now);
        anchor.elapsed().as_nanos() as u64
    } else {
        nanos
    }
}

/// Retrieve the cached elapsed time in milliseconds since process initialization.
#[inline(always)]
pub fn fast_now_millis() -> u64 {
    let millis = CACHED_MILLIS.load(Ordering::Relaxed);
    if millis == 0 {
        let anchor = START_INSTANT.get_or_init(Instant::now);
        anchor.elapsed().as_millis() as u64
    } else {
        millis
    }
}

/// Fast monotonic time measurement wrapper.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FastInstant {
    nanos: u64,
}

impl FastInstant {
    /// Capture current cached instant.
    #[inline(always)]
    pub fn now() -> Self {
        Self {
            nanos: fast_now_nanos(),
        }
    }

    /// Exact high-resolution instant bypassing cache.
    pub fn exact() -> Self {
        let anchor = START_INSTANT.get_or_init(Instant::now);
        Self {
            nanos: anchor.elapsed().as_nanos() as u64,
        }
    }

    /// Calculate duration elapsed since this instant.
    pub fn elapsed(&self) -> Duration {
        let now = fast_now_nanos();
        let diff = now.saturating_sub(self.nanos);
        Duration::from_nanos(diff)
    }

    /// Raw nanoseconds relative to process epoch.
    pub fn as_nanos(&self) -> u64 {
        self.nanos
    }

    /// Convert to milliseconds relative to process epoch.
    pub fn as_millis(&self) -> u64 {
        self.nanos / 1_000_000
    }
}

/// Return current UNIX timestamp in seconds.
pub fn unix_timestamp_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_secs()
}

/// Return current UNIX timestamp in milliseconds.
pub fn unix_timestamp_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_millis() as u64
}
