use std::sync::Arc;

use anyhow::Context;
use reqwest::Url;

use crate::error::Result;

use super::{
    HttpMethod, HttpRequest, HttpRequester, SteamFuture, SteamAppId, WorkshopItemDetails,
    WorkshopItemId, WorkshopItemRef, WorkshopMetadataProvider, WorkshopMetadataRequest,
    WorkshopSearchRequest,
};

const STEAM_WORKSHOP_BROWSE_URL: &str = "https://steamcommunity.com/workshop/browse/";

#[derive(Debug, Clone)]
pub struct SteamCommunityMetadataProvider {
    requester: Arc<dyn HttpRequester>,
}

impl SteamCommunityMetadataProvider {
    pub fn new(requester: Arc<dyn HttpRequester>) -> Self {
        Self { requester }
    }

    fn build_search_request(&self, request: &WorkshopSearchRequest) -> Result<HttpRequest> {
        let url = Url::parse(STEAM_WORKSHOP_BROWSE_URL)
            .context("failed to parse steam workshop browse URL")?;
        let mut query = std::collections::BTreeMap::new();
        query.insert("appid".to_owned(), request.app_id.to_string());
        query.insert("searchtext".to_owned(), request.query.clone());
        query.insert("childpublishedfileid".to_owned(), "0".to_owned());
        query.insert("browsesort".to_owned(), "textsearch".to_owned());
        query.insert("section".to_owned(), "home".to_owned());

        Ok(HttpRequest {
            method: HttpMethod::Get,
            url,
            query,
            headers: std::collections::BTreeMap::new(),
            form: std::collections::BTreeMap::new(),
        })
    }

    fn parse_search_results(
        app_id: SteamAppId,
        body: &str,
        limit: usize,
    ) -> Result<Vec<WorkshopItemDetails>> {
        let mut results = Vec::new();
        let mut cursor = 0usize;

        while let Some(found) = body[cursor..].find("filedetails/?id=") {
            let start = cursor + found + "filedetails/?id=".len();
            let digits_len = body[start..]
                .chars()
                .take_while(|ch| ch.is_ascii_digit())
                .count();
            if digits_len == 0 {
                cursor = start;
                continue;
            }

            let id_text = &body[start..start + digits_len];
            let item_id = id_text.parse::<WorkshopItemId>()?;
            let item = WorkshopItemRef::new(app_id, item_id);
            if !results
                .iter()
                .any(|existing: &WorkshopItemDetails| existing.item == item)
            {
                results.push(WorkshopItemDetails {
                    item,
                    title: None,
                    description: None,
                    preview_url: None,
                    file_type: None,
                    time_created: None,
                    time_updated: None,
                });
                if results.len() >= limit {
                    break;
                }
            }

            cursor = start + digits_len;
        }

        Ok(results)
    }
}

impl WorkshopMetadataProvider for SteamCommunityMetadataProvider {
    fn fetch_item<'a>(
        &'a self,
        _request: WorkshopMetadataRequest,
    ) -> SteamFuture<'a, Result<Option<WorkshopItemDetails>>> {
        Box::pin(async {
            Err(anyhow::anyhow!(
                "steam community metadata fetch by workshop id is not implemented"
            )
            .into())
        })
    }

    fn search_items<'a>(
        &'a self,
        request: WorkshopSearchRequest,
    ) -> SteamFuture<'a, Result<Vec<WorkshopItemDetails>>> {
        Box::pin(async move {
            let request_query = request.query.trim().to_owned();
            if request_query.is_empty() {
                return Err(anyhow::anyhow!("steam workshop search query cannot be empty").into());
            }

            let limit = request.limit.max(1);
            let response = self
                .requester
                .send(self.build_search_request(&request)?)
                .await
                .map_err(|source| {
                    anyhow::anyhow!(
                        "failed to search steam workshop for `{request_query}`: {source:#}"
                    )
                })?;

            if response.status != 200 {
                return Err(anyhow::anyhow!(
                    "steam workshop search for `{request_query}` returned HTTP {}",
                    response.status
                )
                .into());
            }

            let body = String::from_utf8_lossy(&response.body);
            let results = Self::parse_search_results(request.app_id, &body, limit).map_err(
                |source| {
                    anyhow::anyhow!(
                        "failed to parse steam workshop search results for `{request_query}`: {source:#}"
                    )
                },
            )?;

            Ok(results)
        })
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeMap,
        sync::{Arc, Mutex},
    };

    use super::SteamCommunityMetadataProvider;
    use crate::steam::{
        HttpRequest, HttpRequester, SteamAppId, WorkshopItemId, WorkshopMetadataProvider,
        WorkshopSearchRequest,
    };

    #[tokio::test]
    async fn search_items_builds_expected_query() {
        let requester = Arc::new(FakeHttpRequester::new(b"<html></html>".to_vec()));
        let provider = SteamCommunityMetadataProvider::new(requester.clone());

        let results = provider
            .search_items(WorkshopSearchRequest {
                app_id: SteamAppId::NOITA,
                query: "wanddbg".to_owned(),
                limit: 3,
            })
            .await
            .unwrap();

        assert!(results.is_empty());

        let request = requester.last_request.lock().unwrap().clone().unwrap();
        assert_eq!(request.url.as_str(), "https://steamcommunity.com/workshop/browse/");
        assert_eq!(request.query.get("appid"), Some(&"881100".to_owned()));
        assert_eq!(request.query.get("searchtext"), Some(&"wanddbg".to_owned()));
        assert_eq!(
            request.query.get("browsesort"),
            Some(&"textsearch".to_owned())
        );
    }

    #[tokio::test]
    async fn search_items_extracts_unique_workshop_ids() {
        let body = br#"
            <a href="https://steamcommunity.com/sharedfiles/filedetails/?id=2572385079">first</a>
            <a href="https://steamcommunity.com/sharedfiles/filedetails/?id=2572385079">duplicate</a>
            <a href="https://steamcommunity.com/sharedfiles/filedetails/?id=2194781427">second</a>
        "#;
        let provider =
            SteamCommunityMetadataProvider::new(Arc::new(FakeHttpRequester::new(body.to_vec())));

        let results = provider
            .search_items(WorkshopSearchRequest {
                app_id: SteamAppId::NOITA,
                query: "wanddbg".to_owned(),
                limit: 5,
            })
            .await
            .unwrap();

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].item.workshop_id, WorkshopItemId(2572385079));
        assert_eq!(results[1].item.workshop_id, WorkshopItemId(2194781427));
    }

    #[derive(Debug)]
    struct FakeHttpRequester {
        body: Vec<u8>,
        last_request: Mutex<Option<HttpRequest>>,
    }

    impl FakeHttpRequester {
        fn new(body: Vec<u8>) -> Self {
            Self {
                body,
                last_request: Mutex::new(None),
            }
        }
    }

    impl HttpRequester for FakeHttpRequester {
        fn send<'a>(
            &'a self,
            request: HttpRequest,
        ) -> super::super::SteamFuture<'a, crate::error::Result<crate::steam::HttpResponse>>
        {
            Box::pin(async move {
                *self.last_request.lock().unwrap() = Some(request);
                Ok(crate::steam::HttpResponse {
                    status: 200,
                    headers: BTreeMap::new(),
                    body: self.body.clone(),
                })
            })
        }
    }
}
