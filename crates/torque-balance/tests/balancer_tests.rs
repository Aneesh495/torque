use std::net::SocketAddr;
use std::time::Duration;
use torque_balance::algorithms::{
    KetamaRing, LeastConnections, MaglevTable, PeakEwma, SmoothWeightedRoundRobin,
};
use torque_balance::endpoint::Endpoint;
use torque_balance::health::{EndpointHealthTracker, OutlierConfig, OutlierDetector};

fn make_endpoint(ip: &str, port: u16, weight: u32) -> std::sync::Arc<Endpoint> {
    let addr: SocketAddr = format!("{}:{}", ip, port).parse().unwrap();
    Endpoint::new(addr, weight)
}

#[test]
fn test_swrr_weight_distribution() {
    let ep_a = make_endpoint("10.0.0.1", 8080, 5);
    let ep_b = make_endpoint("10.0.0.2", 8080, 1);
    let ep_c = make_endpoint("10.0.0.3", 8080, 1);

    let swrr = SmoothWeightedRoundRobin::new(vec![ep_a.clone(), ep_b.clone(), ep_c.clone()]);

    let mut counts = std::collections::HashMap::new();
    // 7 requests: expect A=5, B=1, C=1
    for _ in 0..7 {
        let ep = swrr.select().expect("select failed");
        *counts.entry(ep.addr).or_insert(0) += 1;
    }

    assert_eq!(counts.get(&ep_a.addr), Some(&5));
    assert_eq!(counts.get(&ep_b.addr), Some(&1));
    assert_eq!(counts.get(&ep_c.addr), Some(&1));
}

#[test]
fn test_least_connections_dispatch() {
    let ep1 = make_endpoint("10.0.1.1", 80, 10);
    let ep2 = make_endpoint("10.0.1.2", 80, 10);

    // Simulate active requests
    ep1.acquire();
    ep1.acquire(); // ep1 active = 2
    ep2.acquire(); // ep2 active = 1

    let lb = LeastConnections::new(vec![ep1.clone(), ep2.clone()]);
    let selected = lb.select().expect("select failed");
    assert_eq!(selected.addr, ep2.addr);
}

#[test]
fn test_peak_ewma_selection() {
    let ep1 = make_endpoint("10.0.2.1", 80, 10);
    let ep2 = make_endpoint("10.0.2.2", 80, 10);

    // Set ep1 latency = 1ms, ep2 latency = 50ms
    ep1.ewma_latency_ms.update(1.0);
    ep2.ewma_latency_ms.update(50.0);

    let lb = PeakEwma::new(vec![ep1.clone(), ep2.clone()]);
    // Over multiple choices, ep1 should be favored
    let mut ep1_wins = 0;
    for _ in 0..20 {
        let s = lb.select().unwrap();
        if s.addr == ep1.addr {
            ep1_wins += 1;
        }
    }
    assert!(ep1_wins >= 15);
}

#[test]
fn test_ketama_consistency() {
    let ep1 = make_endpoint("10.0.3.1", 80, 1);
    let ep2 = make_endpoint("10.0.3.2", 80, 1);

    let ring = KetamaRing::new(vec![ep1, ep2]);

    let key = b"user-session-12345";
    let first = ring.select(key).unwrap().addr;
    for _ in 0..10 {
        assert_eq!(ring.select(key).unwrap().addr, first);
    }
}

#[test]
fn test_maglev_consistency() {
    let ep1 = make_endpoint("10.0.4.1", 80, 1);
    let ep2 = make_endpoint("10.0.4.2", 80, 1);

    let table = MaglevTable::new(vec![ep1, ep2]);

    let key = b"tenant-abc-7788";
    let first = table.select(key).unwrap().addr;
    for _ in 0..10 {
        assert_eq!(table.select(key).unwrap().addr, first);
    }
}

#[test]
fn test_health_tracker_state_transitions() {
    let ep = make_endpoint("10.0.5.1", 80, 1);
    let tracker = EndpointHealthTracker::new(ep.clone());
    assert!(ep.is_available());

    // 2 failures with threshold=3 -> still healthy
    tracker.on_failure(3);
    tracker.on_failure(3);
    assert!(ep.is_available());

    // 3rd failure -> marked unhealthy
    tracker.on_failure(3);
    assert!(!ep.is_available());

    // 1 success with threshold=2 -> still unhealthy
    tracker.on_success(2);
    assert!(!ep.is_available());

    // 2nd success -> restored to healthy
    tracker.on_success(2);
    assert!(ep.is_available());
}

#[test]
fn test_outlier_detector_ejection() {
    let ep = make_endpoint("10.0.6.1", 80, 1);
    let config = OutlierConfig {
        consecutive_5xx_threshold: 3,
        base_ejection_duration: Duration::from_millis(50),
        max_ejection_percent: 50,
    };
    let detector = OutlierDetector::new(ep.clone(), config);
    assert!(ep.is_available());

    detector.on_error();
    detector.on_error();
    assert!(ep.is_available());

    detector.on_error(); // 3rd error triggers ejection
    assert!(!ep.is_available());

    // Wait for duration
    std::thread::sleep(Duration::from_millis(60));
    assert!(detector.check_expiration());
    assert!(ep.is_available());
}
