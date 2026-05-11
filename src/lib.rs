pub mod config;
pub mod error;
pub mod file;
pub mod steam;
pub mod toml;

use anyhow::{Context, anyhow};
use reqwest::{Client, Url};

use crate::{
    config::{ModConfig, ModSource, YoitaConfig},
    error::Result,
    file::WorkspaceLayout,
    steam::SteamClient,
};

#[derive(Debug, Clone)]
pub struct Yoita {
    layout: WorkspaceLayout,
    steam: Option<SteamClient>,
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

        let steam = config
            .steam
            .as_ref()
            .map(|steam| SteamClient::new(http.clone(), steam))
            .transpose()?;

        Ok(Self {
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
}
