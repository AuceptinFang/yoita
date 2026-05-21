use std::{fmt, path::PathBuf, str::FromStr};

use reqwest::Url;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SteamAppId(pub u32);

impl SteamAppId {
    pub const NOITA: Self = Self(881100);
}

impl Default for SteamAppId {
    fn default() -> Self {
        Self::NOITA
    }
}

impl fmt::Display for SteamAppId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct WorkshopItemId(pub u64);

impl fmt::Display for WorkshopItemId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for WorkshopItemId {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> std::result::Result<Self, Self::Err> {
        let value = value.trim();
        if value.is_empty() {
            return Err(anyhow::anyhow!("steam workshop id cannot be empty"));
        }

        Ok(Self(value.parse::<u64>().map_err(|_| {
            anyhow::anyhow!("invalid steam workshop id `{value}`")
        })?))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WorkshopItemRef {
    pub app_id: SteamAppId,
    pub workshop_id: WorkshopItemId,
}

impl WorkshopItemRef {
    pub fn new(app_id: SteamAppId, workshop_id: WorkshopItemId) -> Self {
        Self {
            app_id,
            workshop_id,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkshopFileType {
    Community,
    Microtransaction,
    Collection,
    Art,
    Video,
    Screenshot,
    Game,
    Software,
    Concept,
    WebGuide,
    IntegratedGuide,
    Merch,
    ControllerBinding,
    ReadyToUse,
    WorkshopShowcase,
    GameManaged,
    Unknown(u32),
}

impl From<u32> for WorkshopFileType {
    fn from(value: u32) -> Self {
        match value {
            0 => Self::Community,
            1 => Self::Microtransaction,
            2 => Self::Collection,
            3 => Self::Art,
            4 => Self::Video,
            5 => Self::Screenshot,
            6 => Self::Game,
            7 => Self::Software,
            8 => Self::Concept,
            11 => Self::WebGuide,
            12 => Self::IntegratedGuide,
            14 => Self::Merch,
            15 => Self::ControllerBinding,
            16 => Self::ReadyToUse,
            17 => Self::WorkshopShowcase,
            18 => Self::GameManaged,
            other => Self::Unknown(other),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkshopMetadataRequest {
    pub item: WorkshopItemRef,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkshopSearchRequest {
    pub app_id: SteamAppId,
    pub query: String,
    pub limit: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkshopItemDetails {
    pub item: WorkshopItemRef,
    pub title: Option<String>,
    pub description: Option<String>,
    pub preview_url: Option<Url>,
    pub file_type: Option<WorkshopFileType>,
    pub time_created: Option<u32>,
    pub time_updated: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkshopContentRequest {
    pub item: WorkshopItemRef,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkshopContentKind {
    Directory,
    SingleFile,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkshopItemContent {
    pub item: WorkshopItemRef,
    pub source_path: PathBuf,
    pub kind: WorkshopContentKind,
}

#[cfg(test)]
mod tests {
    use super::WorkshopItemId;

    #[test]
    fn parses_numeric_workshop_id() {
        let item = "3454128340".parse::<WorkshopItemId>().unwrap();
        assert_eq!(item, WorkshopItemId(3454128340));
    }
}
