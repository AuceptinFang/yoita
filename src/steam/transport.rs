use std::{
    collections::BTreeMap, fmt, future::Future, path::PathBuf, pin::Pin, process::Stdio,
    time::Duration,
};

use reqwest::Url;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};

use crate::error::Result;

pub type SteamFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpMethod {
    Get,
    Post,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// 一次通用 HTTP 请求。
///
/// 约定：
/// - URL path 直接体现在 `url` 里，不单独拆成“路径参数”字段
/// - query 参数放在 `query`
/// - 表单请求体放在 `form`
/// - 额外请求头放在 `headers`
pub struct HttpRequest {
    /// HTTP 方法，例如 `GET` / `POST`。
    pub method: HttpMethod,
    /// 完整 URL。
    ///
    /// 如果接口有路径参数，应当在构造请求前就把它们展开到这个 URL 里。
    pub url: Url,
    /// URL 查询参数。
    pub query: BTreeMap<String, String>,
    /// HTTP 请求头。
    pub headers: BTreeMap<String, String>,
    /// `application/x-www-form-urlencoded` 表单字段。
    pub form: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// HTTP 响应的最小抽象。
pub struct HttpResponse {
    /// HTTP 状态码。
    pub status: u16,
    /// 响应头。
    pub headers: BTreeMap<String, String>,
    /// 原始响应体字节。
    pub body: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// 一次外部命令执行请求。
///
/// 当前主要用于启动 `steamcmd`。
pub struct CommandRequest {
    /// 可执行文件路径。
    pub program: PathBuf,
    /// 命令行参数，按 token 切分。
    pub args: Vec<String>,
    /// 进程工作目录。
    ///
    /// 如果外部程序会在当前目录生成日志、缓存或其他状态文件，
    /// 就应该在这里显式指定。
    pub current_dir: Option<PathBuf>,
    /// 额外环境变量。
    pub env: BTreeMap<String, String>,
    /// 命令最长允许运行多久。
    pub timeout: Duration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// 一次外部命令执行结果。
pub struct CommandOutput {
    /// 退出码；如果进程被信号终止或平台拿不到退出码，则可能为 `None`。
    pub exit_status: Option<i32>,
    /// 标准输出。
    pub stdout: String,
    /// 标准错误输出。
    pub stderr: String,
}

pub trait HttpRequester: fmt::Debug + Send + Sync {
    /// 发送一个 HTTP 请求。
    fn send<'a>(&'a self, request: HttpRequest) -> SteamFuture<'a, Result<HttpResponse>>;
}

pub trait CommandRunner: fmt::Debug + Send + Sync {
    /// 启动一个外部命令并等待它结束。
    fn run<'a>(&'a self, request: CommandRequest) -> SteamFuture<'a, Result<CommandOutput>>;
}

#[derive(Debug, Clone)]
pub struct NativeHttpRequester {
    client: reqwest::Client,
}

impl Default for NativeHttpRequester {
    fn default() -> Self {
        let client = reqwest::Client::builder()
            .user_agent(concat!(
                env!("CARGO_PKG_NAME"),
                "/",
                env!("CARGO_PKG_VERSION")
            ))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        Self { client }
    }
}

impl HttpRequester for NativeHttpRequester {
    fn send<'a>(&'a self, request: HttpRequest) -> SteamFuture<'a, Result<HttpResponse>> {
        Box::pin(async move {
            let url = request.url.clone();
            let mut builder = match request.method {
                HttpMethod::Get => self.client.get(request.url),
                HttpMethod::Post => self.client.post(request.url),
            };

            if !request.query.is_empty() {
                builder = builder.query(&request.query);
            }

            if !request.headers.is_empty() {
                let mut headers = HeaderMap::new();
                for (name, value) in &request.headers {
                    let name = HeaderName::try_from(name.as_str()).map_err(|source| {
                        anyhow::anyhow!("invalid HTTP header name `{name}`: {source}")
                    })?;
                    let value = HeaderValue::try_from(value.as_str()).map_err(|source| {
                        anyhow::anyhow!("invalid HTTP header value for `{name}`: {source}")
                    })?;
                    headers.insert(name, value);
                }
                builder = builder.headers(headers);
            }

            if !request.form.is_empty() {
                builder = builder.form(&request.form);
            }

            let response = builder
                .send()
                .await
                .map_err(|source| anyhow::anyhow!("failed to request `{}`: {source}", url))?;
            let status = response.status().as_u16();
            let headers = response
                .headers()
                .iter()
                .map(|(name, value)| {
                    (
                        name.as_str().to_owned(),
                        value.to_str().unwrap_or_default().to_owned(),
                    )
                })
                .collect::<BTreeMap<_, _>>();
            let body = response.bytes().await.map_err(|source| {
                anyhow::anyhow!("failed to read response body from `{}`: {source}", url)
            })?;

            Ok(HttpResponse {
                status,
                headers,
                body: body.to_vec(),
            })
        })
    }
}

#[derive(Debug, Default)]
pub struct NativeCommandRunner;

impl CommandRunner for NativeCommandRunner {
    fn run<'a>(&'a self, request: CommandRequest) -> SteamFuture<'a, Result<CommandOutput>> {
        Box::pin(async move {
            let mut command = tokio::process::Command::new(&request.program);
            command
                .args(&request.args)
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .kill_on_drop(true);

            if let Some(current_dir) = &request.current_dir {
                command.current_dir(current_dir);
            }

            if !request.env.is_empty() {
                command.envs(&request.env);
            }

            let child = command.spawn().map_err(|source| {
                anyhow::anyhow!("failed to spawn `{}`: {source}", request.program.display())
            })?;
            let output = tokio::time::timeout(request.timeout, child.wait_with_output())
                .await
                .map_err(|_| {
                    anyhow::anyhow!(
                        "command `{}` timed out after {:?}",
                        request.program.display(),
                        request.timeout
                    )
                })?
                .map_err(|source| {
                    anyhow::anyhow!(
                        "failed to wait for `{}`: {source}",
                        request.program.display()
                    )
                })?;

            Ok(CommandOutput {
                exit_status: output.status.code(),
                stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            })
        })
    }
}
