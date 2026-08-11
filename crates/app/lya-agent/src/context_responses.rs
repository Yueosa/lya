//! 把消息树的一条路径装配成 Responses API 的 `input` item 列表。
//!
//! 规则与 [`super::context::build_messages`] 对齐：HITL 跳过、思考不回灌、
//! 中断标注、孤儿 tool_call 补全、user/tool 时间戳前缀。

use lya_llm::responses::input::{
    function_call, function_call_output, message_assistant, message_system, message_user,
    normalize_web_search_call_item,
};
use lya_session::{MessagePayload, MessageRecord, MessageRole, MessageStatus, OpenAiMessage};
use serde_json::Value;

use crate::context::{answered_call_ids, time_prefix, INTERRUPTED_MARK, MISSING_RESULT};

/// 装配 Responses 请求的 `instructions` + `input`。
///
/// `system_prompt` 进入 `instructions`；历史消息转为 input items。
/// `memory_section` 挂在历史之后（与 Completions 路径一致）。
pub fn build_responses_input(
    system_prompt: &str,
    path: &[MessageRecord],
    memory_section: Option<&str>,
) -> (String, Vec<Value>) {
    let mut input = Vec::with_capacity(path.len() + 1);
    let answered = answered_call_ids(path);
    let mut previous_at = None;

    for record in path {
        let payload = &record.payload;
        let stamp = time_prefix(record.created_at, previous_at);
        previous_at = Some(record.created_at);

        if payload.role == MessageRole::Hitl {
            continue;
        }
        let Some(openai) = &payload.openai else {
            continue;
        };

        match payload.role {
            MessageRole::Assistant => {
                push_assistant_items(&mut input, payload, openai, payload.status);
                for call in openai.tool_calls.iter().flatten() {
                    if !answered.contains(&call.id) {
                        input.push(function_call_output(call.id.clone(), MISSING_RESULT));
                    }
                }
            }
            MessageRole::Tool => {
                let Some(call_id) = &openai.tool_call_id else {
                    continue;
                };
                input.push(function_call_output(
                    call_id.clone(),
                    format!("{stamp}{}", openai.content),
                ));
            }
            MessageRole::User => {
                input.push(message_user(format!("{stamp}{}", openai.content)));
            }
            MessageRole::System => {
                if openai.content.starts_with("[模式变更]") {
                    continue;
                }
                input.push(message_system(&openai.content));
            }
            MessageRole::Hitl => unreachable!("已在上面跳过"),
        }
    }

    if let Some(memory) = memory_section.map(str::trim).filter(|s| !s.is_empty()) {
        input.push(message_user(memory));
    }

    (system_prompt.to_string(), input)
}

fn push_assistant_items(
    out: &mut Vec<Value>,
    payload: &MessagePayload,
    openai: &OpenAiMessage,
    status: MessageStatus,
) {
    for item in &payload.lya.responses_items {
        if item.get("type").and_then(|t| t.as_str()) == Some("web_search_call") {
            out.push(normalize_web_search_call_item(item));
        } else {
            out.push(item.clone());
        }
    }

    let mut content = openai.content.clone();
    if matches!(
        status,
        MessageStatus::Interrupted | MessageStatus::Streaming
    ) {
        if !content.is_empty() {
            content.push('\n');
        }
        content.push_str(INTERRUPTED_MARK);
    }

    if let Some(calls) = &openai.tool_calls {
        if !calls.is_empty() {
            if !content.is_empty() {
                out.push(message_assistant(content));
            }
            for call in calls {
                out.push(function_call(
                    call.id.clone(),
                    call.function.name.clone(),
                    call.function.arguments.clone(),
                ));
            }
            return;
        }
    }

    if !content.is_empty() {
        out.push(message_assistant(content));
    }
}

#[cfg(test)]
mod tests {
    use lya_session::{
        HitlBlock, MessageKind, MessagePayload, OpenAiFunction, OpenAiToolCall,
    };

    use super::*;
    use crate::context::build_messages;
    use chrono::Utc;
    use lya_session::MessageRecord;
    use serde_json::json;

    fn record(id: i64, payload: MessagePayload) -> MessageRecord {
        MessageRecord {
            id,
            session_id: "s".into(),
            parent_id: None,
            sort_key: id,
            payload,
            created_at: Utc::now(),
        }
    }

    #[test]
    fn single_user_turn() {
        let path = vec![record(
            1,
            MessagePayload::user_text("你好"),
        )];
        let (instructions, input) = build_responses_input("SYSTEM", &path, None);
        assert_eq!(instructions, "SYSTEM");
        assert_eq!(input.len(), 1);
        assert_eq!(input[0]["type"], "message");
        assert_eq!(input[0]["role"], "user");
    }

    #[test]
    fn tool_round_trip() {
        let mut assistant = MessagePayload::assistant_text("我查一下", MessageStatus::Complete);
        assistant.kind = MessageKind::ToolCall;
        assistant.openai.as_mut().unwrap().tool_calls = Some(vec![OpenAiToolCall {
            id: "c1".into(),
            kind: "function".into(),
            function: OpenAiFunction {
                name: "echo".into(),
                arguments: r#"{"text":"x"}"#.into(),
            },
        }]);
        let tool = MessagePayload::tool_result("c1", "echo: x");
        let path = vec![record(1, MessagePayload::user_text("测")), record(2, assistant), record(3, tool)];
        let (_, input) = build_responses_input("S", &path, None);
        assert!(input.iter().any(|i| i["type"] == "function_call"));
        assert!(input.iter().any(|i| i["type"] == "function_call_output"));
    }

    #[test]
    fn hitl_skipped_like_completions() {
        let path = vec![
            record(1, MessagePayload::user_text("x")),
            record(
                2,
                MessagePayload::hitl_pending(
                    MessageKind::ModeChange,
                    HitlBlock::ModeChange {
                        to_mode: "edit".into(),
                        reason: "要改".into(),
                    },
                ),
            ),
        ];
        let completions = build_messages("S", &path, None);
        let (_, responses) = build_responses_input("S", &path, None);
        assert_eq!(completions.len(), 2);
        assert_eq!(responses.len(), 1);
    }

    #[test]
    fn responses_items_missing_action_normalized_on_replay() {
        let mut assistant = MessagePayload::assistant_text("查完了", MessageStatus::Complete);
        assistant.lya.responses_items = vec![serde_json::json!({
            "type": "web_search_call",
            "id": "ws1",
            "status": "completed"
        })];
        let path = vec![record(1, MessagePayload::user_text("今天天气")), record(2, assistant)];
        let (_, input) = build_responses_input("S", &path, None);
        assert_eq!(input[1]["type"], "web_search_call");
        assert_eq!(input[1]["action"]["type"], "search");
        assert_eq!(input[1]["action"]["queries"], json!([]));
    }

    #[test]
    fn responses_items_replayed_before_content() {
        let mut assistant = MessagePayload::assistant_text("查完了", MessageStatus::Complete);
        assistant.lya.responses_items = vec![serde_json::json!({
            "type": "web_search_call",
            "id": "ws1",
            "status": "completed",
            "action": { "type": "search", "queries": ["天气"] }
        })];
        let path = vec![record(1, MessagePayload::user_text("今天天气")), record(2, assistant)];
        let (_, input) = build_responses_input("S", &path, None);
        assert_eq!(input[0]["type"], "message");
        assert_eq!(input[1]["type"], "web_search_call");
        assert_eq!(input[1]["id"], "ws1");
        assert_eq!(input[2]["type"], "message");
        assert_eq!(input[2]["role"], "assistant");
    }
}
