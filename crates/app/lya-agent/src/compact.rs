//! 手动上下文压缩：把较旧的工具结果换成短占位，原文留在 `lya.full_content`。
//!
//! 策略：按工具正文 token 从旧到新裁，大约丢掉一半、保留最近一半；
//! 且至少保住最后一个 tool round。不做对话 fold。

use lya_session::{MessageRecord, MessageRole};
use lya_token::count_text;
use serde::Serialize;

use crate::agent::Agent;
use crate::backend::ChatBackend;
use crate::error::AgentError;

/// 短于该 token 数的工具结果不压缩（减少噪音占位）。
const MIN_PRUNE_TOKENS: u64 = 512;

/// 一次压缩的结果。
#[derive(Debug, Clone, Serialize)]
pub struct CompactReport {
    /// 被换成占位的工具消息条数。
    pub pruned: usize,
    /// 估算省下的 token（原文 − 占位）。
    pub saved_tokens: u64,
}

impl<B: ChatBackend> Agent<B> {
    /// 压缩当前活跃分支上较旧的工具结果。
    pub fn compact_tool_results(&self, session_id: &str) -> Result<CompactReport, AgentError> {
        let sessions = self.sessions();
        if sessions.get_session(session_id)?.is_none() {
            return Err(AgentError::Invalid(format!("会话不存在：{session_id}")));
        }

        let path = sessions.path_to_active_leaf(session_id)?;
        let plan = plan_prune(&path);
        if plan.is_empty() {
            return Ok(CompactReport {
                pruned: 0,
                saved_tokens: 0,
            });
        }

        let name_of = |call_id: &str| tool_name_for(&path, call_id);
        let mut pruned = 0usize;
        let mut saved_tokens = 0u64;

        for msg_id in plan {
            let record = sessions.get_message(session_id, msg_id)?;
            let Some(openai) = record.payload.openai.as_ref() else {
                continue;
            };
            if record.payload.role != MessageRole::Tool || record.payload.is_compacted() {
                continue;
            }
            let original = openai.content.clone();
            let original_tokens = count_text(&original);
            if original_tokens < MIN_PRUNE_TOKENS {
                continue;
            }
            let call_id = openai.tool_call_id.as_deref().unwrap_or("?");
            let tool_name = name_of(call_id);
            let placeholder = format!(
                "（已省略：{tool_name}，约 {original_tokens} tokens；需要请再调用工具获取）"
            );
            let placeholder_tokens = count_text(&placeholder);
            saved_tokens = saved_tokens.saturating_add(original_tokens.saturating_sub(placeholder_tokens));

            let mut payload = record.payload.clone();
            if let Some(openai) = payload.openai.as_mut() {
                openai.content = placeholder;
            }
            payload.lya.full_content = Some(original);
            sessions.update_payload(session_id, msg_id, &payload)?;
            pruned += 1;
        }

        Ok(CompactReport {
            pruned,
            saved_tokens,
        })
    }
}

/// 选出要压缩的消息 id（旧 → 新顺序中的「该丢掉」那一半）。
fn plan_prune(path: &[MessageRecord]) -> Vec<i64> {
    let protected = last_tool_round_ids(path);
    let mut candidates: Vec<(i64, u64)> = Vec::new();

    for record in path {
        if record.payload.role != MessageRole::Tool {
            continue;
        }
        if record.payload.is_compacted() {
            continue;
        }
        if protected.contains(&record.id) {
            continue;
        }
        let Some(content) = record.payload.wire_content() else {
            continue;
        };
        let tokens = count_text(content);
        if tokens < MIN_PRUNE_TOKENS {
            continue;
        }
        candidates.push((record.id, tokens));
    }

    if candidates.is_empty() {
        return Vec::new();
    }

    let total: u64 = candidates.iter().map(|(_, t)| *t).sum();
    // 丢掉约一半 token 质量；至少丢一条（若只有一条候选则丢它，除非它被 last round 保护——已排除）
    let mut drop_budget = total / 2;
    if drop_budget == 0 {
        drop_budget = candidates[0].1;
    }

    let mut drop = Vec::new();
    let mut dropped = 0u64;
    for (id, tokens) in &candidates {
        if dropped >= drop_budget {
            break;
        }
        drop.push(*id);
        dropped += *tokens;
    }
    drop
}

/// 最后一个带 tool_calls 的 assistant 所对应的全部 tool 结果 id。
fn last_tool_round_ids(path: &[MessageRecord]) -> Vec<i64> {
    let mut last_call_ids: Vec<String> = Vec::new();
    for record in path.iter().rev() {
        if record.payload.role != MessageRole::Assistant {
            continue;
        }
        let Some(openai) = &record.payload.openai else {
            continue;
        };
        if let Some(calls) = &openai.tool_calls {
            if !calls.is_empty() {
                last_call_ids = calls.iter().map(|c| c.id.clone()).collect();
                break;
            }
        }
    }
    if last_call_ids.is_empty() {
        return Vec::new();
    }

    path.iter()
        .filter(|record| record.payload.role == MessageRole::Tool)
        .filter(|record| {
            record
                .payload
                .openai
                .as_ref()
                .and_then(|o| o.tool_call_id.as_ref())
                .is_some_and(|id| last_call_ids.iter().any(|c| c == id))
        })
        .map(|record| record.id)
        .collect()
}

fn tool_name_for(path: &[MessageRecord], call_id: &str) -> String {
    for record in path {
        if record.payload.role != MessageRole::Assistant {
            continue;
        }
        let Some(calls) = record
            .payload
            .openai
            .as_ref()
            .and_then(|o| o.tool_calls.as_ref())
        else {
            continue;
        };
        for call in calls {
            if call.id == call_id {
                return call.function.name.clone();
            }
        }
    }
    "tool".into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use lya_session::{
        MessageKind, MessagePayload, MessageStatus, OpenAiFunction, OpenAiMessage, OpenAiToolCall,
    };

    fn tool_record(id: i64, call_id: &str, content: &str) -> MessageRecord {
        MessageRecord {
            id,
            session_id: "s".into(),
            parent_id: Some(id - 1),
            sort_key: id,
            payload: MessagePayload::tool_result(call_id, content),
            created_at: Utc::now(),
        }
    }

    fn assistant_calls(id: i64, calls: &[(&str, &str)]) -> MessageRecord {
        let mut payload = MessagePayload::assistant_text("", MessageStatus::Complete);
        payload.kind = MessageKind::ToolCall;
        payload.openai = Some(OpenAiMessage {
            role: "assistant".into(),
            content: String::new(),
            tool_calls: Some(
                calls
                    .iter()
                    .map(|(cid, name)| OpenAiToolCall {
                        id: (*cid).into(),
                        kind: "function".into(),
                        function: OpenAiFunction {
                            name: (*name).into(),
                            arguments: "{}".into(),
                        },
                    })
                    .collect(),
            ),
            tool_call_id: None,
        });
        MessageRecord {
            id,
            session_id: "s".into(),
            parent_id: Some(id - 1),
            sort_key: id,
            payload,
            created_at: Utc::now(),
        }
    }

    fn big(n: usize) -> String {
        "字".repeat(n)
    }

    #[test]
    fn keeps_last_tool_round_and_drops_older_half() {
        // a1+t1, a2+t2, a3+t3(last round) — t3 受保护；在 t1/t2 里按 token 丢掉约一半
        let t1 = big(800);
        let t2 = big(800);
        let t3 = big(800);
        let path = vec![
            assistant_calls(1, &[("c1", "file_read")]),
            tool_record(2, "c1", &t1),
            assistant_calls(3, &[("c2", "file_read")]),
            tool_record(4, "c2", &t2),
            assistant_calls(5, &[("c3", "file_read")]),
            tool_record(6, "c3", &t3),
        ];
        let drop = plan_prune(&path);
        assert!(!drop.contains(&6), "最后一轮不能裁");
        assert!(!drop.is_empty());
        assert!(drop.iter().all(|id| *id == 2 || *id == 4));
    }

    #[test]
    fn skips_already_compacted_and_short() {
        let short = tool_record(2, "c1", "ok");
        let mut compacted = tool_record(4, "c2", &big(900));
        compacted.payload.lya.full_content = Some(big(900));
        compacted.payload.openai.as_mut().unwrap().content = "（已省略）".into();

        let path = vec![
            assistant_calls(1, &[("c1", "echo")]),
            short,
            assistant_calls(3, &[("c2", "echo")]),
            compacted,
            assistant_calls(5, &[("c3", "echo")]),
            tool_record(6, "c3", &big(900)),
        ];
        assert!(plan_prune(&path).is_empty());
    }
}
