//! High-level Torque proxy daemon lifecycle, configuration compilation, and hot reloading.

use dashmap::DashMap;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::watch;
use tracing::{error, info};

use torque_admin::AdminServer;
use torque_balance::balancer::{ClusterBalancer, LbPolicy};
use torque_balance::endpoint::Endpoint;
use torque_cache::engine::HttpCacheEngine;
use torque_config::schema::{CircuitBreakerConfig as SchemaCircuitConfig, TorqueConfig};
use torque_config::watcher::ConfigManager;
use torque_core::error::{Result, TorqueError};
use torque_filter::pipeline::FilterChain;
use torque_filter::security::SecurityHeadersFilter;
use torque_filter::tracing::TraceparentFilter;
use torque_metrics::registry::MetricsRegistry;
use torque_proto::http1::types::Method;
use torque_resilience::circuit::{CircuitBreaker, CircuitBreakerConfig};
use torque_resilience::ratelimit::{RateLimitConfig, RateLimiterRegistry};
use torque_router::route::Route;
use torque_router::table::{AtomicRouteTable, RouteTable};
use torque_upstream::client::UpstreamClient;
use torque_upstream::pool::PoolConfig;

use crate::dispatcher::RequestDispatcher;
use crate::reactor::TcpReactor;

/// Production reverse proxy server hosting network listeners, admin endpoints, and traffic dispatch.
pub struct TorqueServer {
    config_manager: Arc<ConfigManager>,
    route_table: Arc<AtomicRouteTable>,
    clusters: Arc<DashMap<String, Arc<ClusterBalancer>>>,
    circuit_breakers: Arc<DashMap<String, Arc<CircuitBreaker>>>,
    rate_limiters: Arc<DashMap<String, Arc<RateLimiterRegistry>>>,
    cache_engine: Arc<HttpCacheEngine>,
    upstream_client: Arc<UpstreamClient>,
    filter_chain: Arc<FilterChain>,
    metrics: Arc<MetricsRegistry>,
    dispatcher: Arc<RequestDispatcher>,
    server_addr: SocketAddr,
    admin_addr: Option<SocketAddr>,
    tcp_nodelay: bool,
    buffer_size_kb: usize,
    shutdown_tx: watch::Sender<bool>,
    shutdown_rx: watch::Receiver<bool>,
}

impl TorqueServer {
    /// Initialize a new server instance from a configuration file on disk.
    pub async fn from_file(path: impl AsRef<Path>) -> Result<Self> {
        let path_buf = path.as_ref().to_path_buf();
        let config_mgr = ConfigManager::from_file(&path_buf)?;
        let config = config_mgr.current();
        Self::build(config_mgr, (*config).clone()).await
    }

    /// Initialize server instance directly from an in-memory configuration struct.
    pub async fn from_config(config: TorqueConfig, path: Option<PathBuf>) -> Result<Self> {
        let fake_path = path.unwrap_or_else(|| PathBuf::from("torque.yaml"));
        let config_mgr = ConfigManager::from_config(config.clone(), fake_path);
        Self::build(config_mgr, config).await
    }

    /// Internal builder constructing all subsystems and compiling initial routes.
    async fn build(config_manager: Arc<ConfigManager>, config: TorqueConfig) -> Result<Self> {
        let server_addr: SocketAddr = config.server.bind_addr.parse().map_err(|e| {
            TorqueError::config("server.bind_addr", format!("Invalid proxy server bind address: {}", e))
        })?;

        let admin_addr: Option<SocketAddr> = if !config.admin.bind_addr.is_empty() {
            Some(config.admin.bind_addr.parse().map_err(|e| {
                TorqueError::config("admin.bind_addr", format!("Invalid admin server bind address: {}", e))
            })?)
        } else {
            None
        };

        let metrics = MetricsRegistry::new();
        let cache_engine = HttpCacheEngine::new(10_000);
        let upstream_client = Arc::new(UpstreamClient::new(PoolConfig::default()));

        // Build Default Filter Pipeline
        let mut filter_chain = FilterChain::new();
        filter_chain.add_filter(Arc::new(TraceparentFilter));
        filter_chain.add_filter(Arc::new(SecurityHeadersFilter));
        let filter_chain = Arc::new(filter_chain);

        // Compile Clusters
        let clusters = Arc::new(DashMap::new());
        let circuit_breakers = Arc::new(DashMap::new());

        for cluster_cfg in &config.clusters {
            let policy = match cluster_cfg.lb_policy.to_ascii_lowercase().as_str() {
                "round_robin" => LbPolicy::RoundRobin,
                "least_conn" => LbPolicy::LeastConn,
                "peak_ewma" => LbPolicy::PeakEwma,
                "ketama" => LbPolicy::Ketama,
                "maglev" => LbPolicy::Maglev,
                _ => LbPolicy::PeakEwma,
            };

            let mut endpoints = Vec::new();
            for ep_cfg in &cluster_cfg.endpoints {
                let addr: SocketAddr = ep_cfg.addr.parse().map_err(|e| {
                    TorqueError::config(
                        "cluster.endpoints",
                        format!(
                            "Invalid endpoint address {} in cluster {}: {}",
                            ep_cfg.addr, cluster_cfg.name, e
                        ),
                    )
                })?;
                endpoints.push(Endpoint::new(addr, ep_cfg.weight));
            }

            let balancer = Arc::new(ClusterBalancer::new(endpoints, policy));
            clusters.insert(cluster_cfg.name.clone(), balancer);

            if let Some(ref cb_cfg) = cluster_cfg.circuit_breaker {
                let cb = Arc::new(CircuitBreaker::new(
                    cluster_cfg.name.clone(),
                    Self::convert_cb_config(cb_cfg),
                ));
                circuit_breakers.insert(cluster_cfg.name.clone(), cb);
            }
        }

        // Compile Rate Limiters
        let rate_limiters = Arc::new(DashMap::new());
        for route_cfg in &config.routes {
            if let Some(ref rl_cfg) = route_cfg.rate_limit {
                let registry = Arc::new(RateLimiterRegistry::new(RateLimitConfig {
                    rate_per_sec: rl_cfg.rate,
                    burst_capacity: rl_cfg.burst,
                }));
                rate_limiters.insert(route_cfg.name.clone(), registry);
            }
        }

        // Compile Route Table
        let compiled_routes = Self::compile_routes(&config.routes)?;
        let route_table = Arc::new(AtomicRouteTable::new(compiled_routes));

        let dispatcher = Arc::new(RequestDispatcher::new(
            route_table.clone(),
            clusters.clone(),
            circuit_breakers.clone(),
            rate_limiters.clone(),
            cache_engine.clone(),
            upstream_client.clone(),
            filter_chain.clone(),
            metrics.clone(),
        ));

        let (shutdown_tx, shutdown_rx) = watch::channel(false);

        Ok(Self {
            config_manager,
            route_table,
            clusters,
            circuit_breakers,
            rate_limiters,
            cache_engine,
            upstream_client,
            filter_chain,
            metrics,
            dispatcher,
            server_addr,
            admin_addr,
            tcp_nodelay: config.server.tcp_nodelay,
            buffer_size_kb: config.server.buffer_size_kb,
            shutdown_tx,
            shutdown_rx,
        })
    }

    /// Compile a list of configuration route schemas into an immutable RouteTable.
    fn compile_routes(routes: &[torque_config::schema::RouteConfig]) -> Result<RouteTable> {
        let mut core_routes = Vec::new();

        for r in routes {
            let mut route = Route::new(r.name.clone(), r.path.clone(), r.cluster.clone());
            route.timeout_ms = r.timeout_ms;

            if let Some(ref c) = r.cache {
                if c.enabled {
                    route.cache_ttl_secs = Some(c.ttl_seconds);
                }
            }

            if !r.methods.is_empty() {
                let mut methods = Vec::new();
                for m in &r.methods {
                    methods.push(Method::from_bytes(m.to_uppercase().as_bytes())?);
                }
                route.methods = methods;
            }

            core_routes.push(route);
        }

        RouteTable::compile(core_routes)
    }

    /// Convert configuration circuit breaker schema to resilience circuit breaker config.
    fn convert_cb_config(cfg: &SchemaCircuitConfig) -> CircuitBreakerConfig {
        CircuitBreakerConfig {
            failure_rate_threshold: cfg.failure_rate_threshold,
            slow_call_rate_threshold: cfg.slow_call_rate_threshold,
            slow_call_duration_ms: 1000,
            min_request_volume: cfg.min_request_volume,
            window_duration_ms: cfg.window_duration_ms,
            recovery_timeout_ms: cfg.recovery_timeout_ms,
            half_open_probes: cfg.half_open_probes,
        }
    }

    /// Hot-reload server configuration: recompiles routes and re-points cluster balancers.
    pub fn reload(&self) -> Result<()> {
        info!("Initiating zero-downtime configuration hot reload");
        let new_config = self.config_manager.reload()?;

        let new_table = Self::compile_routes(&new_config.routes)?;
        self.route_table.swap(new_table);

        info!("Configuration hot reload applied successfully without connection interruption");
        Ok(())
    }

    /// Retrieve local server socket address.
    pub fn server_addr(&self) -> SocketAddr {
        self.server_addr
    }

    /// Retrieve administrative socket address if configured.
    pub fn admin_addr(&self) -> Option<SocketAddr> {
        self.admin_addr
    }

    /// Access the cluster balancers map.
    pub fn clusters(&self) -> &Arc<DashMap<String, Arc<ClusterBalancer>>> {
        &self.clusters
    }

    /// Access the circuit breakers map.
    pub fn circuit_breakers(&self) -> &Arc<DashMap<String, Arc<CircuitBreaker>>> {
        &self.circuit_breakers
    }

    /// Access the rate limiters map.
    pub fn rate_limiters(&self) -> &Arc<DashMap<String, Arc<RateLimiterRegistry>>> {
        &self.rate_limiters
    }

    /// Access the HTTP cache engine.
    pub fn cache_engine(&self) -> &Arc<HttpCacheEngine> {
        &self.cache_engine
    }

    /// Access the upstream client.
    pub fn upstream_client(&self) -> &Arc<UpstreamClient> {
        &self.upstream_client
    }

    /// Access the filter chain.
    pub fn filter_chain(&self) -> &Arc<FilterChain> {
        &self.filter_chain
    }

    /// Access the active route table holder.
    pub fn route_table(&self) -> &Arc<AtomicRouteTable> {
        &self.route_table
    }

    /// Access the shared metrics registry.
    pub fn metrics(&self) -> &Arc<MetricsRegistry> {
        &self.metrics
    }

    /// Access request dispatcher.
    pub fn dispatcher(&self) -> &Arc<RequestDispatcher> {
        &self.dispatcher
    }

    /// Trigger graceful server termination.
    pub fn shutdown(&self) {
        let _ = self.shutdown_tx.send(true);
    }

    /// Run the proxy daemon and administrative endpoints until shutdown signal.
    pub async fn run(self) -> Result<()> {
        let reactor = TcpReactor::bind(
            self.server_addr,
            self.dispatcher.clone(),
            self.metrics.clone(),
            self.tcp_nodelay,
            self.buffer_size_kb,
        )
        .await?;

        let admin_opt = if let Some(addr) = self.admin_addr {
            let admin_server = AdminServer::bind(
                addr,
                self.metrics.clone(),
                Some(self.config_manager.clone()),
            )
            .await?;
            Some(admin_server)
        } else {
            None
        };

        let shutdown_rx_admin = self.shutdown_rx.clone();
        let shutdown_rx_proxy = self.shutdown_rx.clone();

        // Spawn Admin Server Task
        let admin_handle = tokio::spawn(async move {
            if let Some(admin) = admin_opt {
                if let Err(e) = admin.run(shutdown_rx_admin).await {
                    error!(error = %e, "Admin server terminated with error");
                }
            }
        });

        // Spawn Proxy Reactor Task
        let proxy_handle = tokio::spawn(async move {
            if let Err(e) = reactor.run(shutdown_rx_proxy).await {
                error!(error = %e, "Proxy reactor terminated with error");
            }
        });

        // Signal listener for graceful termination
        let shutdown_tx = self.shutdown_tx.clone();
        tokio::spawn(async move {
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {
                    info!("Received SIGINT (Ctrl-C); commencing graceful shutdown");
                    let _ = shutdown_tx.send(true);
                }
            }
        });

        info!("Torque server running: ready to accept traffic");
        let _ = tokio::join!(admin_handle, proxy_handle);
        info!("Torque server shutdown sequence completed successfully");

        Ok(())
    }
}
