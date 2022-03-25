//! # Torque Config
//!
//! Strongly-typed declarative configuration schema, semantic validation,
//! multi-format loaders (YAML, JSON, TOML), and atomic zero-downtime hot reloading.

pub mod loader;
pub mod schema;
pub mod validator;
pub mod watcher;

pub use loader::{load_from_file, load_from_str, ConfigFormat};
pub use schema::{
    AdminConfig, CircuitBreakerConfig, ClusterConfig, EndpointConfig, HealthCheckConfig,
    PluginConfig, RateLimitConfig, RouteConfig, ServerConfig, TorqueConfig,
};
pub use validator::validate_config;
pub use watcher::ConfigManager;

/// Config prelude.
pub mod prelude {
    pub use crate::loader::{load_from_file, load_from_str, ConfigFormat};
    pub use crate::schema::TorqueConfig;
    pub use crate::validator::validate_config;
    pub use crate::watcher::ConfigManager;
}
