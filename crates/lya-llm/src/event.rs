//! 流式事件与完成态拼装。

use crate::message::ToolCall;

/// 流式 chat 的统一事件。
///
/// 一轮调用中可能交错出现正文与思考；tool_calls 按 `index` 分片到达；
/// 最后通常有一条带 `finish_reason` 的 [`StreamEvent::Finished`]
///（部分供应商只在最后一帧带 reason，也可能出现在含 delta 的同一 JSON 里——
/// 解析层会拆成先 delta 再 Finished）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamEvent {
    /// 助手正文增量（`delta.content`）。
    TextDelta(String),
    /// 思考/推理增量（`delta.reasoning_content` 或 `delta.reasoning`）。
    ReasoningDelta(String),
    /// tool_calls 某一 index 的增量片段。
    ToolCallDelta(ToolCallDelta),
    /// 本轮生成结束（`finish_reason`，如 `stop` / `tool_calls` / `length`）。
    Finished {
        /// 结束原因；可能为 `None`（对端未给或仅 `[DONE]`）。
        reason: Option<String>,
    },
}

/// 单个 tool_call 槽位的流式增量。
///
/// 同一 `index` 会多次到达：先带 `id`/`name`，再多次追加 `arguments` 片段。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolCallDelta {
    /// 在本轮 `tool_calls` 数组中的下标。
    pub index: usize,
    /// 调用 id（通常只在首片出现）。
    pub id: Option<String>,
    /// 函数名（通常只在首片出现）。
    pub name: Option<String>,
    /// arguments JSON 文本的增量片段。
    pub arguments: Option<String>,
}

/// 一轮调用拼好的完整结果。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ChatCompletion {
    /// 助手正文。
    pub content: String,
    /// 思考全文（若有）。
    pub reasoning: String,
    /// 完整 tool_calls（已按 index 合并）。
    pub tool_calls: Vec<ToolCall>,
    /// 结束原因。
    pub finish_reason: Option<String>,
}

/// 把 [`StreamEvent`] 序列拼成 [`ChatCompletion`]。
///
/// 也可在消费 `chat_stream` 时边收边 `apply`，结束时 `into_completion`。
#[derive(Debug, Default)]
pub struct CompletionAssembler {
    /// 正文缓冲。
    content: String,
    /// 思考缓冲。
    reasoning: String,
    /// 按 index 对齐的 tool_call 构建槽；`None` 表示该下标尚未出现。
    builders: Vec<Option<ToolCallBuilder>>,
    /// 最后见到的 finish_reason。
    finish_reason: Option<String>,
}

/// 单个 tool_call 的拼装槽。
#[derive(Debug, Default, Clone)]
struct ToolCallBuilder {
    /// 调用 id。
    id: String,
    /// 函数名。
    name: String,
    /// 参数 JSON 文本。
    arguments: String,
}

impl CompletionAssembler {
    /// 应用一条流式事件。
    pub fn apply(&mut self, event: &StreamEvent) {
        match event {
            StreamEvent::TextDelta(text) => self.content.push_str(text),
            StreamEvent::ReasoningDelta(text) => self.reasoning.push_str(text),
            StreamEvent::ToolCallDelta(delta) => self.apply_tool_delta(delta),
            StreamEvent::Finished { reason } => {
                if reason.is_some() {
                    self.finish_reason = reason.clone();
                }
            }
        }
    }

    /// 合并一个 tool_call 增量。
    fn apply_tool_delta(&mut self, delta: &ToolCallDelta) {
        while self.builders.len() <= delta.index {
            self.builders.push(None);
        }
        let slot = self.builders[delta.index].get_or_insert_with(ToolCallBuilder::default);
        if let Some(id) = &delta.id {
            if !id.is_empty() {
                slot.id = id.clone();
            }
        }
        if let Some(name) = &delta.name {
            if !name.is_empty() {
                slot.name = name.clone();
            }
        }
        if let Some(args) = &delta.arguments {
            slot.arguments.push_str(args);
        }
    }

    /// 取出完整 completion（消费 self）。
    pub fn into_completion(self) -> ChatCompletion {
        let tool_calls = self
            .builders
            .into_iter()
            .flatten()
            .filter(|b| !b.name.is_empty() || !b.id.is_empty())
            .map(|b| ToolCall {
                id: b.id,
                name: b.name,
                arguments: b.arguments,
            })
            .collect();
        ChatCompletion {
            content: self.content,
            reasoning: self.reasoning,
            tool_calls,
            finish_reason: self.finish_reason,
        }
    }
}
