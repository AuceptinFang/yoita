pub mod config;
pub mod error;
pub mod file;
pub mod steam;
pub mod toml;

use anyhow::{Context, anyhow};
use reqwest::{Client, Url};
use std::{fs, path::PathBuf};

use crate::{
    config::{ModConfig, ModSource, YoitaConfig},
    error::Result,
    file::WorkspaceLayout,
    steam::SteamClient,
};

#[derive(Debug, Clone)]
pub struct Yoita {
    http: Client,
    layout: WorkspaceLayout,
    steam: Option<SteamClient>,
}

#[derive(Debug, Clone)]
pub struct CachedMod {
    pub name: String,
    pub download_url: Url,
    pub archive_path: PathBuf,
    pub bytes: u64,
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

    pub async fn sync(&self, config: &YoitaConfig) -> Result<Vec<CachedMod>> {
        self.layout.prepare()?;

        let mut cached = Vec::new();
        for entry in self.enabled_mods(config)? {
            cached.push(self.download_mod(entry).await?);
        }

        Ok(cached)
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

    async fn download_mod(&self, entry: &ModConfig) -> Result<CachedMod> {
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
            .get(download_url.clone())
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

        Ok(CachedMod {
            name: entry.name.clone(),
            download_url,
            archive_path,
            bytes: bytes.len() as u64,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use reqwest::Url;

    use crate::{
        config::{ModConfig, ModSource, RuntimeConfig, SteamConfig, YoitaConfig},
        file::WorkspaceLayout,
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
    fn prepare_creates_workspace_directories() {
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

        std::fs::remove_dir_all(root).unwrap();
    }
}
