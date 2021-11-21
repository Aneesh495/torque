//! Zero-allocation pooled byte buffer allocator.
//!
//! Pre-allocates slab arenas of uniform capacity to minimize dynamic heap allocations
//! during continuous high-throughput network stream processing.

use std::fmt;
use std::ops::{Deref, DerefMut};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use parking_lot::Mutex;

/// Default slab chunk size in bytes (16 KiB).
pub const DEFAULT_CHUNK_SIZE: usize = 16 * 1024;

/// Internal storage state shared between the pool and pooled buffers.
struct PoolInner {
    chunk_size: usize,
    max_pooled: usize,
    pool: Mutex<Vec<Vec<u8>>>,
    active_count: AtomicUsize,
    total_created: AtomicUsize,
    peak_active: AtomicUsize,
}

/// Thread-safe byte buffer pool managing pre-allocated memory slabs.
#[derive(Clone)]
pub struct BytePool {
    inner: Arc<PoolInner>,
}

impl BytePool {
    /// Create a new BytePool with standard 16 KiB chunk size and maximum capacity of 1024 chunks.
    pub fn new() -> Self {
        Self::with_config(DEFAULT_CHUNK_SIZE, 1024)
    }

    /// Create a new BytePool with custom chunk size and maximum cached slab count.
    pub fn with_config(chunk_size: usize, max_pooled: usize) -> Self {
        assert!(chunk_size > 0, "Chunk size must be greater than zero");
        Self {
            inner: Arc::new(PoolInner {
                chunk_size,
                max_pooled,
                pool: Mutex::new(Vec::with_capacity(max_pooled.min(64))),
                active_count: AtomicUsize::new(0),
                total_created: AtomicUsize::new(0),
                peak_active: AtomicUsize::new(0),
            }),
        }
    }

    /// Acquire a pre-allocated buffer from the pool, or allocate a new one if pool is empty.
    pub fn acquire(&self) -> PooledBuf {
        let raw = {
            let mut guard = self.inner.pool.lock();
            guard.pop()
        };

        let data = match raw {
            Some(mut vec) => {
                vec.clear();
                vec
            }
            None => {
                self.inner.total_created.fetch_add(1, Ordering::Relaxed);
                Vec::with_capacity(self.inner.chunk_size)
            }
        };

        let prev = self.inner.active_count.fetch_add(1, Ordering::Relaxed);
        let current = prev + 1;
        
        // Track peak concurrent allocations
        let mut peak = self.inner.peak_active.load(Ordering::Relaxed);
        while current > peak {
            match self.inner.peak_active.compare_exchange_weak(
                peak,
                current,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(actual) => peak = actual,
            }
        }

        PooledBuf {
            data,
            pool: Some(self.clone()),
        }
    }

    /// Return the fixed chunk size configured for this pool.
    pub fn chunk_size(&self) -> usize {
        self.inner.chunk_size
    }

    /// Number of actively checked out buffers.
    pub fn active_count(&self) -> usize {
        self.inner.active_count.load(Ordering::Relaxed)
    }

    /// Number of idle buffers waiting in the pool.
    pub fn pooled_count(&self) -> usize {
        self.inner.pool.lock().len()
    }

    /// Total number of raw allocations created over the lifetime of the pool.
    pub fn total_created(&self) -> usize {
        self.inner.total_created.load(Ordering::Relaxed)
    }

    /// Historical peak number of concurrent active buffers.
    pub fn peak_active(&self) -> usize {
        self.inner.peak_active.load(Ordering::Relaxed)
    }

    /// Return a raw vector back into the pool.
    fn release(&self, mut vec: Vec<u8>) {
        self.inner.active_count.fetch_sub(1, Ordering::Relaxed);
        if vec.capacity() >= self.inner.chunk_size {
            let mut guard = self.inner.pool.lock();
            if guard.len() < self.inner.max_pooled {
                vec.clear();
                guard.push(vec);
            }
        }
    }
}

impl Default for BytePool {
    fn default() -> Self {
        Self::new()
    }
}

/// RAII wrapper for a pooled buffer returning to the allocator on drop.
pub struct PooledBuf {
    data: Vec<u8>,
    pool: Option<BytePool>,
}

impl PooledBuf {
    /// Wrap an unpooled standalone vector for testing or detached operations.
    pub fn standalone(vec: Vec<u8>) -> Self {
        Self {
            data: vec,
            pool: None,
        }
    }

    /// Append a slice of bytes to the buffer.
    pub fn extend_from_slice(&mut self, other: &[u8]) {
        self.data.extend_from_slice(other);
    }

    /// Clear contents while preserving underlying memory capacity.
    pub fn clear(&mut self) {
        self.data.clear();
    }

    /// Return the allocated memory capacity in bytes.
    pub fn capacity(&self) -> usize {
        self.data.capacity()
    }

    /// Current populated length in bytes.
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Check if buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Extract the underlying raw `Vec<u8>`, detaching from pool recycling.
    pub fn into_inner(mut self) -> Vec<u8> {
        self.pool = None;
        std::mem::take(&mut self.data)
    }
}

impl Deref for PooledBuf {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl DerefMut for PooledBuf {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.data
    }
}

impl AsRef<[u8]> for PooledBuf {
    fn as_ref(&self) -> &[u8] {
        &self.data
    }
}

impl AsMut<[u8]> for PooledBuf {
    fn as_mut(&mut self) -> &mut [u8] {
        &mut self.data
    }
}

impl Drop for PooledBuf {
    fn drop(&mut self) {
        if let Some(pool) = self.pool.take() {
            let vec = std::mem::take(&mut self.data);
            pool.release(vec);
        }
    }
}

impl fmt::Debug for PooledBuf {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "PooledBuf {{ len: {}, capacity: {}, pooled: {} }}",
            self.data.len(),
            self.data.capacity(),
            self.pool.is_some()
        )
    }
}
