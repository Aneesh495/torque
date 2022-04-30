//! Strongly-typed declarative configuration schema.

use serde::{Deserialize, Serialize};

/// Root configuration holding all proxy and service mesh settings.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct TorqueConfig {
    #[serde(default)]
    pub server: ServerConfig,
    #[serde(default)]
    pub admin: AdminConfig,
    #[serde(default)]
    pub clusters: Vec<ClusterConfig>,
    #[serde(default)]
    pub routes: Vec<RouteConfig>,
}

/// Server network reactor and worker thread parameters.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ServerConfig {
    #[serde(default = "default_bind_addr")]
    pub bind_addr: String,
    #[serde(default = "default_worker_threads")]
    pub worker_threads: usize,
    #[serde(default = "default_max_connections")]
    pub max_connections: usize,
    #[serde(default = "default_true")]
    pub tcp_nodelay: bool,
    #[serde(default = "default_buffer_size_kb")]
    pub buffer_size_kb: usize,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind_addr: default_bind_addr(),
            worker_threads: default_worker_threads(),
            max_connections: default_max_connections(),
            tcp_nodelay: true,
            buffer_size_kb: default_buffer_size_kb(),
        }
    }
}

/// Administrative control plane and metrics server configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AdminConfig {
    #[serde(default = "default_admin_bind_addr")]
    pub bind_addr: String,
    #[serde(default = "default_true")]
    pub enable_dashboard: bool,
    #[serde(default = "default_metrics_path")]
    pub metrics_path: String,
}

impl Default for AdminConfig {
    fn default() -> Self {
        Self {
            bind_addr: default_admin_bind_addr(),
            enable_dashboard: true,
            metrics_path: default_metrics_path(),
        }
    }
}

/// Upstream cluster definition.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ClusterConfig {
    pub name: String,
    #[serde(default = "default_lb_policy")]
    pub lb_policy: String,
    #[serde(default)]
    pub endpoints: Vec<EndpointConfig>,
    #[serde(default)]
    pub health_check: Option<HealthCheckConfig>,
    #[serde(default)]
    pub circuit_breaker: Option<CircuitBreakerConfig>,
}

/// Individual upstream endpoint definition.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EndpointConfig {
    pub addr: String,
    #[serde(default = "default_weight")]
    pub weight: u32,
}

/// Active health checking parameters.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HealthCheckConfig {
    #[serde(default = "default_health_path")]
    pub path: String,
    #[serde(default = "default_interval_ms")]
    pub interval_ms: u64,
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,
    #[serde(default = "default_healthy_threshold")]
    pub healthy_threshold: usize,
    #[serde(default = "default_unhealthy_threshold")]
    pub unhealthy_threshold: usize,
}

/// Circuit breaker policy configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CircuitBreakerConfig {
    #[serde(default = "default_failure_rate")]
    pub failure_rate_threshold: f64,
    #[serde(default = "default_slow_call_rate")]
    pub slow_call_rate_threshold: f64,
    #[serde(default = "default_min_volume")]
    pub min_request_volume: usize,
    #[serde(default = "default_window_ms")]
    pub window_duration_ms: u64,
    #[serde(default = "default_recovery_ms")]
    pub recovery_timeout_ms: u64,
    #[serde(default = "default_half_open_probes")]
    pub half_open_probes: usize,
}

/// Route rule matching and upstream dispatch configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RouteConfig {
    pub name: String,
    pub path: String,
    #[serde(default)]
    pub methods: Vec<String>,
    pub cluster: String,
    #[serde(default)]
    pub timeout_ms: Option<u64>,
    #[serde(default)]
    pub rate_limit: Option<RateLimitConfig>,
    #[serde(default)]
    pub cache: Option<CacheConfig>,
    #[serde(default)]
    pub plugins: Vec<PluginConfig>,
}

/// Route-level rate limiting settings.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RateLimitConfig {
    pub rate: u64,
    pub burst: u64,
    #[serde(default = "default_rate_key_type")]
    pub key_type: String,
}

/// Route-level caching settings.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CacheConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_cache_ttl")]
    pub ttl_seconds: u64,
}

/// Filter plugin entry.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PluginConfig {
    pub name: String,
    #[serde(default)]
    pub secret: Option<String>,
    #[serde(default)]
    pub header: Option<String>,
    #[serde(default)]
    pub options: Option<serde_json::Value>,
}

// Serde defaults
fn default_bind_addr() -> String {
    "0.0.0.0:8080".to_string()
}
fn default_admin_bind_addr() -> String {
    "127.0.0.1:9090".to_string()
}
fn default_metrics_path() -> String {
    "/metrics".to_string()
}
fn default_worker_threads() -> usize {
    num_cpus()
}
fn default_max_connections() -> usize {
    50_000
}
fn default_true() -> bool {
    true
}
fn default_buffer_size_kb() -> usize {
    64
}
fn default_lb_policy() -> String {
    "peak_ewma".to_string()
}
fn default_weight() -> u32 {
    10
}
fn default_health_path() -> String {
    "/healthz".to_string()
}
fn default_interval_ms() -> u64 {
    2000
}
fn default_timeout_ms() -> u64 {
    500
}
fn default_healthy_threshold() -> usize {
    2
}
fn default_unhealthy_threshold() -> usize {
    3
}
fn default_failure_rate() -> f64 {
    0.50
}
fn default_slow_call_rate() -> f64 {
    0.80
}
fn default_min_volume() -> usize {
    20
}
fn default_window_ms() -> u64 {
    10_000
}
fn default_recovery_ms() -> u64 {
    5000
}
fn default_half_open_probes() -> usize {
    5
}
fn default_rate_key_type() -> String {
    "ip".to_string()
}
fn default_cache_ttl() -> u64 {
    60
}

fn num_cpus() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
}
