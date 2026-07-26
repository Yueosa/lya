//! HTTP 错误类型。
//!
//! 本模块只描述**传输层 / HTTP 层**失败，不包含业务 JSON schema 错误。
//! 上层 crate（如 `lya-llm`）应在解析响应体时定义自己的错误，并用
//! `#[from] HttpError` 或显式映射包装本类型。

use reqwest::StatusCode;

/// lya-http 可返回的错误。
///
/// 变体按「调用方是否可能重试」大致分组：
/// - [`HttpError::Timeout`] / [`HttpError::Connect`]：通常可有限次重试
///   （幂等 GET / 明确可重试的 POST）
/// - [`HttpError::Status`]：看状态码；4xx 一般不重试，5xx 可策略性重试
/// - [`HttpError::Decode`] / [`HttpError::Body`]：多为本地或协议问题，慎重点
/// - [`HttpError::Build`]：配置非法，不应重试
#[derive(Debug, thiserror::Error)]
pub enum HttpError {
    /// 构造 `reqwest::Client` 失败（极少见，多为 TLS / 系统配置问题）。
    #[error("failed to build http client: {0}")]
    Build(String),

    /// 连接对端失败（DNS、拒绝连接、TLS 握手等）。
    #[error("connect failed: {0}")]
    Connect(String),

    /// 请求整体或连接阶段超时。
    #[error("request timed out: {0}")]
    Timeout(String),

    /// 对端返回了非成功 HTTP 状态码。
    ///
    /// `status` 为状态码；`body` 为已读到的响应体文本（可能截断），便于日志。
    #[error("http status {status}: {body}")]
    Status {
        /// HTTP 状态码。
        status: StatusCode,
        /// 响应体摘要（可能为空或截断）。
        body: String,
    },

    /// 读取或转发响应 body 时失败（连接中断、被 reset 等）。
    #[error("failed to read response body: {0}")]
    Body(String),

    /// 将 body 解码为调用方期望的格式失败（如 JSON）。
    #[error("failed to decode response: {0}")]
    Decode(String),

    /// 其它未细分的 reqwest 错误。
    #[error("http error: {0}")]
    Other(String),
}

impl HttpError {
    /// 从 `reqwest::Error` 归类到更稳定的变体。
    ///
    /// reqwest 的错误信息字符串会保留在变体里，方便排障；语义分支靠
    /// `is_timeout` / `is_connect` 等方法，而不是解析字符串。
    pub fn from_reqwest(err: reqwest::Error) -> Self {
        if err.is_timeout() {
            return Self::Timeout(err.to_string());
        }
        if err.is_connect() {
            return Self::Connect(err.to_string());
        }
        if err.is_body() || err.is_decode() {
            return Self::Body(err.to_string());
        }
        if let Some(status) = err.status() {
            return Self::Status {
                status,
                body: err.to_string(),
            };
        }
        Self::Other(err.to_string())
    }

    /// 是否为超时类错误。
    pub fn is_timeout(&self) -> bool {
        matches!(self, Self::Timeout(_))
    }

    /// 是否为连接失败。
    pub fn is_connect(&self) -> bool {
        matches!(self, Self::Connect(_))
    }

    /// 若为 [`HttpError::Status`]，返回状态码。
    pub fn status(&self) -> Option<StatusCode> {
        match self {
            Self::Status { status, .. } => Some(*status),
            _ => None,
        }
    }
}

impl From<reqwest::Error> for HttpError {
    fn from(err: reqwest::Error) -> Self {
        Self::from_reqwest(err)
    }
}
