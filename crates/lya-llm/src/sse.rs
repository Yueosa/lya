//! SSE `data:` 行解析（OpenAI 兼容 chat/completions 流）。

use serde_json::Value;

use crate::error::LlmError;
use crate::event::{StreamEvent, ToolCallDelta};

/// 解析 SSE 的一行文本。
///
/// - 空行、注释行（`:` 开头）、非 `data:` 行 → `Ok(None)`
/// - `data: [DONE]` → `Ok(None)`（流结束由上层读完 body 判定；也可视作无事件）
/// - 合法 JSON → 拆成 0..N 条 [`StreamEvent`]（同一帧可同时含 content 与 finish_reason）
pub fn parse_sse_line(line: &str) -> Result<Option<Vec<StreamEvent>>, LlmError> {
    let line = line.trim();
    if line.is_empty() || line.starts_with(':') {
        return Ok(None);
    }
    let Some(data) = line.strip_prefix("data:") else {
        return Ok(None);
    };
    let data = data.trim();
    if data == "[DONE]" {
        return Ok(None);
    }
    let value: Value = serde_json::from_str(data)
        .map_err(|err| LlmError::Decode(format!("{err}; data={data}")))?;
    Ok(Some(extract_events(&value)))
}

/// 从一帧 chat completion chunk JSON 抽出事件列表。
pub fn extract_events(value: &Value) -> Vec<StreamEvent> {
    let mut events = Vec::new();
    let Some(choice) = value
        .get("choices")
        .and_then(|c| c.as_array())
        .and_then(|a| a.first())
    else {
        return events;
    };

    if let Some(delta) = choice.get("delta") {
        if let Some(c) = delta.get("content").and_then(|c| c.as_str()) {
            if !c.is_empty() {
                events.push(StreamEvent::TextDelta(c.to_string()));
            }
        }

        // DeepSeek 等：reasoning_content；少数端：reasoning
        if let Some(r) = delta
            .get("reasoning_content")
            .or_else(|| delta.get("reasoning"))
            .and_then(|r| r.as_str())
        {
            if !r.is_empty() {
                events.push(StreamEvent::ReasoningDelta(r.to_string()));
            }
        }

        if let Some(arr) = delta.get("tool_calls").and_then(|t| t.as_array()) {
            for item in arr {
                let index = item.get("index").and_then(|i| i.as_u64()).unwrap_or(0) as usize;
                let id = item
                    .get("id")
                    .and_then(|x| x.as_str())
                    .map(str::to_string);
                let name = item
                    .pointer("/function/name")
                    .and_then(|x| x.as_str())
                    .map(str::to_string);
                let arguments = item
                    .pointer("/function/arguments")
                    .and_then(|x| x.as_str())
                    .map(str::to_string);
                // 跳过完全空的增量
                if id.as_ref().is_none_or(|s| s.is_empty())
                    && name.as_ref().is_none_or(|s| s.is_empty())
                    && arguments.as_ref().is_none_or(|s| s.is_empty())
                {
                    continue;
                }
                events.push(StreamEvent::ToolCallDelta(ToolCallDelta {
                    index,
                    id,
                    name,
                    arguments,
                }));
            }
        }
    }

    if let Some(fr) = choice.get("finish_reason").and_then(|x| x.as_str()) {
        if !fr.is_empty() && fr != "null" {
            events.push(StreamEvent::Finished {
                reason: Some(fr.to_string()),
            });
        }
    }

    events
}

/// 从非流式 chat/completions JSON 提取 assistant message。
pub fn extract_completion(value: &Value) -> Result<crate::event::ChatCompletion, LlmError> {
    let choice = value
        .get("choices")
        .and_then(|c| c.as_array())
        .and_then(|a| a.first())
        .ok_or(LlmError::EmptyChoices)?;

    let finish_reason = choice
        .get("finish_reason")
        .and_then(|x| x.as_str())
        .filter(|s| !s.is_empty() && *s != "null")
        .map(str::to_string);

    let message = choice
        .get("message")
        .ok_or(LlmError::EmptyChoices)?;

    let content = message
        .get("content")
        .and_then(|c| c.as_str())
        .unwrap_or("")
        .to_string();

    let reasoning = message
        .get("reasoning_content")
        .or_else(|| message.get("reasoning"))
        .and_then(|r| r.as_str())
        .unwrap_or("")
        .to_string();

    let mut tool_calls = Vec::new();
    if let Some(arr) = message.get("tool_calls").and_then(|t| t.as_array()) {
        for item in arr {
            let id = item
                .get("id")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string();
            let name = item
                .pointer("/function/name")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string();
            let arguments = item
                .pointer("/function/arguments")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string();
            if !id.is_empty() || !name.is_empty() {
                tool_calls.push(crate::message::ToolCall {
                    id,
                    name,
                    arguments,
                });
            }
        }
    }

    Ok(crate::event::ChatCompletion {
        content,
        reasoning,
        tool_calls,
        finish_reason,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::CompletionAssembler;

    #[test]
    fn sse_text_and_done() {
        let line = r#"data: {"choices":[{"delta":{"content":"喵"}}]}"#;
        let events = parse_sse_line(line).unwrap().unwrap();
        assert_eq!(events, vec![StreamEvent::TextDelta("喵".into())]);
        assert!(parse_sse_line("data: [DONE]").unwrap().is_none());
    }

    #[test]
    fn sse_reasoning_fields() {
        let mut asm = CompletionAssembler::default();
        for line in [
            r#"data: {"choices":[{"delta":{"reasoning_content":"想一下…"}}]}"#,
            r#"data: {"choices":[{"delta":{"reasoning":"再想想"}}]}"#,
            r#"data: {"choices":[{"delta":{"content":"答案"},"finish_reason":"stop"}]}"#,
        ] {
            for ev in parse_sse_line(line).unwrap().unwrap() {
                asm.apply(&ev);
            }
        }
        let done = asm.into_completion();
        assert_eq!(done.reasoning, "想一下…再想想");
        assert_eq!(done.content, "答案");
        assert_eq!(done.finish_reason.as_deref(), Some("stop"));
    }

    #[test]
    fn sse_assembles_tool_calls() {
        let mut asm = CompletionAssembler::default();
        let lines = [
            r#"data: {"choices":[{"delta":{"tool_calls":[{"index":0,"id":"c1","type":"function","function":{"name":"bash","arguments":""}}]}}]}"#,
            r#"data: {"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"{\"op\":\"run\"}"}}]},"finish_reason":"tool_calls"}]}"#,
        ];
        for line in lines {
            for ev in parse_sse_line(line).unwrap().unwrap() {
                asm.apply(&ev);
            }
        }
        let done = asm.into_completion();
        assert_eq!(done.tool_calls.len(), 1);
        assert_eq!(done.tool_calls[0].id, "c1");
        assert_eq!(done.tool_calls[0].name, "bash");
        assert_eq!(done.tool_calls[0].arguments, r#"{"op":"run"}"#);
        assert_eq!(done.finish_reason.as_deref(), Some("tool_calls"));
    }
}
