//! Administrative HTTP server for health probes, Prometheus metrics, and live configuration control.

use std::net::SocketAddr;
use std::sync::Arc;
use bytes::Bytes;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::watch;
use tracing::{debug, error, info, warn};

use torque_config::watcher::ConfigManager;
use torque_core::error::{Result, TorqueError};
use torque_metrics::registry::MetricsRegistry;
use torque_proto::http1::encoder::Http1Encoder;
use torque_proto::http1::parser::{Http1Parser, ParseResult};
use torque_proto::http1::types::{Method, Request, Response};

use crate::dashboard::DASHBOARD_HTML;

/// Administrative server daemon hosting health, metrics, and management endpoints.
pub struct AdminServer {
    listener: TcpListener,
    local_addr: SocketAddr,
    metrics: Arc<MetricsRegistry>,
    config_manager: Option<Arc<ConfigManager>>,
}

impl AdminServer {
    /// Bind the administrative server to a specific socket address.
    pub async fn bind(
        addr: SocketAddr,
        metrics: Arc<MetricsRegistry>,
        config_manager: Option<Arc<ConfigManager>>,
    ) -> Result<Self> {
        let listener = TcpListener::bind(addr)
            .await
            .map_err(TorqueError::io)?;
        let local_addr = listener
            .local_addr()
            .map_err(TorqueError::io)?;

        info!(addr = %local_addr, "Torque admin server bound successfully");

        Ok(Self {
            listener,
            local_addr,
            metrics,
            config_manager,
        })
    }

    /// Return local bound socket address.
    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    /// Process a parsed HTTP request and return an appropriate administrative response.
    pub fn handle_request(&self, req: &Request) -> Response {
        let path = req.uri.split('?').next().unwrap_or(&req.uri);

        match (req.method, path) {
            (Method::Get, "/healthz") | (Method::Get, "/live") => {
                let mut resp = Response::ok(Bytes::from_static(b"{\"status\":\"alive\"}\n"));
                resp.headers.insert("content-type", "application/json");
                resp
            }
            (Method::Get, "/ready") => {
                let mut resp = Response::ok(Bytes::from_static(b"{\"status\":\"ready\"}\n"));
                resp.headers.insert("content-type", "application/json");
                resp
            }
            (Method::Get, "/metrics") => {
                let prom_text = self.metrics.export_prometheus();
                let mut resp = Response::ok(Bytes::from(prom_text));
                resp.headers.insert(
                    "content-type",
                    "text/plain; version=0.0.4; charset=utf-8",
                );
                resp
            }
            (Method::Get, "/stats") => {
                let stats_json = self.metrics.export_json();
                let mut resp = Response::ok(Bytes::from(stats_json));
                resp.headers.insert("content-type", "application/json");
                resp
            }
            (Method::Get, "/config") => match &self.config_manager {
                Some(mgr) => {
                    let config_snapshot = mgr.current();
                    match serde_json::to_string_pretty(&*config_snapshot) {
                        Ok(json) => {
                            let mut resp = Response::ok(Bytes::from(json));
                            resp.headers.insert("content-type", "application/json");
                            resp
                        }
                        Err(e) => {
                            let mut resp = Response::new(500);
                            resp.body = Bytes::from(format!("{{\"error\":\"{}\"}}\n", e));
                            resp.headers.insert("content-type", "application/json");
                            resp
                        }
                    }
                }
                None => {
                    let mut resp = Response::new(503);
                    resp.body = Bytes::from_static(
                        b"{\"error\":\"Configuration manager not configured on admin server\"}\n",
                    );
                    resp.headers.insert("content-type", "application/json");
                    resp
                }
            },
            (Method::Post, "/config/reload") => match &self.config_manager {
                Some(mgr) => match mgr.reload() {
                    Ok(_) => {
                        let mut resp = Response::ok(Bytes::from_static(
                            b"{\"status\":\"reloaded\",\"message\":\"Configuration hot reload successful\"}\n",
                        ));
                        resp.headers.insert("content-type", "application/json");
                        resp
                    }
                    Err(e) => {
                        error!(error = %e, "Administrative configuration reload failed");
                        let mut resp = Response::new(500);
                        resp.body = Bytes::from(format!(
                            "{{\"status\":\"failed\",\"error\":\"{}\"}}\n",
                            e
                        ));
                        resp.headers.insert("content-type", "application/json");
                        resp
                    }
                },
                None => {
                    let mut resp = Response::new(503);
                    resp.body = Bytes::from_static(
                        b"{\"error\":\"Configuration manager not configured on admin server\"}\n",
                    );
                    resp.headers.insert("content-type", "application/json");
                    resp
                }
            },
            (Method::Get, "/dashboard") => {
                let mut resp = Response::ok(Bytes::from_static(DASHBOARD_HTML.as_bytes()));
                resp.headers.insert("content-type", "text/html; charset=utf-8");
                resp
            }
            _ => {
                let mut resp = Response::new(404);
                resp.body = Bytes::from_static(b"{\"error\":\"Not Found\"}\n");
                resp.headers.insert("content-type", "application/json");
                resp
            }
        }
    }

    /// Run the administrative server until receiving a shutdown signal.
    pub async fn run(self, mut shutdown: watch::Receiver<bool>) -> Result<()> {
        let server = Arc::new(self);
        info!(addr = %server.local_addr, "Torque admin event loop running");

        loop {
            tokio::select! {
                res = server.listener.accept() => {
                    match res {
                        Ok((stream, peer_addr)) => {
                            debug!(peer = %peer_addr, "Accepted incoming admin connection");
                            let s = server.clone();
                            tokio::spawn(async move {
                                if let Err(e) = Self::handle_connection(s, stream).await {
                                    debug!(error = %e, "Admin client connection terminated");
                                }
                            });
                        }
                        Err(e) => {
                            warn!(error = %e, "Failed to accept incoming admin TCP connection");
                        }
                    }
                }
                changed = shutdown.changed() => {
                    if changed.is_ok() && *shutdown.borrow() {
                        info!("Received shutdown signal: gracefully halting admin server");
                        break;
                    }
                }
            }
        }

        Ok(())
    }

    async fn handle_connection(server: Arc<Self>, mut stream: TcpStream) -> Result<()> {
        let mut buf = vec![0u8; 8192];
        let mut filled = 0;

        loop {
            let n = stream
                .read(&mut buf[filled..])
                .await
                .map_err(TorqueError::io)?;

            if n == 0 {
                return Ok(());
            }

            filled += n;

            match Http1Parser::parse_request(&buf[..filled])? {
                ParseResult::Complete(req, _consumed) => {
                    let resp = server.handle_request(&req);
                    let mut out = Vec::with_capacity(1024);
                    Http1Encoder::encode_response(&resp, &mut out);

                    stream
                        .write_all(&out)
                        .await
                        .map_err(TorqueError::io)?;
                    stream
                        .flush()
                        .await
                        .map_err(TorqueError::io)?;
                    return Ok(());
                }
                ParseResult::Incomplete => {
                    if filled == buf.len() {
                        buf.resize(buf.len() * 2, 0);
                    }
                }
            }
        }
    }
}
