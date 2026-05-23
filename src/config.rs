use std::path::PathBuf;

use reqwest::Url;
use serde::{Deserialize, Deserializer};

use crate::error::ConfigValidationError;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct YoitaConfig {
    pub config: RuntimeConfig,
    pub steam: Option<SteamConfig>,
    pub mods: Vec<ModConfig>, // TODO: 改成Option，允许为空
}

/// 需要用到的三个目录，存储下载文件的，用于挂载（复制）的，目标目录
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeConfig {
    #[serde(default = "default_cache_dir")]
    pub cache_dir: PathBuf,
    #[serde(default = "default_mount_dir")]
    pub mount_dir: PathBuf,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            cache_dir: default_cache_dir(),
            mount_dir: default_mount_dir(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SteamConfig {
    #[serde(default = "default_steamcmd_path")]
    pub steamcmd_path: PathBuf,
    #[serde(default)]
    pub working_dir: Option<PathBuf>,
    #[serde(default = "default_force_install_dir")]
    pub force_install_dir: PathBuf,
    #[serde(default = "default_app_id")]
    pub app_id: u32,
    #[serde(default = "default_timeout_secs")]
    pub timeout_secs: u64,
    #[serde(default)]
    pub login: SteamLoginConfig,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub password_env: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SteamLoginConfig {
    #[default]
    Anonymous,
    Account,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ModConfig {
    pub name: String,
    pub version: Option<String>,
    pub enabled: bool,
    pub source: ModSource,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ModSource {
    Steam {
        id: String,
    },
    Custom {
        #[serde(deserialize_with = "deserialize_url")]
        url: Url,
    },
}

fn default_cache_dir() -> PathBuf {
    PathBuf::from(".yoita/cache")
}

fn default_mount_dir() -> PathBuf {
    PathBuf::from("mods")
}

fn default_steamcmd_path() -> PathBuf {
    if cfg!(windows) {
        PathBuf::from("./steamcmd.exe")
    } else {
        PathBuf::from("steamcmd")
    }
}

fn default_force_install_dir() -> PathBuf {
    PathBuf::from(".yoita/steamcmd")
}

fn default_app_id() -> u32 {
    881100
}

fn default_timeout_secs() -> u64 {
    300
}

fn default_enabled() -> bool {
    true
}

fn deserialize_url<'de, D>(deserializer: D) -> Result<Url, D::Error>
where
    D: Deserializer<'de>,
{
    let raw = String::deserialize(deserializer)?;
    Url::parse(&raw).map_err(serde::de::Error::custom)
}

fn deserialize_option_url<'de, D>(deserializer: D) -> Result<Option<Url>, D::Error>
where
    D: Deserializer<'de>,
{
    Option::<String>::deserialize(deserializer)?
        .map(|raw| Url::parse(&raw).map_err(serde::de::Error::custom))
        .transpose()
}

#[derive(Debug, Clone, Default, Deserialize)]
pub(crate) struct RawYoitaConfig {
    #[serde(default)]
    pub config: RuntimeConfig,
    #[serde(default)]
    pub steam: Option<SteamConfig>,
    #[serde(default)]
    pub mods: RawMods,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(untagged)]
pub(crate) enum RawMods {
    #[default]
    Missing,
    List(Vec<RawModConfig>),
    Table(toml::Table),
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct RawModConfig {
    pub name: String,
    #[serde(flatten)]
    pub spec: RawModSpec,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub(crate) struct RawModSpec {
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default)]
    pub source: Option<ModSource>,
    #[serde(default, deserialize_with = "deserialize_option_url")]
    pub url: Option<Url>,
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default, rename = "workshop_id")]
    pub legacy_workshop_id: Option<String>,
    #[serde(default)]
    pub kind: Option<String>,
}

impl<'de> Deserialize<'de> for YoitaConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = RawYoitaConfig::deserialize(deserializer)?;
        raw.try_into().map_err(serde::de::Error::custom)
    }
}

impl TryFrom<RawYoitaConfig> for YoitaConfig {
    type Error = ConfigValidationError;

    fn try_from(raw: RawYoitaConfig) -> Result<Self, Self::Error> {
        let mods = match raw.mods {
            RawMods::Missing => Vec::new(),
            RawMods::List(entries) => entries
                .into_iter()
                .map(|entry| build_mod_config(entry.name, entry.spec))
                .collect::<Result<Vec<_>, _>>()?,
            RawMods::Table(entries) => entries
                .into_iter()
                .map(|(name, value)| build_compact_mod(name, value))
                .collect::<Result<Vec<_>, _>>()?,
        };

        if let Some(steam) = raw.steam.as_ref() {
            validate_steam_config(steam)?;
        }

        Ok(Self {
            config: raw.config,
            steam: raw.steam,
            mods,
        })
    }
}

fn build_compact_mod(name: String, value: toml::Value) -> Result<ModConfig, ConfigValidationError> {
    let field = format!("mods.{name}");

    match value {
        toml::Value::String(id) => build_id_shorthand_mod(name, id),
        toml::Value::Table(_) => {
            let spec: RawModSpec = value.try_into().map_err(|error: toml::de::Error| {
                ConfigValidationError::new(&field, error.message())
            })?;

            build_mod_config(name, spec)
        }
        other => Err(ConfigValidationError::new(
            field,
            format!(
                "must be a string or inline table, found {}",
                toml_value_kind(&other)
            ),
        )),
    }
}

fn build_id_shorthand_mod(name: String, id: String) -> Result<ModConfig, ConfigValidationError> {
    build_mod_config(
        name,
        RawModSpec {
            version: None,
            enabled: default_enabled(),
            source: None,
            url: None,
            id: Some(id),
            legacy_workshop_id: None,
            kind: None,
        },
    )
}

fn build_mod_config(name: String, spec: RawModSpec) -> Result<ModConfig, ConfigValidationError> {
    let field = format!("mods.{name}");
    if spec.legacy_workshop_id.is_some() {
        return Err(ConfigValidationError::new(
            &field,
            "uses removed field `workshop_id`; write `id` instead",
        ));
    }

    let source = resolve_source(&field, &name, spec.source, spec.kind, spec.url, spec.id)?;

    Ok(ModConfig {
        name,
        version: spec.version,
        enabled: spec.enabled,
        source,
    })
}

fn resolve_source(
    field: &str,
    default_id: &str,
    source: Option<ModSource>,
    kind: Option<String>,
    url: Option<Url>,
    id: Option<String>,
) -> Result<ModSource, ConfigValidationError> {
    if source.is_some() && (kind.is_some() || url.is_some() || id.is_some()) {
        return Err(ConfigValidationError::new(
            field,
            "cannot mix nested `source` with `kind`, `url`, or `id`",
        ));
    }

    if let Some(source) = source {
        return normalize_source(field, default_id, source);
    }

    let id = normalize_id(field, id)?;

    match kind.as_deref() {
        Some("custom") => match (url, id) {
            (Some(url), None) => Ok(ModSource::Custom { url }),
            (None, _) => Err(ConfigValidationError::new(
                field,
                "with `kind = \"custom\"` must define `url`",
            )),
            (Some(_), Some(_)) => Err(ConfigValidationError::new(
                field,
                "with `kind = \"custom\"` cannot also define `id`",
            )),
        },
        Some("steam") => match (url, id) {
            (None, Some(id)) => Ok(ModSource::Steam { id }),
            (None, None) => Ok(ModSource::Steam {
                id: default_id.to_owned(),
            }),
            (Some(_), _) => Err(ConfigValidationError::new(
                field,
                "with `kind = \"steam\"` cannot also define `url`",
            )),
        },
        Some(other) => Err(ConfigValidationError::new(
            field,
            format!("uses unsupported `kind = \"{other}\"`"),
        )),
        None => match (url, id) {
            (Some(url), None) => Ok(ModSource::Custom { url }),
            (None, Some(id)) => Ok(ModSource::Steam { id }),
            (Some(_), Some(_)) => Err(ConfigValidationError::new(
                field,
                "must not define both `url` and `id` without an explicit `kind`",
            )),
            (None, None) => Ok(ModSource::Steam {
                id: default_id.to_owned(),
            }),
        },
    }
}

fn normalize_source(
    field: &str,
    default_id: &str,
    source: ModSource,
) -> Result<ModSource, ConfigValidationError> {
    match source {
        ModSource::Steam { id } => Ok(ModSource::Steam {
            id: normalize_id(field, Some(id))?.unwrap_or_else(|| default_id.to_owned()),
        }),
        ModSource::Custom { url } => Ok(ModSource::Custom { url }),
    }
}

fn normalize_id(field: &str, id: Option<String>) -> Result<Option<String>, ConfigValidationError> {
    match id {
        Some(id) => {
            let id = id.trim().to_owned();
            if id.is_empty() {
                Err(ConfigValidationError::new(
                    field,
                    "must define a non-empty `id`",
                ))
            } else {
                Ok(Some(id))
            }
        }
        None => Ok(None),
    }
}

fn validate_steam_config(config: &SteamConfig) -> Result<(), ConfigValidationError> {
    if config.app_id == 0 {
        return Err(ConfigValidationError::new(
            "steam.app_id",
            "must be greater than zero",
        ));
    }

    if config.timeout_secs == 0 {
        return Err(ConfigValidationError::new(
            "steam.timeout_secs",
            "must be greater than zero",
        ));
    }

    match config.login {
        SteamLoginConfig::Anonymous => {
            if config.username.is_some() {
                return Err(ConfigValidationError::new(
                    "steam.username",
                    "must not be set when `login = \"anonymous\"`",
                ));
            }

            if config.password_env.is_some() {
                return Err(ConfigValidationError::new(
                    "steam.password_env",
                    "must not be set when `login = \"anonymous\"`",
                ));
            }
        }
        SteamLoginConfig::Account => {
            let username = config
                .username
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty());
            let password_env = config
                .password_env
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty());

            if username.is_none() {
                return Err(ConfigValidationError::new(
                    "steam.username",
                    "is required when `login = \"account\"`",
                ));
            }

            if password_env.is_none() {
                return Err(ConfigValidationError::new(
                    "steam.password_env",
                    "is required when `login = \"account\"`",
                ));
            }
        }
    }

    Ok(())
}

fn toml_value_kind(value: &toml::Value) -> &'static str {
    match value {
        toml::Value::String(_) => "string",
        toml::Value::Integer(_) => "integer",
        toml::Value::Float(_) => "float",
        toml::Value::Boolean(_) => "boolean",
        toml::Value::Datetime(_) => "datetime",
        toml::Value::Array(_) => "array",
        toml::Value::Table(_) => "table",
    }
}
