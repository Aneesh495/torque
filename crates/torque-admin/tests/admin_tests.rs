//! Integration and unit tests for the administrative server.

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::watch;

use torque_admin::AdminServer;
use torque_config::schema::TorqueConfig;
use torque_config::watcher::ConfigManager;
use torque_metrics::registry::MetricsRegistry;
use torque_proto::http1::types::{Method, Request};

fn dummy_config() -> TorqueConfig {
    TorqueConfig::default()
}

#[tokio::test]
async fn test_admin_handle_request_health_and_probes() {
    let metrics = MetricsRegistry::new();
    let config = dummy_config();
    let mgr = ConfigManager::from_config(config, "dummy.yaml");
    let server = AdminServer::bind("127.0.0.1:0".parse().unwrap(), metrics.clone(), Some(mgr))
        .await
        .unwrap();

    // /healthz
    let req = Request::new(Method::Get, "/healthz");
    let resp = server.handle_request(&req);
    assert_eq!(resp.status, 200);
    assert!(String::from_utf8_lossy(&resp.body).contains("alive"));

    // /ready
    let req = Request::new(Method::Get, "/ready");
    let resp = server.handle_request(&req);
    assert_eq!(resp.status, 200);
    assert!(String::from_utf8_lossy(&resp.body).contains("ready"));

    // /dashboard
    let req = Request::new(Method::Get, "/dashboard");
    let resp = server.handle_request(&req);
    assert_eq!(resp.status, 200);
    assert!(String::from_utf8_lossy(&resp.body).contains("Torque Control Plane"));

    // 404
    let req = Request::new(Method::Get, "/unknown-path");
    let resp = server.handle_request(&req);
    assert_eq!(resp.status, 404);
}

#[tokio::test]
async fn test_admin_metrics_and_stats() {
    let metrics = MetricsRegistry::new();
    metrics.record_request(200, 15.2, 512, 1024);
    metrics.record_request(500, 42.1, 128, 64);
    metrics.inc_connection();

    let server = AdminServer::bind("127.0.0.1:0".parse().unwrap(), metrics.clone(), None)
        .await
        .unwrap();

    // /metrics (Prometheus text)
    let req = Request::new(Method::Get, "/metrics");
    let resp = server.handle_request(&req);
    assert_eq!(resp.status, 200);
    let prom_body = String::from_utf8_lossy(&resp.body);
    assert!(prom_body.contains("torque_requests_by_status{class=\"2xx\"} 1"));
    assert!(prom_body.contains("torque_requests_by_status{class=\"5xx\"} 1"));
    assert!(prom_body.contains("torque_active_connections 1"));

    // /stats (JSON snapshot)
    let req = Request::new(Method::Get, "/stats");
    let resp = server.handle_request(&req);
    assert_eq!(resp.status, 200);
    let json_body = String::from_utf8_lossy(&resp.body);
    assert!(json_body.contains("\"connections\":1"));
}

#[tokio::test]
async fn test_admin_live_tcp_exchange_and_shutdown() {
    let metrics = MetricsRegistry::new();
    let server = AdminServer::bind("127.0.0.1:0".parse().unwrap(), metrics.clone(), None)
        .await
        .unwrap();
    let addr = server.local_addr();

    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let server_task = tokio::spawn(async move {
        server.run(shutdown_rx).await.unwrap();
    });

    // Connect via real TCP stream
    let mut client = TcpStream::connect(addr).await.unwrap();
    let request_bytes = b"GET /healthz HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n";
    client.write_all(request_bytes).await.unwrap();

    let mut response_bytes = Vec::new();
    client.read_to_end(&mut response_bytes).await.unwrap();
    let response_str = String::from_utf8_lossy(&response_bytes);

    assert!(response_str.starts_with("HTTP/1.1 200 OK"));
    assert!(response_str.contains("{\"status\":\"alive\"}"));

    // Signal graceful shutdown
    shutdown_tx.send(true).unwrap();
    server_task.await.unwrap();
}
