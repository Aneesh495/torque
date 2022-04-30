//! Asynchronous TCP reactor, worker task dispatcher, and connection pool coordinator.

use bytes::BytesMut;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::watch;
use tracing::{debug, info, warn};

use torque_core::error::{Result, TorqueError};
use torque_metrics::registry::MetricsRegistry;
use torque_proto::http1::encoder::Http1Encoder;
use torque_proto::http1::parser::{Http1Parser, ParseResult};

use crate::dispatcher::RequestDispatcher;

/// RAII guard ensuring active connection metrics gauge decrements when a connection drops.
struct ConnGuard(Arc<MetricsRegistry>);

impl Drop for ConnGuard {
    fn drop(&mut self) {
        self.0.dec_connection();
    }
}

/// High-throughput TCP listener reactor.
pub struct TcpReactor {
    listener: TcpListener,
    local_addr: SocketAddr,
    dispatcher: Arc<RequestDispatcher>,
    metrics: Arc<MetricsRegistry>,
    tcp_nodelay: bool,
    buffer_size: usize,
}

impl TcpReactor {
    /// Bind a new TCP reactor to the configured socket address.
    pub async fn bind(
        addr: SocketAddr,
        dispatcher: Arc<RequestDispatcher>,
        metrics: Arc<MetricsRegistry>,
        tcp_nodelay: bool,
        buffer_size_kb: usize,
    ) -> Result<Self> {
        let listener = TcpListener::bind(addr).await.map_err(TorqueError::io)?;
        let local_addr = listener.local_addr().map_err(TorqueError::io)?;

        info!(addr = %local_addr, "Torque TCP proxy reactor bound successfully");

        Ok(Self {
            listener,
            local_addr,
            dispatcher,
            metrics,
            tcp_nodelay,
            buffer_size: buffer_size_kb * 1024,
        })
    }

    /// Retrieve local bound socket address.
    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    /// Run the reactor event loop until receiving a shutdown signal.
    pub async fn run(self, mut shutdown: watch::Receiver<bool>) -> Result<()> {
        let reactor = Arc::new(self);
        info!(addr = %reactor.local_addr, "Proxy event loop listening for traffic");

        loop {
            tokio::select! {
                res = reactor.listener.accept() => {
                    match res {
                        Ok((stream, client_addr)) => {
                            if reactor.tcp_nodelay {
                                let _ = stream.set_nodelay(true);
                            }

                            reactor.metrics.inc_connection();
                            let disp = reactor.dispatcher.clone();
                            let metrics_ref = reactor.metrics.clone();
                            let buf_cap = reactor.buffer_size;

                            tokio::spawn(async move {
                                let _guard = ConnGuard(metrics_ref);
                                if let Err(e) = Self::handle_connection(stream, client_addr, disp, buf_cap).await {
                                    debug!(peer = %client_addr, error = %e, "Client connection ended");
                                }
                            });
                        }
                        Err(e) => {
                            warn!(error = %e, "Failed to accept incoming TCP connection");
                        }
                    }
                }
                changed = shutdown.changed() => {
                    if changed.is_ok() && *shutdown.borrow() {
                        info!("Proxy reactor received shutdown signal: terminating listener");
                        break;
                    }
                }
            }
        }

        Ok(())
    }

    /// Handle persistent HTTP/1.1 keep-alive session on a client TCP stream.
    async fn handle_connection(
        mut stream: TcpStream,
        client_addr: SocketAddr,
        dispatcher: Arc<RequestDispatcher>,
        buffer_size: usize,
    ) -> Result<()> {
        let mut read_buf = BytesMut::with_capacity(buffer_size);
        let mut out_buf = Vec::with_capacity(buffer_size);

        loop {
            let n = stream
                .read_buf(&mut read_buf)
                .await
                .map_err(TorqueError::io)?;
            if n == 0 {
                return Ok(());
            }

            loop {
                match Http1Parser::parse_request(&read_buf)? {
                    ParseResult::Complete(req, consumed) => {
                        let is_close = req
                            .headers
                            .get("connection")
                            .map(|v| v.eq_ignore_ascii_case("close"))
                            .unwrap_or(false);

                        // Advance read buffer past consumed bytes
                        let _ = read_buf.split_to(consumed);

                        // Dispatch request through complete pipeline
                        let mut resp = dispatcher.dispatch(req, client_addr).await;

                        if is_close {
                            resp.headers.insert("connection", "close");
                        }

                        // Serialize and write response
                        out_buf.clear();
                        Http1Encoder::encode_response(&resp, &mut out_buf);
                        stream.write_all(&out_buf).await.map_err(TorqueError::io)?;
                        stream.flush().await.map_err(TorqueError::io)?;

                        if is_close {
                            return Ok(());
                        }
                    }
                    ParseResult::Incomplete => {
                        // Need more data from socket
                        break;
                    }
                }
            }
        }
    }
}
