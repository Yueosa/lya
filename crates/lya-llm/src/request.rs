//! 一次流式 chat 调用的请求体（按 API 栈区分 wire 形状）。

use serde_json::Value;

use crate::message::ChatMessage;

/// 上层装配好后交给 [`crate::LlmClient::chat_stream`] 的请求。
#[derive(Debug, Clone)]
pub enum ChatStreamRequest {
    /// Chat Completions：`messages` 数组（含 system）。
    Completions(Vec<ChatMessage>),
    /// Responses API：`instructions` + `input` item 列表。
    Responses {
        /// 系统提示，对应请求体 `instructions`。
        instructions: String,
        /// Responses input items。
        input: Vec<Value>,
        /// 是否在请求体注入 provider 原生 `web_search` tool。
        native_web_search: bool,
    },
}
