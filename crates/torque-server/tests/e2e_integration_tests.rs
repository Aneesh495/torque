//! Comprehensive end-to-end integration tests: failover, hot-reloading, and cache invalidation.

use std::fs;
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

async fn spawn_dynamic_upstream(counter: Arc<AtomicUsize>, body_text: &'static str) -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        loop {
            if let Ok((mut stream, _)) = listener.accept().await {
                let cnt = counter.clone();
                tokio::spawn(async move {
                    let mut buf = vec![0u8; 1024];
                    if let Ok(n) = stream.read(&mut buf).await {
                        if n > 0 {
                            cnt.fetch_add(1, Ordering::Relaxed);
                            let resp = format!(
                                "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                                body_text.len(),
                                body_text
                            );
                            let _ = stream.write_all(resp.as_bytes()).await;
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
async fn test_e2e_zero_downtime_hot_reload() {
    let test_id = torque_core::time::fast_now_nanos();
    let config_path = std::env::temp_dir().join(format!("torque_test_config_{test_id}.yaml"));

    let counter1 = Arc::new(AtomicUsize::new(0));
    let addr1 = spawn_dynamic_upstream(counter1.clone(), "cluster1-response").await;

    let counter2 = Arc::new(AtomicUsize::new(0));
    let addr2 = spawn_dynamic_upstream(counter2.clone(), "cluster2-response").await;

    // Initial Config: Only Route Alpha
    let initial_yaml = format!(
        r#"
server:
  bind_addr: "127.0.0.1:0"
admin:
  bind_addr: "127.0.0.1:0"
clusters:
  - name: "c1"
    endpoints:
      - addr: "{addr1}"
  - name: "c2"
    endpoints:
      - addr: "{addr2}"
routes:
  - name: "route-alpha"
    path: "/alpha"
    cluster: "c1"
"#
    );

    fs::write(&config_path, initial_yaml).unwrap();

    let server = TorqueServer::from_file(&config_path).await.unwrap();
    let client_addr: SocketAddr = "127.0.0.1:54321".parse().unwrap();

    // /alpha should match; /beta should return 404
    let req_alpha = Request::new(Method::Get, "/alpha");
    let resp_alpha = server.dispatcher().dispatch(req_alpha, client_addr).await;
    assert_eq!(resp_alpha.status, 200);
    assert_eq!(counter1.load(Ordering::Relaxed), 1);

    let req_beta = Request::new(Method::Get, "/beta");
    let resp_beta = server.dispatcher().dispatch(req_beta, client_addr).await;
    assert_eq!(resp_beta.status, 404);

    // Overwrite config on disk: add Route Beta
    let updated_yaml = format!(
        r#"
server:
  bind_addr: "127.0.0.1:0"
admin:
  bind_addr: "127.0.0.1:0"
clusters:
  - name: "c1"
    endpoints:
      - addr: "{addr1}"
  - name: "c2"
    endpoints:
      - addr: "{addr2}"
routes:
  - name: "route-alpha"
    path: "/alpha"
    cluster: "c1"
  - name: "route-beta"
    path: "/beta"
    cluster: "c2"
"#
    );

    fs::write(&config_path, updated_yaml).unwrap();

    // Trigger zero-downtime hot reload
    server.reload().unwrap();

    // Now /beta must succeed immediately without server restart!
    let req_beta2 = Request::new(Method::Get, "/beta");
    let resp_beta2 = server.dispatcher().dispatch(req_beta2, client_addr).await;
    assert_eq!(resp_beta2.status, 200);
    assert_eq!(counter2.load(Ordering::Relaxed), 1);
    assert!(String::from_utf8_lossy(&resp_beta2.body).contains("cluster2-response"));
}

#[tokio::test]
async fn test_e2e_cache_invalidation_by_tag() {
    let counter = Arc::new(AtomicUsize::new(0));
    let addr = spawn_dynamic_upstream(counter.clone(), "cacheable-body").await;

    let config = TorqueConfig {
        server: ServerConfig::default(),
        admin: AdminConfig::default(),
        clusters: vec![ClusterConfig {
            name: "tag-cluster".to_string(),
            lb_policy: "round_robin".to_string(),
            endpoints: vec![EndpointConfig {
                addr: addr.to_string(),
                weight: 10,
            }],
            health_check: None,
            circuit_breaker: None,
        }],
        routes: vec![RouteConfig {
            name: "tag-route".to_string(),
            path: "/tagged".to_string(),
            methods: vec!["GET".to_string()],
            cluster: "tag-cluster".to_string(),
            timeout_ms: Some(2000),
            rate_limit: None,
            cache: Some(CacheConfig {
                enabled: true,
                ttl_seconds: 120,
            }),
            plugins: Vec::new(),
        }],
    };

    let server = TorqueServer::from_config(config, None).await.unwrap();
    let client_addr: SocketAddr = "127.0.0.1:54322".parse().unwrap();

    // First request: Cache Miss -> forwards to upstream (counter = 1)
    let req1 = Request::new(Method::Get, "/tagged");
    let resp1 = server.dispatcher().dispatch(req1, client_addr).await;
    assert_eq!(resp1.status, 200);
    assert_eq!(counter.load(Ordering::Relaxed), 1);

    // Second request: Cache Hit -> served from memory (counter still = 1)
    let req2 = Request::new(Method::Get, "/tagged");
    let resp2 = server.dispatcher().dispatch(req2, client_addr).await;
    assert_eq!(resp2.status, 200);
    assert_eq!(counter.load(Ordering::Relaxed), 1);

    // Invalidate cache by surrogate tag (cluster name)
    let invalidated = server.cache_engine().invalidate_tag("tag-cluster");
    assert_eq!(invalidated, 1);

    // Third request: Cache Miss after tag invalidation -> re-queries upstream (counter = 2)
    let req3 = Request::new(Method::Get, "/tagged");
    let resp3 = server.dispatcher().dispatch(req3, client_addr).await;
    assert_eq!(resp3.status, 200);
    assert_eq!(counter.load(Ordering::Relaxed), 2);
}
