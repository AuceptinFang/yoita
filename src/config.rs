use std::path::PathBuf;

use reqwest::Url;
use serde::{Deserialize, Deserializer};

use crate::error::ConfigValidationError;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct YoitaConfig {
    pub config: RuntimeConfig,
    pub steam: Option<SteamConfig>,
    pub mods: Vec<ModConfig>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct RuntimeConfig {
    #[serde(default = "default_cache_dir")]
    pub cache_dir: PathBuf,
    #[serde(default = "default_staging_dir")]
    pub staging_dir: PathBuf,
    #[serde(default = "default_mount_dir")]
    pub mount_dir: PathBuf,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            cache_dir: default_cache_dir(),
            staging_dir: default_staging_dir(),
            mount_dir: default_mount_dir(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct SteamConfig {
    #[serde(deserialize_with = "deserialize_url")]
    pub download_endpoint: Url,
    #[serde(default)]
    pub api_key: Option<String>,
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
        workshop_id: String,
    },
    Custom {
        #[serde(deserialize_with = "deserialize_url")]
        url: Url,
    },
}

fn default_cache_dir() -> PathBuf {
    PathBuf::from(".yoita/cache")
}

fn default_staging_dir() -> PathBuf {
    PathBuf::from(".yoita/staging")
}

fn default_mount_dir() -> PathBuf {
    PathBuf::from("mods")
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
    pub workshop_id: Option<String>,
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
        toml::Value::String(raw_source) => build_shorthand_mod(name, raw_source),
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

fn build_shorthand_mod(
    name: String,
    raw_source: String,
) -> Result<ModConfig, ConfigValidationError> {
    let field = format!("mods.{name}");
    let raw_source = raw_source.trim();

    if raw_source.is_empty() {
        return Err(ConfigValidationError::new(
            field,
            "cannot use an empty shorthand source",
        ));
    }

    let source = match Url::parse(raw_source) {
        Ok(url) => ModSource::Custom { url },
        Err(error) if looks_like_url(raw_source) => {
            return Err(ConfigValidationError::new(
                field,
                format!("contains an invalid url: {error}"),
            ));
        }
        Err(_) => ModSource::Steam {
            workshop_id: raw_source.to_owned(),
        },
    };

    build_mod_config(
        name,
        RawModSpec {
            version: None,
            enabled: default_enabled(),
            source: Some(source),
            url: None,
            workshop_id: None,
            kind: None,
        },
    )
}

fn build_mod_config(name: String, spec: RawModSpec) -> Result<ModConfig, ConfigValidationError> {
    let field = format!("mods.{name}");
    let source = resolve_source(&field, spec.source, spec.kind, spec.url, spec.workshop_id)?;

    Ok(ModConfig {
        name,
        version: spec.version,
        enabled: spec.enabled,
        source,
    })
}

fn resolve_source(
    field: &str,
    source: Option<ModSource>,
    kind: Option<String>,
    url: Option<Url>,
    workshop_id: Option<String>,
) -> Result<ModSource, ConfigValidationError> {
    if source.is_some() && (kind.is_some() || url.is_some() || workshop_id.is_some()) {
        return Err(ConfigValidationError::new(
            field,
            "cannot mix nested `source` with `kind`, `url`, or `workshop_id`",
        ));
    }

    if let Some(source) = source {
        return normalize_source(field, source);
    }

    let workshop_id = normalize_workshop_id(field, workshop_id)?;

    match kind.as_deref() {
        Some("custom") => match (url, workshop_id) {
            (Some(url), None) => Ok(ModSource::Custom { url }),
            (None, _) => Err(ConfigValidationError::new(
                field,
                "with `kind = \"custom\"` must define `url`",
            )),
            (Some(_), Some(_)) => Err(ConfigValidationError::new(
                field,
                "with `kind = \"custom\"` cannot also define `workshop_id`",
            )),
        },
        Some("steam") => match (url, workshop_id) {
            (None, Some(workshop_id)) => Ok(ModSource::Steam { workshop_id }),
            (None, None) => Err(ConfigValidationError::new(
                field,
                "with `kind = \"steam\"` must define `workshop_id`",
            )),
            (Some(_), _) => Err(ConfigValidationError::new(
                field,
                "with `kind = \"steam\"` cannot also define `url`",
            )),
        },
        Some(other) => Err(ConfigValidationError::new(
            field,
            format!("uses unsupported `kind = \"{other}\"`"),
        )),
        None => match (url, workshop_id) {
            (Some(url), None) => Ok(ModSource::Custom { url }),
            (None, Some(workshop_id)) => Ok(ModSource::Steam { workshop_id }),
            (Some(_), Some(_)) => Err(ConfigValidationError::new(
                field,
                "must not define both `url` and `workshop_id` without an explicit `kind`",
            )),
            (None, None) => Err(ConfigValidationError::new(
                field,
                "must define either `url` or `workshop_id`",
            )),
        },
    }
}

fn normalize_source(field: &str, source: ModSource) -> Result<ModSource, ConfigValidationError> {
    match source {
        ModSource::Steam { workshop_id } => Ok(ModSource::Steam {
            workshop_id: normalize_workshop_id(field, Some(workshop_id))?.ok_or_else(|| {
                ConfigValidationError::new(field, "must define a non-empty `workshop_id`")
            })?,
        }),
        ModSource::Custom { url } => Ok(ModSource::Custom { url }),
    }
}

fn normalize_workshop_id(
    field: &str,
    workshop_id: Option<String>,
) -> Result<Option<String>, ConfigValidationError> {
    match workshop_id {
        Some(workshop_id) => {
            let workshop_id = workshop_id.trim().to_owned();
            if workshop_id.is_empty() {
                Err(ConfigValidationError::new(
                    field,
                    "must define a non-empty `workshop_id`",
                ))
            } else {
                Ok(Some(workshop_id))
            }
        }
        None => Ok(None),
    }
}

fn looks_like_url(value: &str) -> bool {
    value.contains("://")
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
