use reqwest::{Client, Url};

use crate::{config::SteamConfig, error::Result};

#[derive(Debug, Clone)]
pub struct SteamClient {
    pub http: Client,
    download_endpoint: Url,
    api_key: Option<String>,
}

impl SteamClient {
    pub fn new(http: Client, config: &SteamConfig) -> Result<Self> {
        Ok(Self {
            http,
            download_endpoint: config.download_endpoint.clone(),
            api_key: config.api_key.clone(),
        })
    }

    pub fn download_url(&self, workshop_id: &str) -> Result<Url> {
        if workshop_id.trim().is_empty() {
            return Err(anyhow::anyhow!("steam workshop id cannot be empty").into());
        }

        let mut download_url = self.download_endpoint.clone();
        {
            let mut query = download_url.query_pairs_mut();
            query.append_pair("id", workshop_id);

            if let Some(api_key) = &self.api_key {
                query.append_pair("key", api_key);
            }
        }

        Ok(download_url)
    }
}
