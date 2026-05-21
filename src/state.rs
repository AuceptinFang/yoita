use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use anyhow::Context;
use serde::{Deserialize, Serialize};

use crate::{
    config::{ModConfig, ModSource},
    error::Result,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SyncState {
    #[serde(default = "default_format_version")]
    pub format_version: u32,
    #[serde(default)]
    pub mods: BTreeMap<String, ManagedModState>,
}

impl Default for SyncState {
    fn default() -> Self {
        Self {
            format_version: default_format_version(),
            mods: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManagedModState {
    pub source_kind: String,
    pub source_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    pub source_path: PathBuf,
    pub mount_path: PathBuf,
}

impl SyncState {
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }

        let content = fs::read_to_string(path)
            .with_context(|| format!("failed to read state file `{}`", path.display()))?;

        Ok(toml::from_str(&content)
            .with_context(|| format!("failed to parse state file `{}`", path.display()))?)
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!("failed to create state file parent `{}`", parent.display())
            })?;
        }

        let content = toml::to_string_pretty(self).context("failed to serialize sync state")?;
        fs::write(path, content)
            .with_context(|| format!("failed to write state file `{}`", path.display()))?;

        Ok(())
    }
}

impl ManagedModState {
    pub fn from_mod(entry: &ModConfig, source_path: PathBuf, mount_path: PathBuf) -> Self {
        let (source_kind, source_id) = match &entry.source {
            ModSource::Steam { id } => ("steam".to_owned(), id.clone()),
            ModSource::Custom { url } => ("custom".to_owned(), url.as_str().to_owned()),
        };

        Self {
            source_kind,
            source_id,
            version: entry.version.clone(),
            source_path,
            mount_path,
        }
    }
}

fn default_format_version() -> u32 {
    1
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use crate::{
        config::{ModConfig, ModSource},
        state::{ManagedModState, SyncState},
    };

    #[test]
    fn state_round_trips() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("yoita-state-{}-{unique}", std::process::id()));
        let state_path = root.join("state.toml");

        let mut state = SyncState::default();
        state.mods.insert(
            "edit-always".to_owned(),
            ManagedModState::from_mod(
                &ModConfig {
                    name: "edit-always".to_owned(),
                    version: None,
                    enabled: true,
                    source: ModSource::Steam {
                        id: "edit-always".to_owned(),
                    },
                },
                root.join("cache/edit-always.zip"),
                root.join("mods/edit-always.zip"),
            ),
        );

        state.save(&state_path).unwrap();
        let loaded = SyncState::load(&state_path).unwrap();

        assert_eq!(loaded, state);

        std::fs::remove_dir_all(root).unwrap();
    }
}
