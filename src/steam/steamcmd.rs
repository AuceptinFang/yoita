use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use anyhow::Context;

use crate::config::{SteamBackend, SteamConfig, SteamLoginConfig};
use crate::error::Result;

use super::{
    CommandRequest, CommandRunner, SteamFuture, WorkshopContentProvider, WorkshopContentRequest,
    WorkshopItemContent, WorkshopItemId, content_kind_for_path,
};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum SteamLoginMode {
    #[default]
    Anonymous,
    Account {
        username: String,
        password_env: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SteamCmdConfig {
    pub steamcmd_path: PathBuf,
    pub force_install_dir: PathBuf,
    pub app_id: super::SteamAppId,
    pub login: SteamLoginMode,
    pub timeout: Duration,
}

impl SteamCmdConfig {
    pub fn workshop_content_dir(&self, workshop_id: WorkshopItemId) -> PathBuf {
        self.force_install_dir
            .join("steamapps")
            .join("workshop")
            .join("content")
            .join(self.app_id.to_string())
            .join(workshop_id.to_string())
    }
}

#[derive(Debug, Clone)]
pub(crate) struct SteamCmdContentProvider {
    config: SteamCmdConfig,
    runner: Arc<dyn CommandRunner>,
}

impl SteamCmdContentProvider {
    pub fn new(config: SteamCmdConfig, runner: Arc<dyn CommandRunner>) -> Self {
        Self { config, runner }
    }

    fn content_path(&self, workshop_id: WorkshopItemId) -> PathBuf {
        self.config.workshop_content_dir(workshop_id)
    }

    fn has_content(&self, path: &Path) -> Result<bool> {
        if path.is_file() {
            return Ok(true);
        }

        if !path.is_dir() {
            return Ok(false);
        }

        Ok(fs::read_dir(path)
            .with_context(|| format!("failed to read workshop content dir `{}`", path.display()))?
            .next()
            .transpose()
            .with_context(|| format!("failed to read workshop content entry in `{}`", path.display()))?
            .is_some())
    }

    fn download_request(&self, workshop_id: WorkshopItemId) -> Result<CommandRequest> {
        let mut args = vec![
            "+force_install_dir".to_owned(),
            self.config.force_install_dir.display().to_string(),
        ];

        match &self.config.login {
            SteamLoginMode::Anonymous => {
                args.push("+login".to_owned());
                args.push("anonymous".to_owned());
            }
            SteamLoginMode::Account {
                username,
                password_env,
            } => {
                let password = std::env::var(password_env).with_context(|| {
                    format!(
                        "steam password env var `{password_env}` is not set for user `{username}`"
                    )
                })?;
                args.push("+login".to_owned());
                args.push(username.clone());
                args.push(password);
            }
        }

        args.push("+workshop_download_item".to_owned());
        args.push(self.config.app_id.to_string());
        args.push(workshop_id.to_string());
        args.push("+quit".to_owned());

        Ok(CommandRequest {
            program: self.config.steamcmd_path.clone(),
            args,
            current_dir: None,
            env: BTreeMap::new(),
            timeout: self.config.timeout,
        })
    }

    fn command_succeeded(output: &super::CommandOutput, workshop_id: WorkshopItemId) -> bool {
        let marker = format!("Success. Downloaded item {workshop_id}");
        output.stdout.contains(&marker) || output.stderr.contains(&marker)
    }
}

impl WorkshopContentProvider for SteamCmdContentProvider {
    fn ensure_content<'a>(
        &'a self,
        request: WorkshopContentRequest,
    ) -> SteamFuture<'a, Result<WorkshopItemContent>> {
        Box::pin(async move {
            let source_path = self.content_path(request.item.workshop_id);

            if !self.has_content(&source_path)? {
                fs::create_dir_all(&self.config.force_install_dir).with_context(|| {
                    format!(
                        "failed to create steamcmd install dir `{}`",
                        self.config.force_install_dir.display()
                    )
                })?;

                let output = self
                    .runner
                    .run(self.download_request(request.item.workshop_id)?)
                    .await
                    .with_context(|| {
                        format!(
                            "steamcmd failed while downloading workshop item `{}`",
                            request.item.workshop_id
                        )
                    })?;

                if !Self::command_succeeded(&output, request.item.workshop_id)
                    && !self.has_content(&source_path)?
                {
                    return Err(anyhow::anyhow!(
                        "steamcmd did not download workshop item `{}` (exit {:?})\nstdout:\n{}\nstderr:\n{}",
                        request.item.workshop_id,
                        output.exit_status,
                        output.stdout,
                        output.stderr
                    )
                    .into());
                }
            }

            Ok(WorkshopItemContent {
                item: request.item,
                kind: content_kind_for_path(&source_path),
                source_path,
            })
        })
    }
}

impl TryFrom<&SteamConfig> for SteamCmdConfig {
    type Error = anyhow::Error;

    fn try_from(config: &SteamConfig) -> std::result::Result<Self, Self::Error> {
        match config.backend {
            SteamBackend::SteamCmd => {}
        }

        let login = match config.login {
            SteamLoginConfig::Anonymous => SteamLoginMode::Anonymous,
            SteamLoginConfig::Account => SteamLoginMode::Account {
                username: config
                    .username
                    .as_deref()
                    .map(str::trim)
                    .unwrap_or_default()
                    .to_owned(),
                password_env: config
                    .password_env
                    .as_deref()
                    .map(str::trim)
                    .unwrap_or_default()
                    .to_owned(),
            },
        };

        Ok(Self {
            steamcmd_path: config.steamcmd_path.clone(),
            force_install_dir: absolute_force_install_dir(&config.force_install_dir)?,
            app_id: super::SteamAppId(config.app_id),
            login,
            timeout: Duration::from_secs(config.timeout_secs),
        })
    }
}

fn absolute_force_install_dir(path: &Path) -> std::result::Result<PathBuf, anyhow::Error> {
    if path.is_absolute() {
        return Ok(path.to_path_buf());
    }

    Ok(std::env::current_dir()
        .context("failed to resolve current working directory for steamcmd")?
        .join(path))
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SteamCmdScript {
    pub lines: Vec<String>,
}

impl SteamCmdScript {
    pub fn render(&self) -> String {
        let mut script = self.lines.join("\n");
        if !script.is_empty() {
            script.push('\n');
        }
        script
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::{
        SteamCmdConfig, SteamCmdContentProvider, SteamCmdScript, SteamLoginMode,
        absolute_force_install_dir,
    };
    use crate::steam::{
        CommandOutput, CommandRequest, CommandRunner, SteamAppId, WorkshopContentRequest,
        WorkshopContentProvider, WorkshopItemId, WorkshopItemRef,
    };

    #[test]
    fn steamcmd_content_dir_uses_app_and_item_ids() {
        let config = SteamCmdConfig {
            steamcmd_path: "/tmp/steamcmd.sh".into(),
            force_install_dir: "/tmp/yoita-steamcmd".into(),
            app_id: SteamAppId::NOITA,
            login: SteamLoginMode::Anonymous,
            timeout: std::time::Duration::from_secs(30),
        };

        let path = config.workshop_content_dir(WorkshopItemId(2194781427));
        assert_eq!(
            path,
            std::path::PathBuf::from(
                "/tmp/yoita-steamcmd/steamapps/workshop/content/881100/2194781427"
            )
        );
    }

    #[test]
    fn steamcmd_script_renders_with_trailing_newline() {
        let script = SteamCmdScript {
            lines: vec![
                "@ShutdownOnFailedCommand 1".to_owned(),
                "login anonymous".to_owned(),
                "quit".to_owned(),
            ],
        };

        assert_eq!(
            script.render(),
            "@ShutdownOnFailedCommand 1\nlogin anonymous\nquit\n"
        );
    }

    #[test]
    fn steamcmd_download_request_uses_anonymous_login() {
        let config = SteamCmdConfig {
            steamcmd_path: "/tmp/steamcmd.sh".into(),
            force_install_dir: "/tmp/yoita-steamcmd".into(),
            app_id: SteamAppId::NOITA,
            login: SteamLoginMode::Anonymous,
            timeout: std::time::Duration::from_secs(30),
        };
        let provider = SteamCmdContentProvider::new(config, Arc::new(FakeRunner::default()));

        let request = provider.download_request(WorkshopItemId(2572385079)).unwrap();

        assert_eq!(request.program, std::path::PathBuf::from("/tmp/steamcmd.sh"));
        assert_eq!(
            request.args,
            vec![
                "+force_install_dir",
                "/tmp/yoita-steamcmd",
                "+login",
                "anonymous",
                "+workshop_download_item",
                "881100",
                "2572385079",
                "+quit",
            ]
        );
    }

    #[test]
    fn relative_force_install_dir_is_resolved_against_current_dir() {
        let path = absolute_force_install_dir(std::path::Path::new("tests/.artifacts/steamcmd"))
            .unwrap();

        assert!(path.is_absolute());
        assert!(path.ends_with(std::path::Path::new("tests/.artifacts/steamcmd")));
    }

    #[tokio::test]
    async fn steamcmd_provider_uses_existing_download_without_running_command() {
        let unique = format!("yoita-steam-existing-{}", std::process::id());
        let root = std::env::temp_dir().join(unique);
        let item_dir = root
            .join("steamapps")
            .join("workshop")
            .join("content")
            .join("881100")
            .join("2572385079");
        std::fs::create_dir_all(&item_dir).unwrap();
        std::fs::write(item_dir.join("mod.xml"), b"<Mod />").unwrap();

        let config = SteamCmdConfig {
            steamcmd_path: "/tmp/steamcmd.sh".into(),
            force_install_dir: root.clone(),
            app_id: SteamAppId::NOITA,
            login: SteamLoginMode::Anonymous,
            timeout: std::time::Duration::from_secs(30),
        };
        let provider = SteamCmdContentProvider::new(config, Arc::new(FakeRunner::default()));

        let content = provider
            .ensure_content(WorkshopContentRequest {
                item: WorkshopItemRef::new(SteamAppId::NOITA, WorkshopItemId(2572385079)),
            })
            .await
            .unwrap();

        assert_eq!(content.source_path, item_dir);

        std::fs::remove_dir_all(root).unwrap();
    }

    #[derive(Debug, Default)]
    struct FakeRunner;

    impl CommandRunner for FakeRunner {
        fn run<'a>(
            &'a self,
            _request: CommandRequest,
        ) -> super::super::SteamFuture<'a, crate::error::Result<CommandOutput>> {
            Box::pin(async {
                Ok(CommandOutput {
                    exit_status: Some(0),
                    stdout: String::new(),
                    stderr: String::new(),
                })
            })
        }
    }
}
