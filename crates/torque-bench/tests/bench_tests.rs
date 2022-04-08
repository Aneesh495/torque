//! Integration tests for the benchmarking engine.

use std::net::SocketAddr;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use torque_bench::engine::{BenchConfig, BenchEngine};

async fn spawn_fast_mock_server() -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        loop {
            if let Ok((mut stream, _)) = listener.accept().await {
                tokio::spawn(async move {
                    let mut buf = vec![0u8; 1024];
                    let resp = b"HTTP/1.1 200 OK\r\nContent-Length: 13\r\nConnection: keep-alive\r\n\r\nHello, World!";
                    loop {
                        match stream.read(&mut buf).await {
                            Ok(0) | Err(_) => break,
                            Ok(_) => {
                                if stream.write_all(resp).await.is_err() {
                                    break;
                                }
                                if stream.flush().await.is_err() {
                                    break;
                                }
                            }
                        }
                    }
                });
            }
        }
    });

    addr
}

#[tokio::test]
async fn test_bench_engine_execution() {
    let target_addr = spawn_fast_mock_server().await;

    let config = BenchConfig {
        target_addr,
        path: "/test".to_string(),
        concurrency: 4,
        duration: Duration::from_millis(300),
        pipeline_depth: 1,
    };

    let engine = BenchEngine::new(config);
    let result = engine.run().await.unwrap();

    assert!(result.total_requests > 0);
    assert_eq!(result.failed_requests, 0);
    assert_eq!(result.successful_requests, result.total_requests);
    assert_eq!(result.status_2xx, result.total_requests);
    assert!(result.requests_per_sec > 0.0);

    let csv = result.to_csv();
    assert!(csv.contains("total_requests"));
    assert!(csv.contains("latency_p50_ms"));
}
