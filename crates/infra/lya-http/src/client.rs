//! 共享连接池上的 HTTP 客户端。
//!
//! [`HttpClient`] 是本 crate 的主入口。内部持有一个 `reqwest::Client`：
//! - `reqwest::Client` 本身已是 `Arc` 包装，`Clone` 成本极低
//! - 所有 clone 出去的句柄共享同一连接池、同一 TLS 会话缓存
//!
//! 典型用法：在进程启动时 `HttpClient::new` 一次，再 `clone` 分发给
//! `lya-llm`、网页抓取、图片下载等模块。

use std::sync::Arc;

use bytes::Bytes;
use futures_core::Stream;
use reqwest::{
    Client, Method, RequestBuilder, Response, StatusCode,
    header::{HeaderMap, HeaderName, HeaderValue},
};
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::config::HttpConfig;
use crate::error::HttpError;
use crate::stream::{ByteStream, map_stream_error};

/// 一次成功响应的包装。
///
/// 保留 status / headers，并把 body 消费方式交给调用方选择
///（整包 bytes、JSON、或字节流）。
#[derive(Debug)]
pub struct HttpResponse {
    /// 底层 reqwest 响应。
    ///
    /// 对外仍暴露，便于少数需要直接操作 header / 扩展的场景；
    /// 常规路径请优先用本结构体提供的辅助方法。
    inner: Response,
}

impl HttpResponse {
    /// 从 reqwest 响应构造。
    fn new(inner: Response) -> Self {
        Self { inner }
    }

    /// HTTP 状态码。
    pub fn status(&self) -> StatusCode {
        self.inner.status()
    }

    /// 响应头只读视图。
    pub fn headers(&self) -> &HeaderMap {
        self.inner.headers()
    }

    /// 最终落地的地址。
    ///
    /// 重定向是自动跟随的，所以这未必是当初请求的那个——调用方若对目标有安全
    /// 要求（比如不许访问内网），必须拿这个再校验一次，光验请求前的 URL 会被
    /// 一次 302 绕过去。
    pub fn url(&self) -> &str {
        self.inner.url().as_str()
    }

    /// 是否为 2xx。
    pub fn is_success(&self) -> bool {
        self.inner.status().is_success()
    }

    /// 若非 2xx，读取 body 文本并转为 [`HttpError::Status`]。
    ///
    /// body 最长保留约 4KiB，避免错误路径把巨大 HTML 错误页塞进日志。
    pub async fn error_for_status(self) -> Result<Self, HttpError> {
        let status = self.inner.status();
        if status.is_success() {
            return Ok(self);
        }
        let body = self.inner.text().await.unwrap_or_default();
        let body = truncate_for_error(&body);
        Err(HttpError::Status { status, body })
    }

    /// 读取完整 body 为 [`Bytes`]。
    ///
    /// 适合小响应；大响应请用 [`HttpResponse::bytes_stream`]。
    pub async fn bytes(self) -> Result<Bytes, HttpError> {
        self.inner.bytes().await.map_err(HttpError::from_reqwest)
    }

    /// 读取完整 body 为 UTF-8 文本。
    pub async fn text(self) -> Result<String, HttpError> {
        self.inner.text().await.map_err(HttpError::from_reqwest)
    }

    /// 将 body 反序列化为 JSON。
    pub async fn json<T: DeserializeOwned>(self) -> Result<T, HttpError> {
        self.inner.json::<T>().await.map_err(|err| {
            // reqwest 的 json 失败可能是网络也可能是 serde；统一标成 Decode
            // 更贴近上层「解析失败」语义。若需细分可再看 err.is_decode()。
            if err.is_decode() {
                HttpError::Decode(err.to_string())
            } else {
                HttpError::from_reqwest(err)
            }
        })
    }

    /// 将 body 转为字节流。
    ///
    /// 每个 chunk 是一块 [`Bytes`]；流结束即 body 读完。
    pub fn bytes_stream(self) -> ByteStream {
        let stream = self.inner.bytes_stream();
        Box::pin(MapErrStream { inner: stream })
    }

    /// 取出内部 `reqwest::Response`（进阶用途）。
    pub fn into_inner(self) -> Response {
        self.inner
    }
}

/// 把 `Stream<Item = Result<Bytes, reqwest::Error>>` 映射为
/// `Stream<Item = Result<Bytes, HttpError>>` 的适配器。
struct MapErrStream<S> {
    /// 底层 reqwest bytes stream。
    inner: S,
}

impl<S> Stream for MapErrStream<S>
where
    S: Stream<Item = Result<Bytes, reqwest::Error>> + Unpin,
{
    type Item = Result<Bytes, HttpError>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        use std::task::Poll;
        match std::pin::Pin::new(&mut self.inner).poll_next(cx) {
            Poll::Ready(Some(Ok(chunk))) => Poll::Ready(Some(Ok(chunk))),
            Poll::Ready(Some(Err(err))) => Poll::Ready(Some(Err(map_stream_error(err)))),
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
}

/// 共享连接池的 HTTP 客户端。
///
/// # Clone 语义
///
/// `Clone` 只增加 `Arc` 计数，所有副本共用同一 `reqwest::Client` 与连接池。
/// 因此可以在多个异步任务间随意分发，无需担心「每人一把 Client 建一堆连接」。
///
/// # 线程安全
///
/// `HttpClient` 是 `Send + Sync` 的，可放进 `Arc` 再包一层，或直接 clone。
#[derive(Clone, Debug)]
pub struct HttpClient {
    /// 底层 reqwest 客户端（内部已 Arc）。
    ///
    /// 再包一层 `Arc` 是为了让本结构体的其它未来字段（统计、钩子等）
    /// 也能随 clone 共享；当前主要持有 `Client`。
    inner: Arc<Client>,
}

impl HttpClient {
    /// 按配置构造客户端。
    ///
    /// 失败通常来自 TLS 后端或非法 builder 参数，对应 [`HttpError::Build`]。
    pub fn new(config: &HttpConfig) -> Result<Self, HttpError> {
        let mut builder = Client::builder()
            .user_agent(&config.user_agent)
            .pool_max_idle_per_host(config.pool_max_idle_per_host)
            .tcp_nodelay(true);

        if let Some(timeout) = config.timeout {
            builder = builder.timeout(timeout);
        }
        if let Some(connect_timeout) = config.connect_timeout {
            builder = builder.connect_timeout(connect_timeout);
        }
        if let Some(idle) = config.pool_idle_timeout {
            builder = builder.pool_idle_timeout(idle);
        }
        if let Some(ka) = config.tcp_keepalive {
            builder = builder.tcp_keepalive(ka);
        }
        if config.http2 {
            builder = builder.http2_adaptive_window(true);
        } else {
            builder = builder.http1_only();
        }

        let client = builder
            .build()
            .map_err(|err| HttpError::Build(err.to_string()))?;

        Ok(Self {
            inner: Arc::new(client),
        })
    }

    /// 使用 [`HttpConfig::default`] 构造。
    pub fn with_defaults() -> Result<Self, HttpError> {
        Self::new(&HttpConfig::default())
    }

    /// 访问底层 `reqwest::Client`（进阶：自行拼 Request）。
    pub fn raw(&self) -> &Client {
        &self.inner
    }

    /// 开始构造指定 method + url 的请求。
    ///
    /// 返回的是 reqwest 的 [`RequestBuilder`]，可继续 `.header` / `.json` /
    /// `.timeout` 等，最后用 [`HttpClient::send`] 或 builder 自带的 `.send()`。
    pub fn request(&self, method: Method, url: &str) -> RequestBuilder {
        self.inner.request(method, url)
    }

    /// GET。
    pub fn get(&self, url: &str) -> RequestBuilder {
        self.inner.get(url)
    }

    /// POST。
    pub fn post(&self, url: &str) -> RequestBuilder {
        self.inner.post(url)
    }

    /// PUT。
    pub fn put(&self, url: &str) -> RequestBuilder {
        self.inner.put(url)
    }

    /// DELETE。
    pub fn delete(&self, url: &str) -> RequestBuilder {
        self.inner.delete(url)
    }

    /// 发送已构造好的 [`RequestBuilder`]，得到 [`HttpResponse`]。
    ///
    /// 此方法**不**自动把 4xx/5xx 当成错误；需要的话对结果调用
    /// [`HttpResponse::error_for_status`]。
    pub async fn send(&self, builder: RequestBuilder) -> Result<HttpResponse, HttpError> {
        let response = builder.send().await.map_err(HttpError::from_reqwest)?;
        Ok(HttpResponse::new(response))
    }

    /// JSON POST 便捷方法。
    ///
    /// - `url`：目标地址
    /// - `headers`：额外头（如 `Authorization`）；可为 `None`
    /// - `body`：序列化为 JSON 的请求体
    ///
    /// 成功时要求 2xx，然后将响应体反序列化为 `R`。
    pub async fn post_json<B, R>(
        &self,
        url: &str,
        headers: Option<&HeaderMap>,
        body: &B,
    ) -> Result<R, HttpError>
    where
        B: Serialize + ?Sized,
        R: DeserializeOwned,
    {
        let mut builder = self.post(url).json(body);
        if let Some(headers) = headers {
            builder = builder.headers(headers.clone());
        }
        self.send(builder)
            .await?
            .error_for_status()
            .await?
            .json()
            .await
    }

    /// GET 并将 JSON 响应反序列化。
    pub async fn get_json<R>(&self, url: &str, headers: Option<&HeaderMap>) -> Result<R, HttpError>
    where
        R: DeserializeOwned,
    {
        let mut builder = self.get(url);
        if let Some(headers) = headers {
            builder = builder.headers(headers.clone());
        }
        self.send(builder)
            .await?
            .error_for_status()
            .await?
            .json()
            .await
    }

    /// 发送请求并直接返回 body 字节流（要求 2xx）。
    ///
    /// 适合 LLM 流式补全、SSE、大文件下载。headers 在返回流之前即可读取
    ///（本方法目前只返回流；若需要同时拿 status/headers，请用
    /// [`HttpClient::send`] + [`HttpResponse::bytes_stream`]）。
    pub async fn send_bytes_stream(
        &self,
        builder: RequestBuilder,
    ) -> Result<ByteStream, HttpError> {
        let response = self.send(builder).await?.error_for_status().await?;
        Ok(response.bytes_stream())
    }
}

/// 构造单个请求头。
///
/// 对非法 header name/value 返回 [`HttpError::Build`] 风格的错误字符串封装
///（归入 [`HttpError::Other`]，因这是调用方输入问题）。
pub fn header(name: &str, value: &str) -> Result<(HeaderName, HeaderValue), HttpError> {
    let name: HeaderName = name
        .parse()
        .map_err(|err: reqwest::header::InvalidHeaderName| HttpError::Other(err.to_string()))?;
    let value = HeaderValue::from_str(value).map_err(|err| HttpError::Other(err.to_string()))?;
    Ok((name, value))
}

/// 截断错误 body，避免日志爆炸。
fn truncate_for_error(body: &str) -> String {
    const MAX: usize = 4096;
    if body.len() <= MAX {
        body.to_string()
    } else {
        let mut truncated = body.chars().take(MAX).collect::<String>();
        truncated.push('…');
        truncated
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_builds_with_defaults() {
        let client = HttpClient::with_defaults().expect("client should build");
        // clone 不应 panic / 不应新建不同池的可观察差异（此处只验证可 clone）
        let _cloned = client.clone();
    }

    #[test]
    fn config_builder_chain() {
        let cfg = HttpConfig::new()
            .with_timeout(None)
            .with_pool_max_idle_per_host(16)
            .with_user_agent("lya-test/0");
        assert!(cfg.timeout.is_none());
        assert_eq!(cfg.pool_max_idle_per_host, 16);
        assert_eq!(cfg.user_agent, "lya-test/0");
    }
}
