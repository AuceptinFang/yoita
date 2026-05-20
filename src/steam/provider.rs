use std::fmt;

use crate::error::Result;

use super::{
    SteamFuture, WorkshopContentRequest, WorkshopItemContent, WorkshopItemDetails,
    WorkshopMetadataRequest,
};

pub trait WorkshopMetadataProvider: fmt::Debug + Send + Sync {
    fn fetch_item<'a>(
        &'a self,
        request: WorkshopMetadataRequest,
    ) -> SteamFuture<'a, Result<Option<WorkshopItemDetails>>>;
}

pub trait WorkshopContentProvider: fmt::Debug + Send + Sync {
    fn ensure_content<'a>(
        &'a self,
        request: WorkshopContentRequest,
    ) -> SteamFuture<'a, Result<WorkshopItemContent>>;
}

#[derive(Debug, Default)]
pub struct UnsupportedWorkshopMetadataProvider;

impl WorkshopMetadataProvider for UnsupportedWorkshopMetadataProvider {
    fn fetch_item<'a>(
        &'a self,
        _request: WorkshopMetadataRequest,
    ) -> SteamFuture<'a, Result<Option<WorkshopItemDetails>>> {
        Box::pin(async { Err(anyhow::anyhow!("steam metadata provider is not implemented").into()) })
    }
}

#[derive(Debug, Default)]
pub struct UnsupportedWorkshopContentProvider;

impl WorkshopContentProvider for UnsupportedWorkshopContentProvider {
    fn ensure_content<'a>(
        &'a self,
        _request: WorkshopContentRequest,
    ) -> SteamFuture<'a, Result<WorkshopItemContent>> {
        Box::pin(async { Err(anyhow::anyhow!("steam content provider is not implemented").into()) })
    }
}
