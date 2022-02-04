use std::time::Duration;
use torque_resilience::circuit::{CircuitBreaker, CircuitBreakerConfig, CircuitState};
use torque_resilience::concurrency::ConcurrencyLimiter;
use torque_resilience::ratelimit::{
    GossipAggregator, GossipSyncMessage, LeakyBucket, SlidingWindowLimiter, TokenBucket,
};
use torque_resilience::retry::{Backoff, JitterStrategy, RetryBudget};

#[test]
fn test_circuit_breaker_trip_on_errors() {
    let config = CircuitBreakerConfig {
        failure_rate_threshold: 0.50,
        slow_call_rate_threshold: 0.90,
        slow_call_duration_ms: 1000,
        min_request_volume: 10,
        window_duration_ms: 5000,
        recovery_timeout_ms: 50,
        half_open_probes: 2,
    };

    let cb = CircuitBreaker::new("payment_service", config);
    assert_eq!(cb.state(), CircuitState::Closed);

    // 4 successes, 6 failures -> 60% failure rate (above 50% threshold)
    for _ in 0..4 {
        assert!(cb.allow_request().is_ok());
        cb.on_success(50);
    }
    for _ in 0..6 {
        assert!(cb.allow_request().is_ok());
        cb.on_failure(50);
    }

    // Circuit should now be tripped Open
    assert_eq!(cb.state(), CircuitState::Open);
    assert!(cb.allow_request().is_err());

    // Wait for recovery timeout
    std::thread::sleep(Duration::from_millis(60));

    // First request after recovery timeout transitions to HalfOpen
    assert!(cb.allow_request().is_ok());
    assert_eq!(cb.state(), CircuitState::HalfOpen);

    // Complete trial probe successes -> resets to Closed
    cb.on_success(50);
    assert!(cb.allow_request().is_ok());
    cb.on_success(50);
    assert_eq!(cb.state(), CircuitState::Closed);
}

#[test]
fn test_token_bucket_burst_and_exhaustion() {
    // 10 tokens/sec, burst capacity of 5
    let bucket = TokenBucket::new(10, 5);

    // Consume all 5 burst tokens
    for _ in 0..5 {
        assert!(bucket.try_acquire(1).is_ok());
    }

    // 6th immediate acquire should fail
    let err = bucket.try_acquire(1);
    assert!(err.is_err());
}

#[test]
fn test_leaky_bucket_outflow() {
    // capacity = 3, leak rate = 10 units/sec (1 unit every 100ms)
    let bucket = LeakyBucket::new(3, 10);

    assert!(bucket.try_acquire(1).is_ok());
    assert!(bucket.try_acquire(1).is_ok());
    assert!(bucket.try_acquire(1).is_ok());
    // 4th unit overflows capacity
    assert!(bucket.try_acquire(1).is_err());

    // Wait 150ms for at least 1 unit to leak
    std::thread::sleep(Duration::from_millis(150));
    assert!(bucket.try_acquire(1).is_ok());
}

#[test]
fn test_sliding_window_limiter() {
    // 100ms window, limit = 5
    let limiter = SlidingWindowLimiter::new(100, 5);

    for _ in 0..5 {
        assert!(limiter.try_acquire(1).is_ok());
    }
    assert!(limiter.try_acquire(1).is_err());
}

#[test]
fn test_gossip_sync_roundtrip() {
    let msg = GossipSyncMessage {
        node_id: 42,
        sequence: 1001,
        deltas: vec![(0x1234_5678_9ABC_DEF0, 5), (0xAABB_CCDD_EEFF_0011, 12)],
    };

    let encoded = msg.encode();
    let decoded = GossipSyncMessage::decode(&encoded).expect("decode failed");
    assert_eq!(decoded, msg);

    let agg = GossipAggregator::new();
    agg.ingest(&decoded);
    assert_eq!(agg.get_remote_consumption(0x1234_5678_9ABC_DEF0), 5);
    assert_eq!(agg.get_remote_consumption(0xAABB_CCDD_EEFF_0011), 12);
    assert_eq!(agg.get_remote_consumption(0x9999), 0);
}

#[test]
fn test_jittered_backoff() {
    let backoff = Backoff::new(10, 1000, JitterStrategy::Full);
    let prev = Duration::from_millis(10);

    for attempt in 0..5 {
        let dur = backoff.calculate(attempt, prev);
        assert!(dur <= Duration::from_millis(1000));
    }
}

#[test]
fn test_retry_budget_enforcement() {
    // max 10 tokens, 20% retry ratio
    let budget = RetryBudget::new(10, 0.20);
    assert_eq!(budget.available_tokens(), 10);

    // Consume all 10 retry tokens
    for _ in 0..10 {
        assert!(budget.try_acquire_retry());
    }
    // 11th retry denied
    assert!(!budget.try_acquire_retry());

    // 5 regular client requests deposit: 5 * 0.20 = 1.0 retry token
    for _ in 0..5 {
        budget.record_request();
    }
    assert!(budget.try_acquire_retry());
    assert!(!budget.try_acquire_retry());
}

#[test]
fn test_concurrency_limiter_scaling() {
    let limiter = ConcurrencyLimiter::new("test_cluster", 10.0, 2.0, 50.0);
    assert_eq!(limiter.limit(), 10);

    // Normal fast latencies (10ms) -> limit increases
    for _ in 0..5 {
        limiter.release(10.0);
    }
    assert!(limiter.limit() >= 10);

    // Huge latency spike (500ms) -> limit sheds downward
    for _ in 0..10 {
        limiter.release(500.0);
    }
    assert!(limiter.limit() < 10);
    assert!(limiter.limit() >= 2); // respects min_limit
}
