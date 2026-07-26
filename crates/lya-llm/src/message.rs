//! Chat 消息的 wire 类型。
//!
//! 这里的类型只服务「发给 / 从 API 收回」的 JSON 形状，
//! **不是**会话树里的领域消息。领域块（HITL、分支）由上层映射到本类型。

use serde_json::{json, Map, Value};

/// 消息角色（OpenAI 兼容四类）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// 系统提示。
    System,
    /// 用户输入。
    User,
    /// 助手输出（可含 tool_calls）。
    Assistant,
    /// 工具结果（需带 `tool_call_id`）。
    Tool,
}

impl Role {
    /// 序列化为 API 字符串。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::User => "user",
            Self::Assistant => "assistant",
            Self::Tool => "tool",
        }
    }
}

/// 一次完整的 function / tool 调用（非流式增量）。
///
/// `arguments` 是 **JSON 文本**（可能尚待上层 `serde_json::from_str`），
/// 本 crate 不做 schema 校验。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolCall {
    /// 调用 id（tool 结果消息要用）。
    pub id: String,
    /// 函数名。
    pub name: String,
    /// 参数 JSON 字符串。
    pub arguments: String,
}

/// 发给 chat/completions 的一条消息。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatMessage {
    /// 角色。
    pub role: Role,
    /// 文本内容；assistant 仅有 tool_calls 时可为空串。
    pub content: String,
    /// assistant 发起的 tool 调用列表。
    pub tool_calls: Option<Vec<ToolCall>>,
    /// role=tool 时对应的调用 id。
    pub tool_call_id: Option<String>,
}

impl ChatMessage {
    /// system 消息。
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: Role::System,
            content: content.into(),
            tool_calls: None,
            tool_call_id: None,
        }
    }

    /// user 消息。
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: content.into(),
            tool_calls: None,
            tool_call_id: None,
        }
    }

    /// assistant 纯文本消息。
    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: Role::Assistant,
            content: content.into(),
            tool_calls: None,
            tool_call_id: None,
        }
    }

    /// assistant 带 tool_calls（content 可空）。
    pub fn assistant_tool_calls(content: impl Into<String>, calls: Vec<ToolCall>) -> Self {
        Self {
            role: Role::Assistant,
            content: content.into(),
            tool_calls: Some(calls),
            tool_call_id: None,
        }
    }

    /// tool 结果消息。
    pub fn tool_result(tool_call_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: Role::Tool,
            content: content.into(),
            tool_calls: None,
            tool_call_id: Some(tool_call_id.into()),
        }
    }

    /// 序列化为 OpenAI 兼容 JSON object。
    pub fn to_json(&self) -> Value {
        let mut obj = Map::new();
        obj.insert("role".into(), json!(self.role.as_str()));
        obj.insert("content".into(), json!(self.content));
        if let Some(id) = &self.tool_call_id {
            obj.insert("tool_call_id".into(), json!(id));
        }
        if let Some(calls) = &self.tool_calls {
            if !calls.is_empty() {
                let arr: Vec<Value> = calls
                    .iter()
                    .map(|c| {
                        json!({
                            "id": c.id,
                            "type": "function",
                            "function": {
                                "name": c.name,
                                "arguments": c.arguments,
                            }
                        })
                    })
                    .collect();
                obj.insert("tool_calls".into(), Value::Array(arr));
            }
        }
        Value::Object(obj)
    }
}

/// 将消息列表转为 API `messages` 数组。
pub fn messages_to_json(messages: &[ChatMessage]) -> Value {
    Value::Array(messages.iter().map(ChatMessage::to_json).collect())
}

/// 组装 chat/completions 请求体。
///
/// - 以 `endpoint.params` 为底（含 `model` 等）
/// - 写入 `messages`、`stream`
/// - `tools` 非空时写入 `tools` 字段（元素应为 OpenAI tool 定义 JSON）
pub fn build_chat_body(
    endpoint: &crate::endpoint::LlmEndpoint,
    messages: &[ChatMessage],
    tools: &[Value],
    stream: bool,
) -> Value {
    let mut body = Value::Object(endpoint.params.clone());
    let obj = body.as_object_mut().expect("params 是 object");
    obj.insert("messages".into(), messages_to_json(messages));
    obj.insert("stream".into(), json!(stream));
    if !tools.is_empty() {
        obj.insert("tools".into(), Value::Array(tools.to_vec()));
    }
    body
}
