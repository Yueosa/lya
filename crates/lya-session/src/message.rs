//! 自研 `message_json`：外壳 + 内嵌 OpenAI + lya 扩展。

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// 消息角色（树查询 / HITL 用；可与 OpenAI role 对齐或扩展）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    /// 系统（少用于持久树）。
    System,
    /// 用户。
    User,
    /// 助手。
    Assistant,
    /// 工具结果。
    Tool,
    /// 人机交互打断节点（独立 node）。
    Hitl,
}

impl MessageRole {
    /// 字符串。
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::User => "user",
            Self::Assistant => "assistant",
            Self::Tool => "tool",
            Self::Hitl => "hitl",
        }
    }
}

/// 消息细类。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageKind {
    /// 普通对话。
    Chat,
    /// 工具调用回合的助手消息。
    ToolCall,
    /// 工具结果。
    ToolResult,
    /// HITL：表单。
    Form,
    /// HITL：工具确认。
    ToolConfirm,
    /// HITL：模式切换确认。
    ModeChange,
    /// HITL 用户答复（可选单独节点；也可直接 append user）。
    HitlResponse,
}

/// 消息生命周期状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageStatus {
    /// 流式写入中。
    Streaming,
    /// 等待用户（HITL）。
    Pending,
    /// 已完成。
    Complete,
    /// 中断 / 崩溃残留。
    Interrupted,
    /// HITL 已解决。
    Resolved,
}

/// 内嵌的 OpenAI 兼容消息体（可直接映射到 `lya-llm::ChatMessage`）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OpenAiMessage {
    /// OpenAI role。
    pub role: String,
    /// 文本内容；可空。
    #[serde(default)]
    pub content: String,
    /// assistant 的 tool_calls。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<OpenAiToolCall>>,
    /// tool 结果对应的 call id。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

/// OpenAI tool_call 结构。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OpenAiToolCall {
    /// 调用 id。
    pub id: String,
    /// 固定 `"function"`。
    #[serde(rename = "type")]
    pub kind: String,
    /// 函数名与参数。
    pub function: OpenAiFunction,
}

/// function 字段。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OpenAiFunction {
    /// 名称。
    pub name: String,
    /// 参数 JSON 文本。
    pub arguments: String,
}

/// HITL 种类。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HitlKind {
    /// 结构化表单。
    Form,
    /// 工具确认。
    ToolConfirm,
    /// 模式切换确认。
    ModeChange,
}

/// 表单题型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FormQuestionKind {
    /// 单选，必须给选项。
    Single,
    /// 多选，必须给选项。
    Multi,
    /// 自由文本，不给选项。
    ///
    /// 上一代实现只有单选/多选，「它在哪个目录？」这类问题只能靠表单级的
    /// 补充说明兜，很别扭，所以这里补上正经的文本题。
    Text,
}

/// 表单选项。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FormOption {
    /// 提交时回传的值。
    pub key: String,
    /// 展示给用户的文案。
    pub label: String,
}

/// 表单里的一道题。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FormQuestion {
    /// 题目 id，表单内唯一，答案按它对应。
    pub id: String,
    /// 题干。
    pub text: String,
    /// 题型。
    pub kind: FormQuestionKind,
    /// 选项；文本题为空。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<FormOption>,
    /// 是否额外提供一个备注输入框。
    ///
    /// 显式声明而不是像上一代那样给所有题都挂备注——出题的时候就该想清楚
    /// 这题要不要补充说明。
    #[serde(default)]
    pub allow_note: bool,
}

/// HITL 块内容。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum HitlBlock {
    /// 表单。
    Form {
        /// 表单 id。
        form_id: String,
        /// 标题。
        title: String,
        /// 题目列表。
        questions: Vec<FormQuestion>,
    },
    /// 工具确认。
    ToolConfirm {
        /// 对应 tool_call id。
        tool_call_id: String,
        /// 工具名。
        tool_name: String,
        /// 展示用预览。
        #[serde(default)]
        preview: String,
    },
    /// 模式切换。
    ModeChange {
        /// 目标模式。
        to_mode: String,
        /// 说明。
        #[serde(default)]
        reason: String,
    },
}

/// lya 扩展字段。
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct LyaExtras {
    /// 思考全文（可选；也可只放 blocks）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<String>,
    /// UI / 分析用块列表（自由 JSON 数组，便于演进）。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blocks: Vec<Value>,
    /// HITL 专用结构化块。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hitl: Option<HitlBlock>,
    /// 其它元数据。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Value>,
}

/// 持久化消息 JSON 根对象。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MessagePayload {
    /// schema 版本。
    pub v: u32,
    /// 角色。
    pub role: MessageRole,
    /// 细类。
    pub kind: MessageKind,
    /// 状态。
    pub status: MessageStatus,
    /// OpenAI 兼容体；HITL 未决时常为 `None`。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub openai: Option<OpenAiMessage>,
    /// lya 扩展。
    #[serde(default)]
    pub lya: LyaExtras,
}

impl MessagePayload {
    /// 当前 schema 版本。
    pub const VERSION: u32 = 1;

    /// 用户文本消息。
    pub fn user_text(content: impl Into<String>) -> Self {
        let content = content.into();
        Self {
            v: Self::VERSION,
            role: MessageRole::User,
            kind: MessageKind::Chat,
            status: MessageStatus::Complete,
            openai: Some(OpenAiMessage {
                role: "user".into(),
                content: content.clone(),
                tool_calls: None,
                tool_call_id: None,
            }),
            lya: LyaExtras {
                blocks: vec![serde_json::json!({ "type": "text", "text": content })],
                ..Default::default()
            },
        }
    }

    /// 系统提示消息。
    ///
    /// 用于把「用户手动切换了工作模式」这类系统侧事件写进树。做成持久节点
    /// 而不是临时消息，是因为树是唯一真相：以后回看这段对话，仍然解释得通
    /// 助手的行为边界为什么变了。
    pub fn system_text(content: impl Into<String>) -> Self {
        let content = content.into();
        Self {
            v: Self::VERSION,
            role: MessageRole::System,
            kind: MessageKind::Chat,
            status: MessageStatus::Complete,
            openai: Some(OpenAiMessage {
                role: "system".into(),
                content: content.clone(),
                tool_calls: None,
                tool_call_id: None,
            }),
            lya: LyaExtras {
                blocks: vec![serde_json::json!({ "type": "text", "text": content })],
                ..Default::default()
            },
        }
    }

    /// 助手文本（可标 streaming）。
    pub fn assistant_text(content: impl Into<String>, status: MessageStatus) -> Self {
        let content = content.into();
        Self {
            v: Self::VERSION,
            role: MessageRole::Assistant,
            kind: MessageKind::Chat,
            status,
            openai: Some(OpenAiMessage {
                role: "assistant".into(),
                content: content.clone(),
                tool_calls: None,
                tool_call_id: None,
            }),
            lya: LyaExtras {
                blocks: vec![serde_json::json!({ "type": "text", "text": content })],
                ..Default::default()
            },
        }
    }

    /// 工具结果。
    pub fn tool_result(tool_call_id: impl Into<String>, content: impl Into<String>) -> Self {
        let tool_call_id = tool_call_id.into();
        let content = content.into();
        Self {
            v: Self::VERSION,
            role: MessageRole::Tool,
            kind: MessageKind::ToolResult,
            status: MessageStatus::Complete,
            openai: Some(OpenAiMessage {
                role: "tool".into(),
                content: content.clone(),
                tool_calls: None,
                tool_call_id: Some(tool_call_id),
            }),
            lya: LyaExtras::default(),
        }
    }

    /// 未决 HITL 节点。
    pub fn hitl_pending(kind: MessageKind, block: HitlBlock) -> Self {
        Self {
            v: Self::VERSION,
            role: MessageRole::Hitl,
            kind,
            status: MessageStatus::Pending,
            openai: None,
            lya: LyaExtras {
                hitl: Some(block),
                ..Default::default()
            },
        }
    }

    /// 是否为未决 HITL。
    pub fn is_pending_hitl(&self) -> bool {
        self.role == MessageRole::Hitl && self.status == MessageStatus::Pending
    }
}
