use torque_metrics::access_log::AccessLogRecord;
use torque_metrics::histogram::HdrHistogram;
use torque_metrics::registry::MetricsRegistry;

#[test]
fn test_hdr_histogram_quantiles() {
    let hist = HdrHistogram::new();

    // Record 100 samples between 1ms and 50ms
    for _ in 0..50 {
        hist.record(2.0);
    }
    for _ in 0..40 {
        hist.record(10.0);
    }
    for _ in 0..10 {
        hist.record(50.0);
    }

    assert_eq!(hist.count(), 100);
    assert!(hist.quantile(0.50) <= 5.0);
    assert!(hist.quantile(0.90) <= 25.0);
    assert!(hist.quantile(0.99) <= 100.0);
}

#[test]
fn test_metrics_registry_recording_and_prometheus_export() {
    let registry = MetricsRegistry::new();

    registry.record_request(200, 1.5, 512, 128);
    registry.record_request(201, 2.0, 256, 64);
    registry.record_request(404, 0.8, 128, 32);
    registry.record_request(500, 50.0, 1024, 256);

    let prom = registry.export_prometheus();
    assert!(prom.contains("torque_requests_total 4"));
    assert!(prom.contains("torque_requests_by_status{class=\"2xx\"} 2"));
    assert!(prom.contains("torque_requests_by_status{class=\"4xx\"} 1"));
    assert!(prom.contains("torque_requests_by_status{class=\"5xx\"} 1"));
    assert!(prom.contains("torque_request_duration_ms_count 4"));
}

#[test]
fn test_metrics_json_export() {
    let registry = MetricsRegistry::new();
    registry.record_request(200, 5.0, 100, 50);

    let json = registry.export_json();
    assert!(json.contains("\"total\":1"));
    assert!(json.contains("\"2xx\":1"));
    assert!(json.contains("\"sent\":100"));
}

#[test]
fn test_access_log_record_formatting() {
    let rec = AccessLogRecord {
        client_ip: "192.168.1.100".to_string(),
        method: "POST".to_string(),
        uri: "/api/v1/orders".to_string(),
        status: 201,
        duration_ms: 4.25,
        bytes_sent: 1024,
        cluster: "order_cluster".to_string(),
        trace_id: Some("00-abcdef-1234-01".to_string()),
    };

    let line = rec.to_json_line();
    assert!(line.contains("\"client_ip\":\"192.168.1.100\""));
    assert!(line.contains("\"method\":\"POST\""));
    assert!(line.contains("\"status\":201"));
    assert!(line.contains("\"cluster\":\"order_cluster\""));
    assert!(line.contains("\"trace_id\":\"00-abcdef-1234-01\""));
}
