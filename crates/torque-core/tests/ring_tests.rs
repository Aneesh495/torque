use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use torque_core::ring::{spsc_channel, RingBuffer};

#[test]
fn test_ring_buffer_single_thread() {
    let ring = RingBuffer::new(4);
    assert!(ring.is_empty());
    assert_eq!(ring.capacity(), 4);

    assert!(ring.push(10).is_ok());
    assert!(ring.push(20).is_ok());
    assert!(ring.push(30).is_ok());
    assert!(ring.push(40).is_ok());

    // Buffer full
    assert_eq!(ring.push(50), Err(50));
    assert_eq!(ring.len(), 4);

    assert_eq!(ring.pop(), Some(10));
    assert_eq!(ring.pop(), Some(20));
    assert_eq!(ring.len(), 2);

    assert!(ring.push(60).is_ok());
    assert_eq!(ring.pop(), Some(30));
    assert_eq!(ring.pop(), Some(40));
    assert_eq!(ring.pop(), Some(60));
    assert_eq!(ring.pop(), None);
}

#[test]
fn test_ring_buffer_multi_thread_stress() {
    let ring = Arc::new(RingBuffer::new(1024));
    let num_producers = 4;
    let items_per_producer = 25_000;
    let total_items = num_producers * items_per_producer;

    let mut producer_handles = vec![];
    for p in 0..num_producers {
        let r = ring.clone();
        producer_handles.push(thread::spawn(move || {
            for i in 0..items_per_producer {
                let val = (p * 1_000_000) + i;
                while let Err(ret) = r.push(val) {
                    std::hint::spin_loop();
                    if r.push(ret).is_ok() {
                        break;
                    }
                }
            }
        }));
    }

    let consumed_count = Arc::new(AtomicUsize::new(0));
    let mut consumer_handles = vec![];
    for _ in 0..2 {
        let r = ring.clone();
        let c = consumed_count.clone();
        consumer_handles.push(thread::spawn(move || {
            while c.load(Ordering::Relaxed) < total_items {
                if let Some(_val) = r.pop() {
                    c.fetch_add(1, Ordering::Relaxed);
                } else {
                    std::hint::spin_loop();
                }
            }
        }));
    }

    for h in producer_handles {
        h.join().unwrap();
    }
    for h in consumer_handles {
        h.join().unwrap();
    }

    assert_eq!(consumed_count.load(Ordering::Relaxed), total_items);
}

#[test]
fn test_spsc_channel() {
    let (tx, rx) = spsc_channel::<usize>(128);
    let count = 50_000;

    let t1 = thread::spawn(move || {
        for i in 0..count {
            while let Err(val) = tx.push(i) {
                std::hint::spin_loop();
                if tx.push(val).is_ok() {
                    break;
                }
            }
        }
    });

    let t2 = thread::spawn(move || {
        let mut received = 0;
        while received < count {
            if let Some(val) = rx.pop() {
                assert_eq!(val, received);
                received += 1;
            } else {
                std::hint::spin_loop();
            }
        }
    });

    t1.join().unwrap();
    t2.join().unwrap();
}
