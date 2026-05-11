use std::{fs, path::PathBuf};

use anyhow::Context;
use reqwest::Url;

use crate::{
    config::{ModConfig, ModSource, RuntimeConfig},
    error::Result,
};

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

    pub fn prepare(&self) -> Result<()> {
        for dir in [&self.cache_dir, &self.staging_dir, &self.mount_dir] {
            fs::create_dir_all(dir).with_context(|| {
                format!("failed to create workspace directory `{}`", dir.display())
            })?;
        }

        Ok(())
    }

    pub fn archive_path(&self, entry: &ModConfig, download_url: &Url) -> PathBuf {
        let extension = infer_extension(download_url);
        let version = sanitize_fragment(entry.version.as_deref().unwrap_or("latest"), "latest");
        let name = sanitize_fragment(&entry.name, "mod");

        match &entry.source {
            ModSource::Steam { workshop_id } => self
                .cache_dir
                .join("steam")
                .join(sanitize_fragment(workshop_id, "workshop"))
                .join(format!("{name}-{version}.{extension}")),
            ModSource::Custom { .. } => self
                .cache_dir
                .join("custom")
                .join(format!("{name}-{version}.{extension}")),
        }
    }
}

fn infer_extension(download_url: &Url) -> String {
    download_url
        .path_segments()
        .and_then(|mut segments| segments.next_back())
        .and_then(|segment| segment.rsplit_once('.'))
        .map(|(_, ext)| ext.trim())
        .filter(|ext| !ext.is_empty())
        .map(|ext| sanitize_fragment(ext, "zip"))
        .unwrap_or_else(|| "zip".to_owned())
}

fn sanitize_fragment(value: &str, fallback: &str) -> String {
    let mut sanitized = String::with_capacity(value.len());
    let mut last_was_sep = false;

    for ch in value.chars() {
        let normalized = if ch.is_ascii_alphanumeric() {
            last_was_sep = false;
            ch.to_ascii_lowercase()
        } else if matches!(ch, '.' | '_' | '-') {
            last_was_sep = false;
            ch
        } else if last_was_sep {
            continue;
        } else {
            last_was_sep = true;
            '-'
        };

        sanitized.push(normalized);
    }

    let trimmed = sanitized.trim_matches(['-', '.', '_']).to_owned();

    if trimmed.is_empty() {
        fallback.to_owned()
    } else {
        trimmed
    }
}
