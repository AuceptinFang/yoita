use std::path::PathBuf;

use crate::config::RuntimeConfig;

#[derive(Debug, Clone)]
pub struct WorkspaceLayout {
    pub cache_dir: PathBuf,
    pub staging_dir: PathBuf,
    pub mount_dir: PathBuf,
}

impl WorkspaceLayout {
    pub fn from_config(config: &RuntimeConfig) -> Self {
        Self {
            cache_dir: config.cache_dir.clone(),
            staging_dir: config.staging_dir.clone(),
            mount_dir: config.mount_dir.clone(),
        }
    }
}
