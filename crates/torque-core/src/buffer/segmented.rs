//! Chained segmented buffer for zero-copy stream processing and vectored I/O.
//!
//! Allows aggregating scattered byte buffers without copying, advancing read cursors,
//! and exposing slices to vectored system calls (`writev`).

use bytes::{Buf, Bytes};
use std::collections::VecDeque;
use std::io::IoSlice;

/// Non-contiguous buffer holding a chain of byte segments.
#[derive(Default, Debug, Clone)]
pub struct SegmentedBuf {
    segments: VecDeque<Bytes>,
    total_len: usize,
}

impl SegmentedBuf {
    /// Construct a new empty SegmentedBuf.
    pub fn new() -> Self {
        Self {
            segments: VecDeque::new(),
            total_len: 0,
        }
    }

    /// Append a new byte segment to the end of the chain.
    pub fn push_back(&mut self, chunk: Bytes) {
        if !chunk.is_empty() {
            self.total_len += chunk.len();
            self.segments.push_back(chunk);
        }
    }

    /// Prepend a byte segment to the front of the chain.
    pub fn push_front(&mut self, chunk: Bytes) {
        if !chunk.is_empty() {
            self.total_len += chunk.len();
            self.segments.push_front(chunk);
        }
    }

    /// Total number of readable bytes across all segments.
    pub fn len(&self) -> usize {
        self.total_len
    }

    /// Whether the segmented buffer has zero readable bytes.
    pub fn is_empty(&self) -> bool {
        self.total_len == 0
    }

    /// Number of active non-contiguous segments currently stored.
    pub fn segment_count(&self) -> usize {
        self.segments.len()
    }

    /// Clear all segments and reset length to zero.
    pub fn clear(&mut self) {
        self.segments.clear();
        self.total_len = 0;
    }

    /// Extract continuous linear bytes into a destination slice.
    pub fn copy_to_slice(&self, dst: &mut [u8]) {
        let mut offset = 0;
        for seg in &self.segments {
            if offset >= dst.len() {
                break;
            }
            let to_copy = (dst.len() - offset).min(seg.len());
            dst[offset..offset + to_copy].copy_from_slice(&seg[..to_copy]);
            offset += to_copy;
        }
    }

    /// Flatten all segments into a single contiguous `Bytes` instance.
    pub fn to_bytes(&self) -> Bytes {
        if self.segments.is_empty() {
            Bytes::new()
        } else if self.segments.len() == 1 {
            self.segments[0].clone()
        } else {
            let mut vec = Vec::with_capacity(self.total_len);
            for seg in &self.segments {
                vec.extend_from_slice(seg);
            }
            Bytes::from(vec)
        }
    }

    /// Populate an array of `IoSlice` references for vectored `writev` transmission.
    pub fn fill_io_slices<'a>(&'a self, dst: &mut [IoSlice<'a>]) -> usize {
        let count = self.segments.len().min(dst.len());
        for (i, seg) in self.segments.iter().take(count).enumerate() {
            dst[i] = IoSlice::new(seg);
        }
        count
    }
}

impl Buf for SegmentedBuf {
    fn remaining(&self) -> usize {
        self.total_len
    }

    fn chunk(&self) -> &[u8] {
        match self.segments.front() {
            Some(b) => b.as_ref(),
            None => &[],
        }
    }

    fn advance(&mut self, mut cnt: usize) {
        assert!(cnt <= self.total_len, "Cannot advance beyond total length");
        self.total_len -= cnt;

        while cnt > 0 && !self.segments.is_empty() {
            let front_len = self.segments[0].len();
            if cnt >= front_len {
                self.segments.pop_front();
                cnt -= front_len;
            } else {
                let front = self.segments.pop_front().unwrap();
                self.segments.push_front(front.slice(cnt..));
                cnt = 0;
            }
        }
    }
}
