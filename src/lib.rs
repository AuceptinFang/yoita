pub mod config;
pub mod error;
pub mod file;
pub mod state;
pub mod steam;
pub mod toml;

use std::{collections::BTreeSet, fs, path::PathBuf};

use anyhow::{Context, anyhow};
use reqwest::{Client, Url};

use crate::{
    config::{ModConfig, ModSource, YoitaConfig},
    error::Result,
    file::WorkspaceLayout,
    state::{ManagedModState, SyncState},
    steam::SteamClient,
};

#[derive(Debug, Clone)]
pub struct Yoita {
    http: Client,
    layout: WorkspaceLayout,
    steam: Option<SteamClient>,
}

#[derive(Debug, Clone)]
pub struct SyncedMod {
    pub name: String,
    pub source_path: PathBuf,
    pub mount_path: PathBuf,
    pub bytes: u64,
}

#[derive(Debug, Clone)]
pub struct SyncReport {
    pub mods: Vec<SyncedMod>,
    pub removed_mounts: Vec<PathBuf>,
    pub state_path: PathBuf,
}

#[derive(Debug, Clone)]
struct ResolvedMod {
    source_path: PathBuf,
    bytes: u64,
}

impl Yoita {
    pub fn from_config(config: &YoitaConfig) -> Result<Self> {
        let http = Client::builder()
            .user_agent(concat!(
                env!("CARGO_PKG_NAME"),
                "/",
                env!("CARGO_PKG_VERSION")
            ))
            .build()
            .context("failed to build HTTP client")?;

        let steam = config.steam.as_ref().map(SteamClient::new).transpose()?;

        Ok(Self {
            http,
            layout: WorkspaceLayout::from_config(&config.config),
            steam,
        })
    }

    pub fn layout(&self) -> &WorkspaceLayout {
        &self.layout
    }

    pub fn enabled_mods<'a>(&self, config: &'a YoitaConfig) -> Result<Vec<&'a ModConfig>> {
        config
            .mods
            .iter()
            .filter(|entry| entry.enabled)
            .map(|entry| {
                self.download_url(entry)?;
                Ok(entry)
            })
            .collect()
    }

    pub async fn sync(&self, config: &YoitaConfig) -> Result<SyncReport> {
        self.layout.prepare()?;

        let state_path = self.layout.state_path();
        let mut state = SyncState::load(&state_path)?;
        let enabled_mods = self.enabled_mods(config)?;
        let desired_names = enabled_mods
            .iter()
            .map(|entry| entry.name.clone())
            .collect::<BTreeSet<_>>();

        let mut synced = Vec::new();
        for entry in enabled_mods {
            let previous = state.mods.get(&entry.name).cloned();
            let resolved = self.resolve_mod(entry).await?;
            let mount_path = self.layout.mount_path(entry, &resolved.source_path);

            self.layout
                .sync_to_mount(&resolved.source_path, &mount_path)?;

            if let Some(previous) = previous {
                if previous.mount_path != mount_path {
                    self.layout.remove_mount_path(&previous.mount_path)?;
                }
            }

            state.mods.insert(
                entry.name.clone(),
                ManagedModState::from_mod(entry, resolved.source_path.clone(), mount_path.clone()),
            );

            synced.push(SyncedMod {
                name: entry.name.clone(),
                source_path: resolved.source_path,
                mount_path,
                bytes: resolved.bytes,
            });
        }

        let removed_mounts = self.prune_removed_mounts(&mut state, &desired_names)?;
        state.save(&state_path)?;

        Ok(SyncReport {
            mods: synced,
            removed_mounts,
            state_path,
        })
    }

    pub fn download_url(&self, entry: &ModConfig) -> Result<Url> {
        match &entry.source {
            ModSource::Steam { workshop_id } => {
                let steam = self.steam.as_ref().ok_or_else(|| {
                    anyhow!(
                        "mod `{}` uses steam source but `[steam]` is missing",
                        entry.name
                    )
                })?;

                Ok(steam.download_url(workshop_id).with_context(|| {
                    format!(
                        "failed to resolve steam download url for mod `{}`",
                        entry.name
                    )
                })?)
            }
            ModSource::Custom { url } => Ok(url.clone()),
        }
    }

    async fn resolve_mod(&self, entry: &ModConfig) -> Result<ResolvedMod> {
        self.download_mod(entry).await
    }

    async fn download_mod(&self, entry: &ModConfig) -> Result<ResolvedMod> {
        let download_url = self.download_url(entry)?;
        let archive_path = self.layout.archive_path(entry, &download_url);

        if let Some(parent) = archive_path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!(
                    "failed to create cache directory for mod `{}` at `{}`",
                    entry.name,
                    parent.display()
                )
            })?;
        }

        let response = self
            .http
            .get(download_url)
            .send()
            .await
            .with_context(|| format!("failed to request mod `{}`", entry.name))?
            .error_for_status()
            .with_context(|| format!("download server rejected mod `{}`", entry.name))?;

        let bytes = response
            .bytes()
            .await
            .with_context(|| format!("failed to read response body for mod `{}`", entry.name))?;

        tokio::fs::write(&archive_path, &bytes)
            .await
            .with_context(|| {
                format!(
                    "failed to write downloaded mod `{}` to `{}`",
                    entry.name,
                    archive_path.display()
                )
            })?;

        Ok(ResolvedMod {
            source_path: archive_path,
            bytes: bytes.len() as u64,
        })
    }

    fn prune_removed_mounts(
        &self,
        state: &mut SyncState,
        desired_names: &BTreeSet<String>,
    ) -> Result<Vec<PathBuf>> {
        let stale_mods = state
            .mods
            .keys()
            .filter(|name| !desired_names.contains(*name))
            .cloned()
            .collect::<Vec<_>>();
        let mut removed_mounts = Vec::new();

        for name in stale_mods {
            if let Some(previous) = state.mods.remove(&name) {
                self.layout.remove_mount_path(&previous.mount_path)?;
                removed_mounts.push(previous.mount_path);
            }
        }

        Ok(removed_mounts)
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeSet,
        path::Path,
        time::{SystemTime, UNIX_EPOCH},
    };

    use reqwest::{Client, Url};

    use crate::{
        config::{ModConfig, ModSource, RuntimeConfig, SteamConfig, YoitaConfig},
        file::WorkspaceLayout,
        state::{ManagedModState, SyncState},
    };

    use super::Yoita;

    #[test]
    fn download_url_builds_workshop_query() {
        let config = YoitaConfig {
            config: RuntimeConfig::default(),
            steam: Some(SteamConfig {
                download_endpoint: Url::parse("https://example.invalid/workshop/download").unwrap(),
                api_key: Some("secret".to_owned()),
            }),
            mods: vec![ModConfig {
                name: "reference-steam-mod".to_owned(),
                version: Some("latest".to_owned()),
                enabled: true,
                source: ModSource::Steam {
                    workshop_id: "1234567890".to_owned(),
                },
            }],
        };

        let app = Yoita::from_config(&config).unwrap();
        let url = app.download_url(&config.mods[0]).unwrap();

        assert_eq!(
            url.as_str(),
            "https://example.invalid/workshop/download?id=1234567890&key=secret"
        );
    }

    #[test]
    fn archive_path_is_stable_for_steam_mod() {
        let layout = WorkspaceLayout {
            cache_dir: std::path::PathBuf::from("/tmp/yoita-cache"),
            staging_dir: std::path::PathBuf::from("/tmp/yoita-staging"),
            mount_dir: std::path::PathBuf::from("/tmp/yoita-mount"),
        };
        let mod_config = ModConfig {
            name: "Reference Steam Mod".to_owned(),
            version: Some("1.0.0".to_owned()),
            enabled: true,
            source: ModSource::Steam {
                workshop_id: "1234567890".to_owned(),
            },
        };

        let path = layout.archive_path(
            &mod_config,
            &Url::parse("https://example.invalid/workshop/download").unwrap(),
        );

        assert_eq!(
            path,
            std::path::PathBuf::from(
                "/tmp/yoita-cache/steam/1234567890/reference-steam-mod-1.0.0.zip"
            )
        );
    }

    #[test]
    fn mount_path_is_stable_for_archive_file() {
        let layout = WorkspaceLayout {
            cache_dir: std::path::PathBuf::from("/tmp/yoita-cache"),
            staging_dir: std::path::PathBuf::from("/tmp/yoita-staging"),
            mount_dir: std::path::PathBuf::from("/tmp/yoita-mount"),
        };
        let mod_config = ModConfig {
            name: "Reference Steam Mod".to_owned(),
            version: Some("1.0.0".to_owned()),
            enabled: true,
            source: ModSource::Steam {
                workshop_id: "1234567890".to_owned(),
            },
        };

        let path = layout.mount_path(
            &mod_config,
            Path::new("/tmp/yoita-cache/steam/1234567890/reference-steam-mod-1.0.0.zip"),
        );

        assert_eq!(
            path,
            std::path::PathBuf::from("/tmp/yoita-mount/reference-steam-mod.zip")
        );
    }

    #[test]
    fn prepare_creates_workspace_and_state_directories() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("yoita-test-{}-{unique}", std::process::id()));
        let layout = WorkspaceLayout {
            cache_dir: root.join("cache"),
            staging_dir: root.join("staging"),
            mount_dir: root.join("mount"),
        };

        layout.prepare().unwrap();

        assert!(layout.cache_dir.exists());
        assert!(layout.staging_dir.exists());
        assert!(layout.mount_dir.exists());
        assert!(layout.state_path().parent().unwrap().exists());

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn prune_removed_mounts_only_touches_managed_mount_results() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("yoita-prune-{}-{unique}", std::process::id()));
        let layout = WorkspaceLayout {
            cache_dir: root.join("cache"),
            staging_dir: root.join("staging"),
            mount_dir: root.join("mount"),
        };
        layout.prepare().unwrap();

        let stale_mount = layout.mount_dir.join("edit-always.zip");
        std::fs::write(&stale_mount, b"old mod").unwrap();

        let mut state = SyncState::default();
        state.mods.insert(
            "edit-always".to_owned(),
            ManagedModState {
                source_kind: "steam".to_owned(),
                source_id: "edit-always".to_owned(),
                version: None,
                source_path: layout
                    .cache_dir
                    .join("steam/edit-always/edit-always-latest.zip"),
                mount_path: stale_mount.clone(),
            },
        );

        let app = Yoita {
            http: Client::builder().build().unwrap(),
            layout: layout.clone(),
            steam: None,
        };

        let removed = app
            .prune_removed_mounts(&mut state, &BTreeSet::new())
            .unwrap();

        assert_eq!(removed, vec![stale_mount.clone()]);
        assert!(state.mods.is_empty());
        assert!(!stale_mount.exists());

        std::fs::remove_dir_all(root).unwrap();
    }
}
