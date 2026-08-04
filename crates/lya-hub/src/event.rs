//! LyaSSE：服务端推给客户端的事件信封。
//!
//! SSE 在 lya 里不只是「聊天增量的管道」，而是**打破 REST 只能客户端发问**的那
//! 一层。以后还要承载桌面通知、会话列表变化、配置被另一端改动等等，所以信封
//! 从一开始就留出 [`Envelope::scope`]：
//!
//! ```text
//! event: message_delta
//! data: {"scope":"session:abc","type":"message_delta","seq":42,"payload":{…}}
//! ```
//!
//! 现在只有 `session:<id>` 一种作用域。将来加 `global` 时，客户端的分发逻辑
//! **一行都不用改**——它本来就按 scope 路由；想把两者合并成一条连接也只是服务端
//! 多路复用，协议不变。反过来，若现在把事件写成裸的 `{"delta":"…"}`，加第二种
//! 事件源时所有客户端都得跟着改。

use lya_agent::{AgentEvent, CallKind, TurnEndReason};
use serde::Serialize;
use serde_json::{Value, json};

/// 事件作用域。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Scope {
    /// 某个会话内的事件。
    Session(String),
    /// 全应用事件：通知、会话列表变化、配置变更等。
    Global,
}

impl Scope {
    /// 线上表示。
    pub fn as_wire(&self) -> String {
        match self {
            Self::Session(id) => format!("session:{id}"),
            Self::Global => "global".into(),
        }
    }
}

/// 一条推给客户端的事件。
#[derive(Debug, Clone, Serialize)]
pub struct Envelope {
    /// 作用域，形如 `session:<id>` 或 `global`。
    pub scope: String,
    /// 事件类型，同时也是 SSE 的 `event:` 字段。
    #[serde(rename = "type")]
    pub kind: String,
    /// 递增序号；只用于排查问题，客户端不必依赖它对齐——
    /// 快照本身就是幂等的。
    pub seq: u64,
    /// 事件内容。
    pub payload: Value,
}

impl Envelope {
    /// 构造一条会话内事件。
    pub fn session(id: &str, kind: &str, seq: u64, payload: Value) -> Self {
        Self {
            scope: Scope::Session(id.to_string()).as_wire(),
            kind: kind.to_string(),
            seq,
            payload,
        }
    }
}

/// 把 agent 的事件翻成线上事件。
///
/// 返回 `None` 表示这条事件不外发（比如纯粹的内部计数）。
pub fn from_agent(session_id: &str, seq: u64, event: &AgentEvent) -> Option<Envelope> {
    let (kind, payload) = match event {
        AgentEvent::RoundStarted { round } => ("round_started", json!({ "round": round })),
        AgentEvent::Delta(text) => ("message_delta", json!({ "text": text })),
        AgentEvent::Reasoning(text) => ("reasoning_delta", json!({ "text": text })),
        // 带完整记录，让订阅者拿快照起步之后光靠事件流就能维护完整状态
        AgentEvent::MessageCommitted { record } => {
            ("message_committed", json!({ "record": record }))
        }
        AgentEvent::MessageUpdated { record } => ("message_updated", json!({ "record": record })),
        AgentEvent::MessageDeleted { id } => ("message_deleted", json!({ "id": id })),
        AgentEvent::CallStarted {
            call_id,
            name,
            kind,
        } => (
            "call_started",
            json!({
                "call_id": call_id,
                "name": name,
                "kind": call_kind(*kind),
            }),
        ),
        AgentEvent::CallFinished {
            call_id,
            name,
            success,
        } => (
            "call_finished",
            json!({ "call_id": call_id, "name": name, "success": success }),
        ),
        AgentEvent::ToolBatchStarted {
            batch_id,
            message_id,
            calls,
        } => (
            "tool_batch_started",
            json!({
                "batch_id": batch_id,
                "message_id": message_id,
                "calls": calls.iter().map(|call| json!({
                    "call_id": call.call_id,
                    "name": call.name,
                    "needs_review": call.needs_review,
                })).collect::<Vec<_>>(),
            }),
        ),
        AgentEvent::AwaitHuman {
            message_id,
            batch_id,
            review_index,
            review_total,
        } => (
            "await_human",
            json!({
                "message_id": message_id,
                "batch_id": batch_id,
                "review_index": review_index,
                "review_total": review_total,
            }),
        ),
        AgentEvent::ProviderSearch {
            call_id,
            phase,
            query,
        } => (
            "provider_search",
            json!({
                "call_id": call_id,
                "phase": phase.as_str(),
                "query": query,
            }),
        ),
        AgentEvent::TurnEnd { reason } => ("turn_end", json!({ "reason": turn_reason(reason) })),
    };
    Some(Envelope::session(session_id, kind, seq, payload))
}

/// 若该 agent 事件应触发桌面通知，返回全局事件的 `(kind, payload)`。
///
/// 托盘订阅 `/api/events` 后据此调 `notify-send`；HITL 按 `message_id` 去重由
/// 消费方负责。
pub fn notify_global(
    session_id: &str,
    session_title: &str,
    event: &AgentEvent,
) -> Option<(&'static str, Value)> {
    let base = json!({
        "session_id": session_id,
        "session_title": session_title,
    });
    match event {
        AgentEvent::AwaitHuman {
            message_id,
            batch_id,
            review_index,
            review_total,
        } => Some((
            "notify_hitl",
            json!({
                "session_id": session_id,
                "session_title": session_title,
                "message_id": message_id,
                "batch_id": batch_id,
                "review_index": review_index,
                "review_total": review_total,
            }),
        )),
        AgentEvent::TurnEnd { reason } => match reason {
            TurnEndReason::Completed => Some(("notify_completed", base)),
            TurnEndReason::Failed(message) => Some((
                "notify_failed",
                json!({
                    "session_id": session_id,
                    "session_title": session_title,
                    "message": message,
                }),
            )),
            TurnEndReason::MaxRounds => Some(("notify_max_rounds", base)),
            TurnEndReason::ToolFailureLoop { count, last_tool } => Some((
                "notify_failed",
                json!({
                    "session_id": session_id,
                    "session_title": session_title,
                    "message": format!("`{last_tool}` 连续失败 {count} 次，已中止本轮"),
                }),
            )),
            TurnEndReason::AwaitingHuman
            | TurnEndReason::Cancelled
            | TurnEndReason::EmptyResponse => None,
        },
        _ => None,
    }
}

fn call_kind(kind: CallKind) -> &'static str {
    match kind {
        CallKind::Tool => "tool",
        CallKind::Action => "action",
    }
}

/// 结束原因序列化成 `{ "kind": …, "message": … }`，
/// 让客户端能按 kind 分支而不是解析文案。
fn turn_reason(reason: &TurnEndReason) -> Value {
    match reason {
        TurnEndReason::Completed => json!({ "kind": "completed" }),
        TurnEndReason::AwaitingHuman => json!({ "kind": "awaiting_human" }),
        TurnEndReason::MaxRounds => json!({ "kind": "max_rounds" }),
        TurnEndReason::ToolFailureLoop { count, last_tool } => json!({
            "kind": "tool_failure_loop",
            "count": count,
            "last_tool": last_tool,
        }),
        TurnEndReason::Cancelled => json!({ "kind": "cancelled" }),
        TurnEndReason::EmptyResponse => json!({ "kind": "empty_response" }),
        TurnEndReason::Failed(message) => json!({ "kind": "failed", "message": message }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scope_is_part_of_every_event() {
        let envelope = from_agent("abc", 1, &AgentEvent::Delta("喵".into())).unwrap();
        assert_eq!(envelope.scope, "session:abc");
        assert_eq!(envelope.kind, "message_delta");
        assert_eq!(envelope.payload["text"], "喵");
    }

    #[test]
    fn global_scope_is_reserved() {
        assert_eq!(Scope::Global.as_wire(), "global");
    }

    #[test]
    fn turn_end_reason_is_structured_not_prose() {
        let failed = from_agent(
            "abc",
            2,
            &AgentEvent::TurnEnd {
                reason: TurnEndReason::Failed("401".into()),
            },
        )
        .unwrap();
        assert_eq!(failed.payload["reason"]["kind"], "failed");
        assert_eq!(failed.payload["reason"]["message"], "401");

        let done = from_agent(
            "abc",
            3,
            &AgentEvent::TurnEnd {
                reason: TurnEndReason::Completed,
            },
        )
        .unwrap();
        assert_eq!(done.payload["reason"]["kind"], "completed");
    }

    #[test]
    fn tool_batch_started_event_shape() {
        use lya_agent::{AgentEvent, BatchCallInfo};
        let envelope = from_agent(
            "abc",
            5,
            &AgentEvent::ToolBatchStarted {
                batch_id: "b1".into(),
                message_id: 3,
                calls: vec![BatchCallInfo {
                    call_id: "c1".into(),
                    name: "bash".into(),
                    needs_review: true,
                }],
            },
        )
        .unwrap();
        assert_eq!(envelope.kind, "tool_batch_started");
        assert_eq!(envelope.payload["batch_id"], "b1");
        assert_eq!(envelope.payload["calls"][0]["needs_review"], true);
    }

    #[test]
    fn notify_global_maps_turn_end_and_hitl() {
        let hitl = notify_global(
            "s1",
            "测试",
            &AgentEvent::AwaitHuman {
                message_id: 9,
                batch_id: Some("b1".into()),
                review_index: Some(1),
                review_total: Some(2),
            },
        )
        .unwrap();
        assert_eq!(hitl.0, "notify_hitl");
        assert_eq!(hitl.1["message_id"], 9);

        let done = notify_global(
            "s1",
            "测试",
            &AgentEvent::TurnEnd {
                reason: TurnEndReason::Completed,
            },
        )
        .unwrap();
        assert_eq!(done.0, "notify_completed");

        assert!(notify_global(
            "s1",
            "测试",
            &AgentEvent::TurnEnd {
                reason: TurnEndReason::AwaitingHuman,
            },
        )
        .is_none());
    }

    #[test]
    fn provider_search_event_maps_to_sse() {
        use lya_agent::{AgentEvent, ProviderSearchPhase};
        let envelope = from_agent(
            "abc",
            6,
            &AgentEvent::ProviderSearch {
                call_id: "ws1".into(),
                phase: ProviderSearchPhase::Searching,
                query: Some("天气".into()),
            },
        )
        .unwrap();
        assert_eq!(envelope.kind, "provider_search");
        assert_eq!(envelope.payload["call_id"], "ws1");
        assert_eq!(envelope.payload["phase"], "searching");
        assert_eq!(envelope.payload["query"], "天气");
    }

    #[test]
    fn serializes_with_type_field() {
        let envelope = Envelope::session("s", "ping", 7, json!({}));
        let text = serde_json::to_string(&envelope).unwrap();
        assert!(text.contains(r#""type":"ping""#));
        assert!(text.contains(r#""scope":"session:s""#));
        assert!(text.contains(r#""seq":7"#));
    }
}
