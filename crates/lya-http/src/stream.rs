//! 字节流类型别名与错误映射。
//!
//! LLM / SSE / 大文件下载都应优先走流，而不是 `bytes().await` 整包缓冲。

use bytes::Bytes;
use futures_core::Stream;
use std::pin::Pin;

use crate::error::HttpError;

/// 出站响应的字节流类型。
///
/// - 成功项：一块 `Bytes`（引用计数，clone 便宜）
/// - 错误项：已归类的 [`HttpError`]
///
/// 使用 `Pin<Box<dyn Stream...>>` 是为了抹掉 reqwest 具体 stream 类型，
/// 让上层 API 稳定、不泄漏 `reqwest` 细节到公共签名里太多。
pub type ByteStream = Pin<Box<dyn Stream<Item = Result<Bytes, HttpError>> + Send>>;

/// 将 reqwest 字节流错误映射为 [`HttpError`]。
///
/// 供 [`crate::HttpClient`] 内部使用；也允许上层在自己包了一层 stream
/// 时复用同一错误归类逻辑。
pub fn map_stream_error(err: reqwest::Error) -> HttpError {
    HttpError::from_reqwest(err)
}
