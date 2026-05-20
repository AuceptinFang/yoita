use std::{
    collections::BTreeMap,
    fmt,
    future::Future,
    path::PathBuf,
    pin::Pin,
    time::Duration,
};

use reqwest::Url;

use crate::error::Result;

pub type SteamFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpMethod {
    Get,
    Post,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpRequest {
    pub method: HttpMethod,
    pub url: Url,
    pub query: BTreeMap<String, String>,
    pub headers: BTreeMap<String, String>,
    pub form: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpResponse {
    pub status: u16,
    pub headers: BTreeMap<String, String>,
    pub body: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandRequest {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub current_dir: Option<PathBuf>,
    pub env: BTreeMap<String, String>,
    pub timeout: Duration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandOutput {
    pub exit_status: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

pub trait HttpRequester: fmt::Debug + Send + Sync {
    fn send<'a>(&'a self, request: HttpRequest) -> SteamFuture<'a, Result<HttpResponse>>;
}

pub trait CommandRunner: fmt::Debug + Send + Sync {
    fn run<'a>(&'a self, request: CommandRequest) -> SteamFuture<'a, Result<CommandOutput>>;
}
