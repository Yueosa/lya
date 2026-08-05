//! # lya-llm
//!
//! OpenAI 兼容的 LLM 客户端：Chat Completions 与 Responses API 双栈。
//!
//! ## 设计目标
//!
//! - **只做传输与 wire 解析**：组请求、发 HTTP、解析非流式 JSON / SSE 流。
//! - **依赖 [`lya-http`]**：共享连接池；本 crate 不自建 Client。
//! - **流式优先**：[`LlmClient::chat_stream`] 产出 [`StreamEvent`]，
//!   上层可边收边展示；需要整包时用 [`CompletionAssembler`] 或
//!   [`LlmClient::chat`]。
//! - **供应商差异收敛在解析层**：例如 DeepSeek 的 `reasoning_content`
//!   与部分端的 `reasoning` 都映射为 [`StreamEvent::ReasoningDelta`]。
//!
//! ## 明确不做什么
//!
//! - 不负责 prompt 拼接（身份 / persona / tool schema / 历史）→ 留给 `lya-prompt`
//! - 不负责 tool 参数业务校验与执行 → 留给 `lya-tools` / agent
//! - 不做「失败自动重试」；是否重试由 agent 决定
//! - 不读 `model.toml`；endpoint 由上层注入 [`LlmEndpoint`]
//!
//! ## 模块结构
//!
//! - [`endpoint`] — 模型端点（URL / Key / 按栈 params）
//! - [`message`] — Completions 请求/响应侧 wire 类型
//! - [`responses`] — Responses API 请求体与 SSE 解析
//! - [`request`] — [`ChatStreamRequest`] 统一入口
//! - [`event`] — 流式事件与完成态拼装
//! - [`sse`] — Completions SSE `data:` 行解析
//! - [`client`] — [`LlmClient`]
//! - [`error`] — [`LlmError`]

#![deny(missing_docs)]

pub mod client;
pub mod endpoint;
pub mod error;
pub mod event;
pub mod message;
pub mod request;
pub mod responses;
pub mod sse;

pub use client::{ChatEventStream, LlmClient};
pub use endpoint::LlmEndpoint;
pub use error::LlmError;
// 调用栈与 capability 键住在 lya-base：它们是 models.toml 与请求体之间的合约，
// 配置层和这里都得认，而两边互不依赖。转出来省得调用方多写一条依赖
pub use lya_base::{ApiMode, CAPABILITY_TEXT, CAPABILITY_VISION, CAPABILITY_WEB_SEARCH};
pub use event::{
    ChatCompletion, CompletionAssembler, StreamEvent, ToolCallDelta, WebSearchStatus,
};
pub use message::{build_chat_body, ChatMessage, Role, ToolCall};
pub use request::ChatStreamRequest;
pub use responses::build_responses_body;
