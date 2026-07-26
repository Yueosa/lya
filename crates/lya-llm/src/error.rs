//! LLM 调用错误。
//!
//! 区分传输失败、协议解析失败与「对端返回了空 choices」等语义，
//! 方便 agent 决定是否重试或提示用户。

use lya_http::HttpError;

/// `lya-llm` 可返回的错误。
#[derive(Debug, thiserror::Error)]
pub enum LlmError {
    /// 底层 HTTP / 连接池错误。
    #[error(transparent)]
    Http(HttpError),

    /// 对端返回非成功 HTTP 状态（已尽量带上 body 摘要）。
    #[error("llm http {status}: {body}")]
    Api {
        /// HTTP 状态码（数值，避免本 crate 直接依赖 reqwest 类型）。
        status: u16,
        /// 响应体摘要。
        body: String,
    },

    /// SSE 行或 JSON 帧解析失败。
    #[error("llm sse/decode error: {0}")]
    Decode(String),

    /// 响应成功但 `choices` 为空或缺少 message。
    #[error("llm response has no usable choice")]
    EmptyChoices,

    /// 其它本层逻辑错误。
    #[error("llm error: {0}")]
    Other(String),
}

impl LlmError {
    /// 是否为超时（透传自 [`HttpError`]）。
    pub fn is_timeout(&self) -> bool {
        matches!(self, Self::Http(err) if err.is_timeout())
    }

    /// 是否为连接失败。
    pub fn is_connect(&self) -> bool {
        matches!(self, Self::Http(err) if err.is_connect())
    }
}

impl From<HttpError> for LlmError {
    fn from(err: HttpError) -> Self {
        if let HttpError::Status { status, body } = &err {
            return Self::Api {
                status: status.as_u16(),
                body: body.clone(),
            };
        }
        Self::Http(err)
    }
}
