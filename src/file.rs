use std::{
    fs,
    path::{Path, PathBuf},
};

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

        if let Some(parent) = self.state_path().parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!("failed to create state directory `{}`", parent.display())
            })?;
        }

        Ok(())
    }

    pub fn state_path(&self) -> PathBuf {
        self.cache_dir
            .parent()
            .map(|parent| parent.join("state.toml"))
            .unwrap_or_else(|| PathBuf::from("state.toml"))
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

    pub fn mount_path(&self, entry: &ModConfig, source_path: &Path) -> PathBuf {
        let name = sanitize_fragment(&entry.name, "mod");

        if source_path.is_dir() {
            self.mount_dir.join(name)
        } else {
            let extension = infer_path_extension(source_path).unwrap_or_else(|| "zip".to_owned());
            self.mount_dir.join(format!("{name}.{extension}"))
        }
    }

    pub fn sync_to_mount(&self, source_path: &Path, mount_path: &Path) -> Result<()> {
        self.ensure_managed_mount_path(mount_path)?;

        if let Some(parent) = mount_path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!(
                    "failed to create mount parent directory `{}`",
                    parent.display()
                )
            })?;
        }

        if mount_path.exists() {
            remove_path(mount_path).with_context(|| {
                format!("failed to replace mount path `{}`", mount_path.display())
            })?;
        }

        if source_path.is_dir() {
            copy_dir_all(source_path, mount_path).with_context(|| {
                format!(
                    "failed to sync mounted directory from `{}` to `{}`",
                    source_path.display(),
                    mount_path.display()
                )
            })?;
        } else {
            fs::copy(source_path, mount_path).with_context(|| {
                format!(
                    "failed to sync mounted file from `{}` to `{}`",
                    source_path.display(),
                    mount_path.display()
                )
            })?;
        }

        Ok(())
    }

    pub fn remove_mount_path(&self, mount_path: &Path) -> Result<()> {
        self.ensure_managed_mount_path(mount_path)?;

        if mount_path.exists() {
            remove_path(mount_path).with_context(|| {
                format!("failed to remove mount path `{}`", mount_path.display())
            })?;
        }

        Ok(())
    }

    fn ensure_managed_mount_path(&self, mount_path: &Path) -> Result<()> {
        if !mount_path.starts_with(&self.mount_dir) {
            return Err(anyhow::anyhow!(
                "refusing to manage path `{}` outside mount dir `{}`",
                mount_path.display(),
                self.mount_dir.display()
            )
            .into());
        }

        Ok(())
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

fn infer_path_extension(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(str::trim)
        .filter(|ext| !ext.is_empty())
        .map(|ext| sanitize_fragment(ext, "zip"))
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

fn copy_dir_all(source: &Path, target: &Path) -> Result<()> {
    fs::create_dir_all(target)
        .with_context(|| format!("failed to create directory `{}`", target.display()))?;

    for entry in fs::read_dir(source)
        .with_context(|| format!("failed to read directory `{}`", source.display()))?
    {
        let entry =
            entry.with_context(|| format!("failed to read entry in `{}`", source.display()))?;
        let entry_type = entry.file_type().with_context(|| {
            format!("failed to read file type for `{}`", entry.path().display())
        })?;
        let destination = target.join(entry.file_name());

        if entry_type.is_dir() {
            copy_dir_all(&entry.path(), &destination)?;
        } else {
            fs::copy(entry.path(), &destination).with_context(|| {
                format!(
                    "failed to copy file `{}` to `{}`",
                    entry.path().display(),
                    destination.display()
                )
            })?;
        }
    }

    Ok(())
}

fn remove_path(path: &Path) -> Result<()> {
    if path.is_dir() {
        fs::remove_dir_all(path)
            .with_context(|| format!("failed to remove directory `{}`", path.display()))?;
    } else {
        fs::remove_file(path)
            .with_context(|| format!("failed to remove file `{}`", path.display()))?;
    }

    Ok(())
}
