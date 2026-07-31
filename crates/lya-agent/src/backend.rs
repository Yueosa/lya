//! LLM 后端抽象。
//!
//! 抽这一层只为一件事：让循环本身可测。真实实现就是
//! [`lya_llm::LlmClient`]，测试里换成脚本化的假后端，就能验证轮数上限、
//! 工具分发、HITL 挂起、取消这些逻辑，而不必真的联网。

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use lya_llm::{ChatEventStream, ChatMessage, LlmClient, LlmEndpoint, LlmError};
use serde_json::Value;

/// 发起一次流式 chat 的能力。
pub trait ChatBackend: Send + Sync {
    /// 发起流式请求。
    ///
    /// 参数取所有权而不是借用，省掉一层生命周期，反正每轮都要重新装配。
    fn chat_stream<'a>(
        &'a self,
        endpoint: &'a LlmEndpoint,
        messages: Vec<ChatMessage>,
        tools: Vec<Value>,
    ) -> Pin<Box<dyn Future<Output = Result<ChatEventStream, LlmError>> + Send + 'a>>;
}

/// 允许把后端包在 `Arc` 里共享给多个 agent。
impl<T: ChatBackend + ?Sized> ChatBackend for Arc<T> {
    fn chat_stream<'a>(
        &'a self,
        endpoint: &'a LlmEndpoint,
        messages: Vec<ChatMessage>,
        tools: Vec<Value>,
    ) -> Pin<Box<dyn Future<Output = Result<ChatEventStream, LlmError>> + Send + 'a>> {
        (**self).chat_stream(endpoint, messages, tools)
    }
}

impl ChatBackend for LlmClient {
    fn chat_stream<'a>(
        &'a self,
        endpoint: &'a LlmEndpoint,
        messages: Vec<ChatMessage>,
        tools: Vec<Value>,
    ) -> Pin<Box<dyn Future<Output = Result<ChatEventStream, LlmError>> + Send + 'a>> {
        Box::pin(async move { LlmClient::chat_stream(self, endpoint, &messages, &tools).await })
    }
}
