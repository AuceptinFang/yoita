use std::{fmt, path::PathBuf, str::FromStr};

use reqwest::Url;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
/// Steam 应用 id。
///
/// 例如 Noita 的 app id 是 `881100`。
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
/// Steam Workshop 条目 id。
///
/// 例如 `2572385079`。
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
/// 唯一标识一个 Workshop 条目。
///
/// `app_id` 和 `workshop_id` 合在一起，就是后续请求内容或元数据时的主键。
pub struct WorkshopItemRef {
    /// Steam 应用 id。
    pub app_id: SteamAppId,
    /// Workshop 条目 id。
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
/// 按已知 Workshop 条目 id 获取元数据的请求。
///
/// 这里没有单独拆分“路径参数”字段：
/// `app_id` / `workshop_id` 都包含在 `item` 里，具体映射到 URL path、query
/// 还是其他传输层参数，由具体 provider 决定。
pub struct WorkshopMetadataRequest {
    /// 要查询的 Workshop 条目标识。
    pub item: WorkshopItemRef,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// 按名称或关键字搜索 Workshop 条目的请求。
///
/// 在当前的 `SteamCommunityMetadataProvider` 实现里：
/// - 没有路径参数
/// - `app_id` 会映射到 query 参数 `appid`
/// - `query` 会映射到 query 参数 `searchtext`
/// - `limit` 只是本地截断上限，不会直接发给 Steam
pub struct WorkshopSearchRequest {
    /// 目标游戏的 Steam app id，对应 query 参数 `appid`。
    pub app_id: SteamAppId,
    /// 搜索关键字，对应 query 参数 `searchtext`。
    pub query: String,
    /// 最多保留多少条搜索结果。
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
/// 确保某个 Workshop 条目的本地内容已经可用的请求。
///
/// 常见实现会根据 `item.app_id` 和 `item.workshop_id`
/// 去拼接下载命令或本地缓存目录。
pub struct WorkshopContentRequest {
    /// 要下载或复用本地缓存的 Workshop 条目标识。
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
