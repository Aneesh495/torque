//! Upstream persistent TCP connection and HTTP message exchange.

use std::net::SocketAddr;
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use torque_core::error::{Result, TorqueError};
use torque_proto::http1::{Http1Encoder, Http1Parser, ParseResult, Request, Response};

/// Connected TCP socket to an upstream destination.
pub struct UpstreamConnection {
    pub addr: SocketAddr,
    stream: TcpStream,
    created_at: Instant,
    last_used: Instant,
}

impl UpstreamConnection {
    /// Establish a new TCP connection to an upstream address.
    pub async fn connect(addr: SocketAddr, timeout_dur: Duration) -> Result<Self> {
        let stream = tokio::time::timeout(timeout_dur, TcpStream::connect(addr))
            .await
            .map_err(|_| TorqueError::UpstreamTimeout {
                cluster: "upstream".to_string(),
                timeout_ms: timeout_dur.as_millis() as u64,
            })?
            .map_err(|e| TorqueError::UpstreamUnreachable {
                cluster: "upstream".to_string(),
                endpoint: addr.to_string(),
                detail: format!("TCP connect failed: {}", e),
            })?;

        let _ = stream.set_nodelay(true);

        let now = Instant::now();
        Ok(Self {
            addr,
            stream,
            created_at: now,
            last_used: now,
        })
    }

    /// Dispatch an HTTP request and await the corresponding response.
    pub async fn exchange(&mut self, req: &Request, timeout_dur: Duration) -> Result<Response> {
        let mut req_buf = Vec::with_capacity(1024);
        Http1Encoder::encode_request(req, &mut req_buf);

        let exchange_fut = async {
            self.stream.write_all(&req_buf).await.map_err(|e| {
                TorqueError::UpstreamUnreachable {
                    cluster: "upstream".to_string(),
                    endpoint: self.addr.to_string(),
                    detail: format!("Write error: {}", e),
                }
            })?;

            let mut resp_buf = Vec::with_capacity(4096);
            let mut chunk = [0u8; 4096];

            loop {
                let n = self.stream.read(&mut chunk).await.map_err(|e| {
                    TorqueError::UpstreamUnreachable {
                        cluster: "upstream".to_string(),
                        endpoint: self.addr.to_string(),
                        detail: format!("Read error: {}", e),
                    }
                })?;

                if n == 0 {
                    if resp_buf.is_empty() {
                        return Err(TorqueError::UpstreamUnreachable {
                            cluster: "upstream".to_string(),
                            endpoint: self.addr.to_string(),
                            detail: "Connection closed by upstream peer before response"
                                .to_string(),
                        });
                    }
                    break;
                }

                resp_buf.extend_from_slice(&chunk[..n]);

                match Http1Parser::parse_response(&resp_buf)? {
                    ParseResult::Complete(resp, _) => {
                        self.last_used = Instant::now();
                        return Ok(resp);
                    }
                    ParseResult::Incomplete => {
                        // Continue reading from socket
                    }
                }
            }

            match Http1Parser::parse_response(&resp_buf)? {
                ParseResult::Complete(resp, _) => {
                    self.last_used = Instant::now();
                    Ok(resp)
                }
                ParseResult::Incomplete => Err(TorqueError::protocol(
                    "Truncated upstream HTTP response on connection close",
                )),
            }
        };

        tokio::time::timeout(timeout_dur, exchange_fut)
            .await
            .map_err(|_| TorqueError::UpstreamTimeout {
                cluster: "upstream".to_string(),
                timeout_ms: timeout_dur.as_millis() as u64,
            })?
    }

    /// Check if connection has exceeded idle lifetime limit.
    pub fn is_expired(&self, max_idle: Duration) -> bool {
        self.last_used.elapsed() > max_idle
    }

    /// Timestamp when this TCP connection was originally established.
    pub fn created_at(&self) -> Instant {
        self.created_at
    }
}
