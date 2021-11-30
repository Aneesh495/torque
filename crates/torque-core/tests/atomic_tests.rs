use std::sync::atomic::Ordering;
use torque_core::atomic::{AtomicBitSet, AtomicEwma, AtomicF64};

#[test]
fn test_atomic_f64_ops() {
    let af = AtomicF64::new(10.5);
    assert_eq!(af.load(Ordering::Relaxed), 10.5);

    af.store(20.25, Ordering::Relaxed);
    assert_eq!(af.load(Ordering::Relaxed), 20.25);

    let old = af.fetch_add(5.75);
    assert_eq!(old, 20.25);
    assert_eq!(af.load(Ordering::Relaxed), 26.0);
}

#[test]
fn test_atomic_ewma() {
    let ewma = AtomicEwma::new(0.5, 100.0);
    assert_eq!(ewma.get(), 100.0);

    // sample 200.0 -> 0.5 * 200 + 0.5 * 100 = 150.0
    let res = ewma.update(200.0);
    assert_eq!(res, 150.0);
    assert_eq!(ewma.get(), 150.0);

    // sample 100.0 -> 0.5 * 100 + 0.5 * 150 = 125.0
    let res2 = ewma.update(100.0);
    assert_eq!(res2, 125.0);
}

#[test]
fn test_atomic_bitset() {
    let bitset = AtomicBitSet::new();
    assert_eq!(bitset.count_ones(), 0);
    assert!(!bitset.test(0));
    assert!(!bitset.test(7));

    bitset.set(0);
    bitset.set(7);
    bitset.set(63);

    assert!(bitset.test(0));
    assert!(bitset.test(7));
    assert!(bitset.test(63));
    assert!(!bitset.test(1));
    assert_eq!(bitset.count_ones(), 3);

    bitset.clear(7);
    assert!(!bitset.test(7));
    assert_eq!(bitset.count_ones(), 2);
}
