use std::net::SocketAddr;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use torque_balance::endpoint::Endpoint;
use torque_proto::http1::{Method, Request};
use torque_upstream::{PoolConfig, UpstreamClient, UpstreamConnection};

async fn spawn_mock_server() -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        loop {
            if let Ok((mut socket, _)) = listener.accept().await {
                tokio::spawn(async move {
                    let mut buf = [0u8; 1024];
                    while let Ok(n) = socket.read(&mut buf).await {
                        if n == 0 {
                            break;
                        }
                        // Respond 200 OK
                        let resp = b"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 12\r\n\r\nhello-mocked";
                        let _ = socket.write_all(resp).await;
                    }
                });
            }
        }
    });

    addr
}

#[tokio::test]
async fn test_upstream_connection_exchange() {
    let addr = spawn_mock_server().await;

    let mut conn = UpstreamConnection::connect(addr, Duration::from_millis(1000))
        .await
        .expect("connect failed");

    let req = Request::new(Method::Get, "/healthz");
    let resp = conn
        .exchange(&req, Duration::from_millis(1000))
        .await
        .expect("exchange failed");

    assert_eq!(resp.status, 200);
    assert_eq!(resp.body.as_ref(), b"hello-mocked");
}

#[tokio::test]
async fn test_connection_pool_reuse_and_forward() {
    let addr = spawn_mock_server().await;
    let endpoint = Endpoint::new(addr, 10);

    let client = UpstreamClient::new(PoolConfig::default());
    let req = Request::new(Method::Get, "/api/v1/ping");

    // First request opens connection
    let resp1 = client
        .forward(&endpoint, &req, Duration::from_millis(1000))
        .await
        .expect("first forward failed");
    assert_eq!(resp1.status, 200);

    // Second request reuses pooled connection
    let resp2 = client
        .forward(&endpoint, &req, Duration::from_millis(1000))
        .await
        .expect("second forward failed");
    assert_eq!(resp2.status, 200);

    // Latency must have been recorded
    assert!(endpoint.ewma_latency_ms.get() > 0.0);
    assert_eq!(
        endpoint
            .consecutive_failures
            .load(std::sync::atomic::Ordering::Relaxed),
        0
    );
}
