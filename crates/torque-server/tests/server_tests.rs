//! End-to-end and integration tests for the Torque proxy server.

use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use torque_config::schema::{
    AdminConfig, CacheConfig, ClusterConfig, EndpointConfig, RouteConfig, ServerConfig,
    TorqueConfig,
};
use torque_proto::http1::types::{Method, Request};
use torque_server::TorqueServer;

/// Spawn a mock upstream HTTP/1.1 server on an ephemeral port.
async fn spawn_mock_upstream(counter: Arc<AtomicUsize>) -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        loop {
            if let Ok((mut stream, _)) = listener.accept().await {
                let cnt = counter.clone();
                tokio::spawn(async move {
                    let mut buf = vec![0u8; 4096];
                    if let Ok(n) = stream.read(&mut buf).await {
                        if n > 0 {
                            cnt.fetch_add(1, Ordering::Relaxed);
                            let body = b"{\"message\":\"Hello from mock upstream\"}\n";
                            let resp = format!(
                                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                                body.len()
                            );
                            let _ = stream.write_all(resp.as_bytes()).await;
                            let _ = stream.write_all(body).await;
                            let _ = stream.flush().await;
                        }
                    }
                });
            }
        }
    });

    addr
}

#[tokio::test]
async fn test_server_dispatch_and_security_headers() {
    let upstream_counter = Arc::new(AtomicUsize::new(0));
    let upstream_addr = spawn_mock_upstream(upstream_counter.clone()).await;

    let config = TorqueConfig {
        server: ServerConfig {
            bind_addr: "127.0.0.1:0".to_string(),
            worker_threads: 2,
            max_connections: 1000,
            tcp_nodelay: true,
            buffer_size_kb: 64,
        },
        admin: AdminConfig {
            bind_addr: "127.0.0.1:0".to_string(),
            enable_dashboard: true,
            metrics_path: "/metrics".to_string(),
        },
        clusters: vec![ClusterConfig {
            name: "mock-cluster".to_string(),
            lb_policy: "round_robin".to_string(),
            endpoints: vec![EndpointConfig {
                addr: upstream_addr.to_string(),
                weight: 10,
            }],
            health_check: None,
            circuit_breaker: None,
        }],
        routes: vec![RouteConfig {
            name: "items-route".to_string(),
            path: "/api/items".to_string(),
            methods: vec!["GET".to_string()],
            cluster: "mock-cluster".to_string(),
            timeout_ms: Some(2000),
            rate_limit: None,
            cache: None,
            plugins: Vec::new(),
        }],
    };

    let server = TorqueServer::from_config(config, None).await.unwrap();

    // 1. Matched Route
    let req = Request::new(Method::Get, "/api/items");
    let client_addr: SocketAddr = "127.0.0.1:55000".parse().unwrap();
    let resp = server.dispatcher().dispatch(req, client_addr).await;

    assert_eq!(resp.status, 200);
    assert_eq!(upstream_counter.load(Ordering::Relaxed), 1);
    assert_eq!(
        resp.headers.get("x-content-type-options"),
        Some("nosniff")
    );
    assert_eq!(resp.headers.get("x-frame-options"), Some("DENY"));
    assert!(String::from_utf8_lossy(&resp.body).contains("Hello from mock upstream"));

    // 2. Unmatched Route (404)
    let req404 = Request::new(Method::Get, "/unknown-endpoint");
    let resp404 = server.dispatcher().dispatch(req404, client_addr).await;
    assert_eq!(resp404.status, 404);
}

#[tokio::test]
async fn test_server_caching_behavior() {
    let upstream_counter = Arc::new(AtomicUsize::new(0));
    let upstream_addr = spawn_mock_upstream(upstream_counter.clone()).await;

    let config = TorqueConfig {
        server: ServerConfig {
            bind_addr: "127.0.0.1:0".to_string(),
            worker_threads: 2,
            max_connections: 1000,
            tcp_nodelay: true,
            buffer_size_kb: 64,
        },
        admin: AdminConfig::default(),
        clusters: vec![ClusterConfig {
            name: "cache-cluster".to_string(),
            lb_policy: "round_robin".to_string(),
            endpoints: vec![EndpointConfig {
                addr: upstream_addr.to_string(),
                weight: 10,
            }],
            health_check: None,
            circuit_breaker: None,
        }],
        routes: vec![RouteConfig {
            name: "cached-route".to_string(),
            path: "/data".to_string(),
            methods: vec!["GET".to_string()],
            cluster: "cache-cluster".to_string(),
            timeout_ms: Some(2000),
            rate_limit: None,
            cache: Some(CacheConfig {
                enabled: true,
                ttl_seconds: 60,
            }),
            plugins: Vec::new(),
        }],
    };

    let server = TorqueServer::from_config(config, None).await.unwrap();
    let client_addr: SocketAddr = "127.0.0.1:55001".parse().unwrap();

    // First request: Cache Miss -> forwards to upstream
    let req1 = Request::new(Method::Get, "/data");
    let resp1 = server.dispatcher().dispatch(req1, client_addr).await;
    assert_eq!(resp1.status, 200);
    assert_eq!(upstream_counter.load(Ordering::Relaxed), 1);

    // Second request: Cache Hit -> served from ARC, does not touch upstream
    let req2 = Request::new(Method::Get, "/data");
    let resp2 = server.dispatcher().dispatch(req2, client_addr).await;
    assert_eq!(resp2.status, 200);
    assert_eq!(upstream_counter.load(Ordering::Relaxed), 1);
    assert_eq!(resp2.headers.get("x-cache"), Some("HIT"));
}

#[tokio::test]
async fn test_server_live_tcp_reactor_exchange() {
    let upstream_counter = Arc::new(AtomicUsize::new(0));
    let upstream_addr = spawn_mock_upstream(upstream_counter.clone()).await;

    let config = TorqueConfig {
        server: ServerConfig {
            bind_addr: "127.0.0.1:0".to_string(),
            worker_threads: 2,
            max_connections: 1000,
            tcp_nodelay: true,
            buffer_size_kb: 64,
        },
        admin: AdminConfig {
            bind_addr: String::new(), // Disable admin for this test
            enable_dashboard: false,
            metrics_path: "/metrics".to_string(),
        },
        clusters: vec![ClusterConfig {
            name: "tcp-cluster".to_string(),
            lb_policy: "round_robin".to_string(),
            endpoints: vec![EndpointConfig {
                addr: upstream_addr.to_string(),
                weight: 10,
            }],
            health_check: None,
            circuit_breaker: None,
        }],
        routes: vec![RouteConfig {
            name: "live-route".to_string(),
            path: "/live-call".to_string(),
            methods: vec!["GET".to_string()],
            cluster: "tcp-cluster".to_string(),
            timeout_ms: Some(2000),
            rate_limit: None,
            cache: None,
            plugins: Vec::new(),
        }],
    };

    let server = TorqueServer::from_config(config, None).await.unwrap();
    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

    let reactor = torque_server::TcpReactor::bind(
        "127.0.0.1:0".parse().unwrap(),
        server.dispatcher().clone(),
        server.metrics().clone(),
        true,
        64,
    )
    .await
    .unwrap();
    let proxy_addr = reactor.local_addr();

    let reactor_task = tokio::spawn(async move {
        reactor.run(shutdown_rx).await.unwrap();
    });

    // Connect via real client TCP stream
    let mut client = tokio::net::TcpStream::connect(proxy_addr).await.unwrap();
    let req_bytes = b"GET /live-call HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n";
    client.write_all(req_bytes).await.unwrap();

    let mut resp_bytes = Vec::new();
    client.read_to_end(&mut resp_bytes).await.unwrap();
    let resp_str = String::from_utf8_lossy(&resp_bytes);

    assert!(resp_str.starts_with("HTTP/1.1 200 OK"));
    assert!(resp_str.contains("Hello from mock upstream"));
    assert_eq!(upstream_counter.load(Ordering::Relaxed), 1);

    // Shutdown reactor
    shutdown_tx.send(true).unwrap();
    reactor_task.await.unwrap();
}
