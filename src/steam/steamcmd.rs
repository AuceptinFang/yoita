use std::{path::PathBuf, time::Duration};

use crate::config::{SteamBackend, SteamConfig, SteamLoginConfig};

use super::WorkshopItemId;

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
            force_install_dir: config.force_install_dir.clone(),
            app_id: super::SteamAppId(config.app_id),
            login,
            timeout: Duration::from_secs(config.timeout_secs),
        })
    }
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
    use super::{SteamCmdConfig, SteamCmdScript, SteamLoginMode};
    use crate::steam::{SteamAppId, WorkshopItemId};

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
}
