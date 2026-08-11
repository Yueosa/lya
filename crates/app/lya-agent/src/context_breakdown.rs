//! 对话路径分块（与 [`super::context::build_messages`] 对齐，按 role 归类）。

use lya_session::{MessagePayload, MessageRecord, MessageRole, MessageStatus, OpenAiMessage};

use crate::context::{answered_call_ids, time_prefix, INTERRUPTED_MARK, MISSING_RESULT};

/// 活跃分支上各块将要（或不会）进入上下文的文本。
#[derive(Debug, Clone, Default)]
pub struct ConversationBreakdown {
    /// 用户消息（含时间前缀）。
    pub user: String,
    /// 模型正文（不含思考；含中断标注）。
    pub assistant: String,
    /// assistant 的 tool_calls（name + arguments）。
    pub tool_calls: String,
    /// tool 结果（含时间前缀；含孤儿占位）。
    pub tool_results: String,
    /// 路径上的 system 节点（不含已跳过的模式变更标记）。
    pub system: String,
    /// Responses 原生 output items（如 web_search_call）。
    pub provider_items: String,
    /// 落库但未回灌的思考。
    pub reasoning: String,
    /// HITL 节点（装配时跳过，不进 wire）。
    pub hitl: String,
}

/// 按与 wire 相同的规则遍历路径并分桶。
pub fn breakdown_path(path: &[MessageRecord]) -> ConversationBreakdown {
    let mut out = ConversationBreakdown::default();
    let answered = answered_call_ids(path);
    let mut previous_at = None;

    for record in path {
        let payload = &record.payload;
        let stamp = time_prefix(record.created_at, previous_at);
        previous_at = Some(record.created_at);

        if payload.role == MessageRole::Hitl {
            append_hitl(&mut out.hitl, payload);
            continue;
        }

        append_reasoning(&mut out.reasoning, payload);

        let Some(openai) = &payload.openai else {
            continue;
        };

        match payload.role {
            MessageRole::Assistant => {
                append_assistant(&mut out, payload, openai, payload.status);
                for call in openai.tool_calls.iter().flatten() {
                    if !answered.contains(&call.id) {
                        append_line(
                            &mut out.tool_results,
                            &format!("{stamp}{MISSING_RESULT}"),
                        );
                    }
                }
            }
            MessageRole::Tool => {
                let Some(call_id) = &openai.tool_call_id else {
                    continue;
                };
                let _ = call_id;
                append_line(
                    &mut out.tool_results,
                    &format!("{stamp}{}", openai.content),
                );
            }
            MessageRole::User => {
                append_line(&mut out.user, &format!("{stamp}{}", openai.content));
            }
            MessageRole::System => {
                if openai.content.starts_with("[模式变更]") {
                    continue;
                }
                append_line(&mut out.system, &openai.content);
            }
            MessageRole::Hitl => unreachable!("已在上面跳过"),
        }
    }

    out
}

fn append_reasoning(out: &mut String, payload: &MessagePayload) {
    if payload.role != MessageRole::Assistant {
        return;
    }
    if let Some(reasoning) = payload.lya.reasoning.as_deref() {
        if !reasoning.is_empty() {
            append_line(out, reasoning);
        }
    }
}

fn append_hitl(out: &mut String, payload: &MessagePayload) {
    if let Some(hitl) = &payload.lya.hitl {
        if let Ok(json) = serde_json::to_string(hitl) {
            append_line(out, &json);
        }
    }
    if let Some(openai) = &payload.openai {
        if !openai.content.is_empty() {
            append_line(out, &openai.content);
        }
    }
}

fn append_assistant(
    out: &mut ConversationBreakdown,
    payload: &MessagePayload,
    openai: &OpenAiMessage,
    status: MessageStatus,
) {
    for item in &payload.lya.responses_items {
        if let Ok(json) = serde_json::to_string(item) {
            append_line(&mut out.provider_items, &json);
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

    if !content.is_empty() {
        append_line(&mut out.assistant, &content);
    }

    if let Some(calls) = &openai.tool_calls {
        for call in calls {
            append_line(&mut out.tool_calls, &call.function.name);
            append_line(&mut out.tool_calls, &call.function.arguments);
        }
    }
}

fn append_line(out: &mut String, line: &str) {
    if line.is_empty() {
        return;
    }
    if !out.is_empty() {
        out.push('\n');
    }
    out.push_str(line);
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use lya_session::{
        HitlBlock, MessageKind, MessagePayload, MessageStatus, OpenAiFunction, OpenAiToolCall,
    };

    use super::*;
    use crate::context::build_messages;
    use lya_session::MessageRecord;

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
    fn splits_user_assistant_and_tool_results() {
        let mut assistant = MessagePayload::assistant_text("reply", MessageStatus::Complete);
        assistant.openai.as_mut().unwrap().tool_calls = Some(vec![OpenAiToolCall {
            id: "c1".into(),
            kind: "function".into(),
            function: OpenAiFunction {
                name: "read".into(),
                arguments: r#"{"p":"a"}"#.into(),
            },
        }]);
        let path = vec![
            record(1, MessagePayload::user_text("hi")),
            record(2, assistant),
            record(3, MessagePayload::tool_result("c1", "ok")),
        ];
        let breakdown = breakdown_path(&path);
        assert!(breakdown.user.contains("hi"));
        assert!(breakdown.assistant.contains("reply"));
        assert!(breakdown.tool_calls.contains("read"));
        assert!(breakdown.tool_results.contains("ok"));
        assert!(breakdown.hitl.is_empty());
    }

    #[test]
    fn reasoning_is_split_from_assistant() {
        let mut assistant = MessagePayload::assistant_text("out", MessageStatus::Complete);
        assistant.lya.reasoning = Some("think".into());
        let path = vec![record(1, assistant)];
        let breakdown = breakdown_path(&path);
        assert!(breakdown.reasoning.contains("think"));
        assert!(breakdown.assistant.contains("out"));
        assert!(!breakdown.assistant.contains("think"));
    }

    #[test]
    fn hitl_is_tracked_separately() {
        let path = vec![record(
            1,
            MessagePayload::hitl_pending(
                MessageKind::HitlResponse,
                HitlBlock::ToolConfirm {
                    tool_call_id: "c1".into(),
                    tool_name: "shell".into(),
                    arguments: serde_json::json!({}),
                    summary: String::new(),
                    steps: vec![],
                    reasons: vec![],
                },
            ),
        )];
        let breakdown = breakdown_path(&path);
        assert!(breakdown.hitl.contains("shell"));
        assert!(breakdown.user.is_empty());
    }

    #[test]
    fn wire_total_matches_build_messages_non_system() {
        let path = vec![
            record(1, MessagePayload::user_text("u")),
            record(
                2,
                MessagePayload::assistant_text("a", MessageStatus::Complete),
            ),
        ];
        let breakdown = breakdown_path(&path);
        let wire = format!(
            "{}{}{}{}{}",
            breakdown.user,
            breakdown.assistant,
            breakdown.tool_calls,
            breakdown.tool_results,
            breakdown.provider_items,
        );
        let messages = build_messages("SYS", &path, None);
        let serialized: String = messages
            .iter()
            .filter(|m| m.role != lya_llm::Role::System)
            .map(|m| m.content.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(wire.contains("u"));
        assert!(serialized.contains("u"));
    }
}
