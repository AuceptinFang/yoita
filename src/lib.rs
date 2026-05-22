pub mod config;
pub mod error;
pub mod file;
pub mod state;
pub mod steam;
pub mod toml;

use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, anyhow};
use reqwest::{Client, Url};

use crate::{
    config::{ModConfig, ModSource, YoitaConfig},
    error::Result,
    file::WorkspaceLayout,
    state::{ManagedModState, SyncState},
    steam::{
        SteamCmdConfig, SteamContext, WorkshopContentRequest, WorkshopItemDetails, WorkshopItemId,
        WorkshopItemRef, WorkshopSearchRequest,
    },
};

#[derive(Debug, Clone)]
pub struct Yoita {
    http: Client,
    layout: WorkspaceLayout,
    steam: Option<SteamContext>,
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

        let steam = config
            .steam
            .as_ref()
            .map(|steam| {
                let steamcmd = SteamCmdConfig::try_from(steam)
                    .context("failed to build steamcmd runtime config")?;
                Ok::<SteamContext, anyhow::Error>(SteamContext::steamcmd(steamcmd))
            })
            .transpose()?;

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
        Ok(config.mods.iter().filter(|entry| entry.enabled).collect())
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

    async fn resolve_mod(&self, entry: &ModConfig) -> Result<ResolvedMod> {
        match &entry.source {
            ModSource::Steam { id } => self.resolve_steam_mod(entry, id).await,
            ModSource::Custom { url } => self.download_custom_mod(entry, url).await,
        }
    }

    async fn resolve_steam_mod(&self, entry: &ModConfig, id: &str) -> Result<ResolvedMod> {
        let steam = self.steam.as_ref().ok_or_else(|| {
            anyhow!(
                "mod `{}` uses steam source but `[steam]` is missing",
                entry.name
            )
        })?;

        // 尝试获取workshop_id, 用户直接提供/用户提供mod名+搜索
        let workshop_id = match id.parse::<WorkshopItemId>() {
            Ok(id) => id,
            Err(_) => self
                .resolve_steam_workshop_id_by_name(steam, entry, id)
                .await
                .with_context(|| {
                    format!(
                        "failed to resolve steam workshop id for mod `{}` from name `{}`",
                        entry.name, id
                    )
                })?,
        };
        let content = steam
            .content()
            .ensure_content(WorkshopContentRequest {
                item: WorkshopItemRef::new(steam.app_id(), workshop_id),
            })
            .await
            .with_context(|| format!("failed to resolve steam content for mod `{}`", entry.name))?;

        Ok(ResolvedMod {
            bytes: path_size(&content.source_path)?,
            source_path: content.source_path,
        })
    }

    async fn resolve_steam_workshop_id_by_name(
        &self,
        steam: &SteamContext,
        entry: &ModConfig,
        query: &str,
    ) -> Result<WorkshopItemId> {
        let results = steam
            .metadata()
            .search_items(WorkshopSearchRequest {
                app_id: steam.app_id(),
                query: query.to_owned(),
                limit: 5,
            })
            .await?;

        let selected = select_workshop_match(query, &results).ok_or_else(|| {
            anyhow!(
                "steam workshop search returned no results for mod `{}`",
                entry.name
            )
        })?;

        if results.len() > 1 {
            tracing::warn!(
                mod_name = %entry.name,
                query,
                chosen = %selected.item.workshop_id,
                candidates = results.len(),
                "steam name lookup returned multiple matches; choosing best current match"
            );
        }

        Ok(selected.item.workshop_id)
    }

    async fn download_custom_mod(
        &self,
        entry: &ModConfig,
        download_url: &Url,
    ) -> Result<ResolvedMod> {
        let archive_path = self.layout.custom_archive_path(entry, download_url);

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

fn path_size(path: &Path) -> Result<u64> {
    let metadata = fs::metadata(path)
        .with_context(|| format!("failed to read metadata for `{}`", path.display()))?;

    if metadata.is_file() {
        return Ok(metadata.len());
    }

    let mut total = 0;
    for entry in fs::read_dir(path)
        .with_context(|| format!("failed to read directory `{}`", path.display()))?
    {
        let entry =
            entry.with_context(|| format!("failed to read entry in `{}`", path.display()))?;
        total += path_size(&entry.path())?;
    }

    Ok(total)
}

fn select_workshop_match<'a>(
    query: &str,
    results: &'a [WorkshopItemDetails],
) -> Option<&'a WorkshopItemDetails> {
    let normalized_query = normalize_workshop_query(query);
    results
        .iter()
        .find(|item| {
            item.title
                .as_deref()
                .map(normalize_workshop_query)
                .as_deref()
                == Some(normalized_query.as_str())
        })
        .or_else(|| results.first())
}

fn normalize_workshop_query(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeSet,
        path::Path,
        sync::{Arc, Mutex},
        time::{SystemTime, UNIX_EPOCH},
    };

    use reqwest::{Client, Url};

    use crate::{
        config::{ModConfig, ModSource, SteamConfig, YoitaConfig},
        file::WorkspaceLayout,
        state::{ManagedModState, SyncState},
        steam::{
            SteamAppId, SteamContext, SteamServices, WorkshopContentKind, WorkshopContentProvider,
            WorkshopContentRequest, WorkshopItemContent, WorkshopItemDetails, WorkshopItemId,
            WorkshopItemRef, WorkshopMetadataProvider, WorkshopMetadataRequest,
            WorkshopSearchRequest,
        },
    };

    use super::Yoita;

    #[test]
    fn custom_archive_path_is_stable() {
        let layout = WorkspaceLayout {
            cache_dir: std::path::PathBuf::from("/tmp/yoita-cache"),
            staging_dir: std::path::PathBuf::from("/tmp/yoita-staging"),
            mount_dir: std::path::PathBuf::from("/tmp/yoita-mount"),
        };
        let mod_config = ModConfig {
            name: "Reference Custom Mod".to_owned(),
            version: Some("1.0.0".to_owned()),
            enabled: true,
            source: ModSource::Custom {
                url: Url::parse("https://example.invalid/mods/reference-custom-mod.zip").unwrap(),
            },
        };

        let path = layout.custom_archive_path(
            &mod_config,
            &Url::parse("https://example.invalid/mods/reference-custom-mod.zip").unwrap(),
        );

        assert_eq!(
            path,
            std::path::PathBuf::from("/tmp/yoita-cache/custom/reference-custom-mod-1.0.0.zip")
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
                id: "1234567890".to_owned(),
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

    #[tokio::test]
    async fn steam_source_searches_by_name_when_id_is_not_numeric() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("yoita-name-search-{}-{unique}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("mod.xml"), b"<Mod />").unwrap();

        let metadata = Arc::new(FakeMetadataProvider {
            results: vec![WorkshopItemDetails {
                item: WorkshopItemRef::new(SteamAppId::NOITA, WorkshopItemId(2572385079)),
                title: Some("wanddbg".to_owned()),
                description: None,
                preview_url: None,
                file_type: None,
                time_created: None,
                time_updated: None,
            }],
            seen_query: Mutex::new(None),
        });
        let content = Arc::new(FakeContentProvider {
            expected_item: WorkshopItemRef::new(SteamAppId::NOITA, WorkshopItemId(2572385079)),
            source_path: root.clone(),
        });

        let app = Yoita {
            http: Client::builder().build().unwrap(),
            layout: WorkspaceLayout {
                cache_dir: std::env::temp_dir().join("yoita-name-search-cache"),
                staging_dir: std::env::temp_dir().join("yoita-name-search-staging"),
                mount_dir: std::env::temp_dir().join("yoita-name-search-mount"),
            },
            steam: Some(SteamContext::new(
                crate::steam::SteamCmdConfig {
                    steamcmd_path: "steamcmd".into(),
                    force_install_dir: ".yoita/steamcmd".into(),
                    app_id: SteamAppId::NOITA,
                    login: crate::steam::SteamLoginMode::Anonymous,
                    timeout: std::time::Duration::from_secs(30),
                },
                SteamServices::new(metadata.clone(), content),
            )),
        };
        let mod_config = ModConfig {
            name: "wanddbg".to_owned(),
            version: None,
            enabled: true,
            source: ModSource::Steam {
                id: "wanddbg".to_owned(),
            },
        };

        let resolved = app.resolve_mod(&mod_config).await.unwrap();

        assert_eq!(resolved.source_path, root);
        assert_eq!(
            *metadata.seen_query.lock().unwrap(),
            Some("wanddbg".to_owned())
        );

        std::fs::remove_dir_all(resolved.source_path).unwrap();
    }

    #[tokio::test]
    async fn steam_source_reports_command_failure() {
        let config = YoitaConfig {
            config: Default::default(),
            steam: Some(SteamConfig {
                backend: crate::config::SteamBackend::SteamCmd,
                steamcmd_path: "/definitely/missing/steamcmd".into(),
                force_install_dir: ".yoita/steamcmd".into(),
                app_id: 881100,
                timeout_secs: 300,
                login: crate::config::SteamLoginConfig::Anonymous,
                username: None,
                password_env: None,
            }),
            mods: vec![ModConfig {
                name: "wanddbg".to_owned(),
                version: None,
                enabled: true,
                source: ModSource::Steam {
                    id: "3454128340".to_owned(),
                },
            }],
        };

        let app = Yoita::from_config(&config).unwrap();
        let error = app.resolve_mod(&config.mods[0]).await.unwrap_err();
        let chain = format!("{error:#}");

        assert!(chain.contains("failed to resolve steam content for mod `wanddbg`"));
        assert!(chain.contains("failed to spawn"));
    }

    #[derive(Debug)]
    struct FakeMetadataProvider {
        results: Vec<WorkshopItemDetails>,
        seen_query: Mutex<Option<String>>,
    }

    impl WorkshopMetadataProvider for FakeMetadataProvider {
        fn fetch_item<'a>(
            &'a self,
            _request: WorkshopMetadataRequest,
        ) -> crate::steam::SteamFuture<'a, crate::error::Result<Option<WorkshopItemDetails>>>
        {
            Box::pin(async { Ok(None) })
        }

        fn search_items<'a>(
            &'a self,
            request: WorkshopSearchRequest,
        ) -> crate::steam::SteamFuture<'a, crate::error::Result<Vec<WorkshopItemDetails>>> {
            Box::pin(async move {
                *self.seen_query.lock().unwrap() = Some(request.query);
                Ok(self.results.clone())
            })
        }
    }

    #[derive(Debug)]
    struct FakeContentProvider {
        expected_item: WorkshopItemRef,
        source_path: std::path::PathBuf,
    }

    impl WorkshopContentProvider for FakeContentProvider {
        fn ensure_content<'a>(
            &'a self,
            request: WorkshopContentRequest,
        ) -> crate::steam::SteamFuture<'a, crate::error::Result<WorkshopItemContent>> {
            Box::pin(async move {
                assert_eq!(request.item, self.expected_item);
                Ok(WorkshopItemContent {
                    item: request.item,
                    source_path: self.source_path.clone(),
                    kind: WorkshopContentKind::Directory,
                })
            })
        }
    }
}
