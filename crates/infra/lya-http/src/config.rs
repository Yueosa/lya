//! HTTP 客户端配置。
//!
//! [`HttpConfig`] 描述「如何建连接池」，而不是单次请求的 header / body。
//! 单次请求的超时若需要覆盖，应在请求层用 `reqwest` 的 per-request timeout
//!（当前 [`crate::HttpClient`] 先统一用本配置的全局超时）。

use std::time::Duration;

/// 出站 HTTP 连接池与默认超时配置。
///
/// 字段全部是「池级 / 客户端级」参数：改动后需重新 [`crate::HttpClient::new`]
/// 才会生效（已有客户端不会热更新）。
#[derive(Debug, Clone)]
pub struct HttpConfig {
    /// 整次请求的默认超时（含连接 + 读 body）。
    ///
    /// `None` 表示不设全局超时。流式 LLM 响应往往很长，上层若自己管
    /// 读超时，可设为 `None`，或把本值设得足够大。
    pub timeout: Option<Duration>,

    /// 建立 TCP / TLS 连接的超时。
    ///
    /// 只覆盖「连上对端」阶段，不含读 body。`None` 表示沿用 reqwest 默认。
    pub connect_timeout: Option<Duration>,

    /// 池中空闲连接的最长存活时间。
    ///
    /// 超时后连接被丢弃，下次请求会重新握手。过短会浪费握手；过长可能
    /// 撞上对端/中间设备的静默断连。
    pub pool_idle_timeout: Option<Duration>,

    /// 每个 host 允许保留的空闲连接上限。
    ///
    /// 影响并发复用能力。对同一 API（如 DeepSeek）高频短请求可适当加大。
    pub pool_max_idle_per_host: usize,

    /// 是否启用 TCP keepalive。
    ///
    /// 有助于探测半开连接；对长 SSE / 长流式响应尤其有用。
    pub tcp_keepalive: Option<Duration>,

    /// 是否启用 HTTP/2。
    ///
    /// 默认开启。部分旧代理可能对 h2 不友好，可关回 HTTP/1.1。
    pub http2: bool,

    /// User-Agent 字符串。
    ///
    /// 便于对端日志识别；也避免某些 CDN 对缺省 UA 的怪异行为。
    pub user_agent: String,
}

impl Default for HttpConfig {
    /// 面向本机 agent 的保守默认值。
    ///
    /// - 总超时 120s：覆盖多数 LLM 非流式调用；流式场景建议上层覆盖或置 `None`
    /// - 连接超时 10s
    /// - 空闲连接 90s
    /// - 每 host 空闲 8 条
    /// - TCP keepalive 30s
    fn default() -> Self {
        Self {
            timeout: Some(Duration::from_secs(120)),
            connect_timeout: Some(Duration::from_secs(10)),
            pool_idle_timeout: Some(Duration::from_secs(90)),
            pool_max_idle_per_host: 8,
            tcp_keepalive: Some(Duration::from_secs(30)),
            http2: true,
            user_agent: "lya-http/0.1".to_string(),
        }
    }
}

impl HttpConfig {
    /// 构造一份默认配置（等同 [`Default::default`]）。
    pub fn new() -> Self {
        Self::default()
    }

    /// 设置整次请求默认超时。传入 `None` 表示禁用全局超时。
    pub fn with_timeout(mut self, timeout: Option<Duration>) -> Self {
        self.timeout = timeout;
        self
    }

    /// 设置连接超时。
    pub fn with_connect_timeout(mut self, connect_timeout: Option<Duration>) -> Self {
        self.connect_timeout = connect_timeout;
        self
    }

    /// 设置每 host 空闲连接上限。
    pub fn with_pool_max_idle_per_host(mut self, n: usize) -> Self {
        self.pool_max_idle_per_host = n;
        self
    }

    /// 设置 User-Agent。
    pub fn with_user_agent(mut self, ua: impl Into<String>) -> Self {
        self.user_agent = ua.into();
        self
    }
}
