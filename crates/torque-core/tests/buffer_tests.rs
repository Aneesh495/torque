use bytes::Buf;
use torque_core::buffer::{BytePool, SegmentedBuf};

#[test]
fn test_byte_pool_acquisition_and_recycling() {
    let pool = BytePool::with_config(4096, 8);
    assert_eq!(pool.chunk_size(), 4096);
    assert_eq!(pool.active_count(), 0);
    assert_eq!(pool.pooled_count(), 0);

    {
        let mut buf1 = pool.acquire();
        assert_eq!(buf1.capacity(), 4096);
        assert_eq!(pool.active_count(), 1);
        buf1.extend_from_slice(b"hello torque");
        assert_eq!(&buf1[..], b"hello torque");
    }

    // After drop, buffer is recycled back into the pool
    assert_eq!(pool.active_count(), 0);
    assert_eq!(pool.pooled_count(), 1);

    // Second acquire should reuse recycled buffer without reallocating
    {
        let mut buf2 = pool.acquire();
        assert_eq!(buf2.len(), 0);
        assert_eq!(buf2.capacity(), 4096);
        assert_eq!(pool.active_count(), 1);
        assert_eq!(pool.pooled_count(), 0);
        buf2.extend_from_slice(b"reused slab");
    }

    assert_eq!(pool.active_count(), 0);
    assert_eq!(pool.pooled_count(), 1);
}

#[test]
fn test_segmented_buf_operations() {
    let mut sbuf = SegmentedBuf::new();
    assert!(sbuf.is_empty());
    assert_eq!(sbuf.len(), 0);

    sbuf.push_back(bytes::Bytes::from_static(b"GET /api/v1 "));
    sbuf.push_back(bytes::Bytes::from_static(b"HTTP/1.1\r\n"));
    sbuf.push_back(bytes::Bytes::from_static(b"Host: example.com\r\n\r\n"));

    assert_eq!(sbuf.segment_count(), 3);
    assert_eq!(sbuf.len(), 43);

    let flat = sbuf.to_bytes();
    assert_eq!(&flat[..12], b"GET /api/v1 ");

    // Advance 4 bytes ("GET ")
    sbuf.advance(4);
    assert_eq!(sbuf.len(), 39);
    assert_eq!(sbuf.chunk(), b"/api/v1 ");

    // Copy to slice
    let mut dst = vec![0u8; 8];
    sbuf.copy_to_slice(&mut dst);
    assert_eq!(&dst, b"/api/v1 ");
}
