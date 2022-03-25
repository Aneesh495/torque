//! Dynamic configuration manager with atomic hot-reloading.

use arc_swap::ArcSwap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use torque_core::error::Result;

use super::loader::load_from_file;
use super::schema::TorqueConfig;

/// Manages active configuration snapshot and triggers zero-downtime hot reloads.
pub struct ConfigManager {
    path: PathBuf,
    current: ArcSwap<TorqueConfig>,
}

impl ConfigManager {
    /// Initialize configuration manager by loading config from file.
    pub fn from_file(path: impl AsRef<Path>) -> Result<Arc<Self>> {
        let path = path.as_ref().to_path_buf();
        let config = load_from_file(&path)?;

        Ok(Arc::new(Self {
            path,
            current: ArcSwap::from_pointee(config),
        }))
    }

    /// Construct directly with in-memory initial configuration.
    pub fn from_config(config: TorqueConfig, path: impl AsRef<Path>) -> Arc<Self> {
        Arc::new(Self {
            path: path.as_ref().to_path_buf(),
            current: ArcSwap::from_pointee(config),
        })
    }

    /// Trigger a hot-reload by re-reading from disk and atomically swapping the pointer.
    pub fn reload(&self) -> Result<Arc<TorqueConfig>> {
        let new_config = load_from_file(&self.path)?;
        let arc = Arc::new(new_config);
        self.current.store(arc.clone());
        tracing::info!(path = %self.path.display(), "Torque configuration hot-reloaded successfully");
        Ok(arc)
    }

    /// Retrieve the current active configuration snapshot.
    pub fn current(&self) -> Arc<TorqueConfig> {
        self.current.load_full()
    }

    /// Path to the configuration file on disk.
    pub fn path(&self) -> &Path {
        &self.path
    }
}
