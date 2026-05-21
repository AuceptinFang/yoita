use std::sync::Arc;

use super::{
    NativeCommandRunner, SteamAppId, SteamCmdConfig, UnsupportedWorkshopContentProvider,
    UnsupportedWorkshopMetadataProvider, WorkshopContentProvider, WorkshopMetadataProvider,
};
use super::steamcmd::SteamCmdContentProvider;

#[derive(Debug, Clone)]
pub struct SteamServices {
    metadata: Arc<dyn WorkshopMetadataProvider>,
    content: Arc<dyn WorkshopContentProvider>,
}

impl SteamServices {
    pub fn new(
        metadata: Arc<dyn WorkshopMetadataProvider>,
        content: Arc<dyn WorkshopContentProvider>,
    ) -> Self {
        Self { metadata, content }
    }

    pub fn unsupported() -> Self {
        Self::new(
            Arc::new(UnsupportedWorkshopMetadataProvider),
            Arc::new(UnsupportedWorkshopContentProvider),
        )
    }

    pub fn steamcmd(config: SteamCmdConfig) -> Self {
        Self::new(
            Arc::new(UnsupportedWorkshopMetadataProvider),
            Arc::new(SteamCmdContentProvider::new(
                config,
                Arc::new(NativeCommandRunner),
            )),
        )
    }

    pub fn metadata(&self) -> &dyn WorkshopMetadataProvider {
        self.metadata.as_ref()
    }

    pub fn content(&self) -> &dyn WorkshopContentProvider {
        self.content.as_ref()
    }
}

#[derive(Debug, Clone)]
pub struct SteamContext {
    config: SteamCmdConfig,
    services: SteamServices,
}

impl SteamContext {
    pub fn new(config: SteamCmdConfig, services: SteamServices) -> Self {
        Self { config, services }
    }

    pub fn unsupported(config: SteamCmdConfig) -> Self {
        Self::new(config, SteamServices::unsupported())
    }

    pub fn steamcmd(config: SteamCmdConfig) -> Self {
        Self::new(config.clone(), SteamServices::steamcmd(config))
    }

    pub fn app_id(&self) -> SteamAppId {
        self.config.app_id
    }

    pub fn config(&self) -> &SteamCmdConfig {
        &self.config
    }

    pub fn metadata(&self) -> &dyn WorkshopMetadataProvider {
        self.services.metadata()
    }

    pub fn content(&self) -> &dyn WorkshopContentProvider {
        self.services.content()
    }
}
