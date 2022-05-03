//! Configuration file loader supporting YAML, JSON, and TOML.

use std::fs;
use std::path::Path;
use torque_core::error::{Result, TorqueError};

use super::schema::TorqueConfig;
use super::validator::validate_config;

/// File serialization format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigFormat {
    Yaml,
    Json,
    Toml,
}

impl ConfigFormat {
    /// Infer format from file path extension.
    pub fn from_path(path: &Path) -> Self {
        match path.extension().and_then(|s| s.to_str()) {
            Some("json") => ConfigFormat::Json,
            Some("toml") => ConfigFormat::Toml,
            _ => ConfigFormat::Yaml,
        }
    }
}

/// Parse configuration from a raw string slice according to format.
pub fn load_from_str(content: &str, format: ConfigFormat) -> Result<TorqueConfig> {
    let config: TorqueConfig = match format {
        ConfigFormat::Yaml => serde_yaml::from_str(content)
            .map_err(|e| TorqueError::config("yaml_parser", format!("YAML parse error: {e}")))?,
        ConfigFormat::Json => serde_json::from_str(content)
            .map_err(|e| TorqueError::config("json_parser", format!("JSON parse error: {e}")))?,
        ConfigFormat::Toml => toml::from_str(content)
            .map_err(|e| TorqueError::config("toml_parser", format!("TOML parse error: {e}")))?,
    };

    validate_config(&config)?;
    Ok(config)
}

/// Read and parse configuration file from the filesystem.
pub fn load_from_file(path: impl AsRef<Path>) -> Result<TorqueConfig> {
    let path = path.as_ref();
    let content = fs::read_to_string(path).map_err(|e| {
        TorqueError::config(
            "file_loader",
            format!("Cannot read config file '{}': {}", path.display(), e),
        )
    })?;

    let format = ConfigFormat::from_path(path);
    load_from_str(&content, format)
}
