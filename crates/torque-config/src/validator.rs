//! Semantic validation and consistency checks for Torque configuration.

use std::collections::HashSet;
use std::net::SocketAddr;
use torque_core::error::{Result, TorqueError};

use super::schema::TorqueConfig;

/// Perform comprehensive validation of a parsed TorqueConfig.
pub fn validate_config(config: &TorqueConfig) -> Result<()> {
    // 1. Validate server bind address
    if config.server.bind_addr.parse::<SocketAddr>().is_err() {
        return Err(TorqueError::config(
            "server",
            format!("Invalid server bind_addr: {}", config.server.bind_addr),
        ));
    }

    // 2. Validate admin bind address
    if config.admin.bind_addr.parse::<SocketAddr>().is_err() {
        return Err(TorqueError::config(
            "admin",
            format!("Invalid admin bind_addr: {}", config.admin.bind_addr),
        ));
    }

    // 3. Validate clusters
    let mut cluster_names = HashSet::new();
    for c in &config.clusters {
        if c.name.trim().is_empty() {
            return Err(TorqueError::config("clusters", "Cluster name cannot be empty"));
        }
        if !cluster_names.insert(&c.name) {
            return Err(TorqueError::config(
                "clusters",
                format!("Duplicate cluster name: {}", c.name),
            ));
        }

        if c.endpoints.is_empty() {
            return Err(TorqueError::config(
                "clusters",
                format!("Cluster '{}' must have at least one endpoint", c.name),
            ));
        }

        for ep in &c.endpoints {
            if ep.addr.parse::<SocketAddr>().is_err() {
                return Err(TorqueError::config(
                    "clusters",
                    format!("Invalid endpoint socket address in cluster '{}': {}", c.name, ep.addr),
                ));
            }
        }
    }

    // 4. Validate routes
    let mut route_names = HashSet::new();
    for r in &config.routes {
        if r.name.trim().is_empty() {
            return Err(TorqueError::config("routes", "Route name cannot be empty"));
        }
        if !route_names.insert(&r.name) {
            return Err(TorqueError::config(
                "routes",
                format!("Duplicate route name: {}", r.name),
            ));
        }

        if !r.path.starts_with('/') {
            return Err(TorqueError::config(
                "routes",
                format!("Route path must start with '/': {}", r.path),
            ));
        }

        if !cluster_names.contains(&r.cluster) {
            return Err(TorqueError::config(
                "routes",
                format!(
                    "Route '{}' targets unknown upstream cluster '{}'",
                    r.name, r.cluster
                ),
            ));
        }
    }

    Ok(())
}
