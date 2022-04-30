//! Lock-free ring buffer implementations for low-latency thread communication.
//!
//! Includes a Multi-Producer Multi-Consumer (MPMC) queue based on Vyukov bounded ring
//! sequence counters, alongside a Single-Producer Single-Consumer (SPSC) ring.

use std::cell::UnsafeCell;
use std::mem::MaybeUninit;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

/// Cache line size for modern 64-bit microarchitectures (64 bytes).
const CACHE_LINE_SIZE: usize = 64;

/// Internal slot structure holding sequence generation number and payload.
struct Cell<T> {
    sequence: AtomicUsize,
    value: UnsafeCell<MaybeUninit<T>>,
}

/// Bounded Multi-Producer Multi-Consumer (MPMC) lock-free ring buffer.
pub struct RingBuffer<T> {
    buffer: Box<[Cell<T>]>,
    mask: usize,
    capacity: usize,
    enqueue_pos: AtomicUsize,
    _pad0: [u8; CACHE_LINE_SIZE],
    dequeue_pos: AtomicUsize,
    _pad1: [u8; CACHE_LINE_SIZE],
}

// Safety: RingBuffer is thread-safe for Send types.
unsafe impl<T: Send> Send for RingBuffer<T> {}
unsafe impl<T: Send> Sync for RingBuffer<T> {}

impl<T> RingBuffer<T> {
    /// Construct a new bounded ring buffer. Capacity is rounded up to the nearest power of two.
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 1, "Capacity must be greater than one");
        let capacity = capacity.next_power_of_two();
        let mask = capacity - 1;

        let mut cells = Vec::with_capacity(capacity);
        for i in 0..capacity {
            cells.push(Cell {
                sequence: AtomicUsize::new(i),
                value: UnsafeCell::new(MaybeUninit::uninit()),
            });
        }

        Self {
            buffer: cells.into_boxed_slice(),
            mask,
            capacity,
            enqueue_pos: AtomicUsize::new(0),
            _pad0: [0; CACHE_LINE_SIZE],
            dequeue_pos: AtomicUsize::new(0),
            _pad1: [0; CACHE_LINE_SIZE],
        }
    }

    /// Push an element into the ring buffer.
    /// Returns `Err(value)` if the buffer is currently full.
    pub fn push(&self, value: T) -> Result<(), T> {
        let mut pos = self.enqueue_pos.load(Ordering::Relaxed);
        loop {
            let cell = &self.buffer[pos & self.mask];
            let seq = cell.sequence.load(Ordering::Acquire);
            let dif = seq as isize - pos as isize;

            if dif == 0 {
                match self.enqueue_pos.compare_exchange_weak(
                    pos,
                    pos + 1,
                    Ordering::Relaxed,
                    Ordering::Relaxed,
                ) {
                    Ok(_) => {
                        unsafe {
                            (*cell.value.get()).write(value);
                        }
                        cell.sequence.store(pos + 1, Ordering::Release);
                        return Ok(());
                    }
                    Err(actual) => pos = actual,
                }
            } else if dif < 0 {
                // Buffer is full
                return Err(value);
            } else {
                pos = self.enqueue_pos.load(Ordering::Relaxed);
            }
        }
    }

    /// Pop an element from the ring buffer.
    /// Returns `None` if the buffer is currently empty.
    pub fn pop(&self) -> Option<T> {
        let mut pos = self.dequeue_pos.load(Ordering::Relaxed);
        loop {
            let cell = &self.buffer[pos & self.mask];
            let seq = cell.sequence.load(Ordering::Acquire);
            let dif = seq as isize - (pos + 1) as isize;

            if dif == 0 {
                match self.dequeue_pos.compare_exchange_weak(
                    pos,
                    pos + 1,
                    Ordering::Relaxed,
                    Ordering::Relaxed,
                ) {
                    Ok(_) => {
                        let value = unsafe { (*cell.value.get()).assume_init_read() };
                        cell.sequence.store(pos + self.mask + 1, Ordering::Release);
                        return Some(value);
                    }
                    Err(actual) => pos = actual,
                }
            } else if dif < 0 {
                // Buffer is empty
                return None;
            } else {
                pos = self.dequeue_pos.load(Ordering::Relaxed);
            }
        }
    }

    /// Current approximate number of items waiting in the ring buffer.
    pub fn len(&self) -> usize {
        let enq = self.enqueue_pos.load(Ordering::Relaxed);
        let deq = self.dequeue_pos.load(Ordering::Relaxed);
        enq.saturating_sub(deq)
    }

    /// Check if the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Maximum item capacity of the ring buffer.
    pub fn capacity(&self) -> usize {
        self.capacity
    }
}

impl<T> Drop for RingBuffer<T> {
    fn drop(&mut self) {
        while self.pop().is_some() {}
    }
}

/// Producer half of an SPSC lock-free channel.
pub struct SpscProducer<T> {
    inner: Arc<SpscInner<T>>,
}

/// Consumer half of an SPSC lock-free channel.
pub struct SpscConsumer<T> {
    inner: Arc<SpscInner<T>>,
}

struct SpscInner<T> {
    buffer: Box<[UnsafeCell<MaybeUninit<T>>]>,
    capacity: usize,
    mask: usize,
    head: AtomicUsize,
    _pad0: [u8; CACHE_LINE_SIZE],
    tail: AtomicUsize,
    _pad1: [u8; CACHE_LINE_SIZE],
}

unsafe impl<T: Send> Send for SpscInner<T> {}
unsafe impl<T: Send> Sync for SpscInner<T> {}

/// Create a bounded Single-Producer Single-Consumer (SPSC) channel pair.
pub fn spsc_channel<T>(capacity: usize) -> (SpscProducer<T>, SpscConsumer<T>) {
    let capacity = capacity.next_power_of_two();
    let mask = capacity - 1;

    let mut buffer = Vec::with_capacity(capacity);
    for _ in 0..capacity {
        buffer.push(UnsafeCell::new(MaybeUninit::uninit()));
    }

    let inner = Arc::new(SpscInner {
        buffer: buffer.into_boxed_slice(),
        capacity,
        mask,
        head: AtomicUsize::new(0),
        _pad0: [0; CACHE_LINE_SIZE],
        tail: AtomicUsize::new(0),
        _pad1: [0; CACHE_LINE_SIZE],
    });

    (
        SpscProducer {
            inner: inner.clone(),
        },
        SpscConsumer { inner },
    )
}

impl<T> SpscProducer<T> {
    /// Attempt to enqueue an item without blocking.
    pub fn push(&self, value: T) -> Result<(), T> {
        let head = self.inner.head.load(Ordering::Relaxed);
        let tail = self.inner.tail.load(Ordering::Acquire);

        if head.wrapping_sub(tail) >= self.inner.capacity {
            return Err(value);
        }

        let idx = head & self.inner.mask;
        unsafe {
            (*self.inner.buffer[idx].get()).write(value);
        }
        self.inner
            .head
            .store(head.wrapping_add(1), Ordering::Release);
        Ok(())
    }
}

impl<T> SpscConsumer<T> {
    /// Attempt to dequeue an item without blocking.
    pub fn pop(&self) -> Option<T> {
        let tail = self.inner.tail.load(Ordering::Relaxed);
        let head = self.inner.head.load(Ordering::Acquire);

        if tail == head {
            return None;
        }

        let idx = tail & self.inner.mask;
        let value = unsafe { (*self.inner.buffer[idx].get()).assume_init_read() };
        self.inner
            .tail
            .store(tail.wrapping_add(1), Ordering::Release);
        Some(value)
    }
}

impl<T> Drop for SpscInner<T> {
    fn drop(&mut self) {
        let mut tail = self.tail.load(Ordering::Relaxed);
        let head = self.head.load(Ordering::Relaxed);
        while tail != head {
            let idx = tail & self.mask;
            unsafe {
                (*self.buffer[idx].get()).assume_init_drop();
            }
            tail = tail.wrapping_add(1);
        }
    }
}
