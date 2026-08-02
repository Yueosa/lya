//! 把消息树的一条路径装配成发给 API 的 `messages` 数组。
//!
//! 这里是「持久化的领域消息」与「wire 消息」之间唯一的转换点，几条规则：
//!
//! - **HITL 节点跳过**：它的 `openai` 本来就是 `None`，只服务界面与状态恢复
//! - **思考不回灌**：reasoning 落库只为展示，多数 API 也不接受回传
//! - **被中断的助手消息带标注**：让模型知道自己上次说到一半被打断了，
//!   否则它会假定那句话完整说完了
//! - **孤儿 tool_call 必须补**：崩溃可能留下「发了 tool_calls 但结果没写回」
//!   的状态，直接发出去 API 会拒绝——每个 `tool_call_id` 都必须有对应的
//!   tool 消息
//! - **user / tool 消息带时间戳前缀**：模型靠它知道现在几点
//!
//! ## 时间戳为什么加在这里而不是系统提示词里
//!
//! 系统提示词是缓存前缀的最前面，往里塞当前时间等于每轮都换一个前缀，
//! API 商的缓存全量失效。加在消息前缀上则相反：时间戳取自消息**不可变的
//! 创建时间**，同一条历史消息每次渲染的结果完全一样，新消息只是往尾部追加。
//! 系统提示词里只放一段静态说明（`lya_prompt::TIME_ANCHOR`）解释前缀含义。

use chrono::{DateTime, Duration, Local, Utc};
use lya_llm::{ChatMessage, ToolCall};
use lya_session::{MessageRecord, MessageRole, MessageStatus, OpenAiMessage};

/// 助手消息被中断时追加的标注。
pub const INTERRUPTED_MARK: &str = "（此处被中断）";

/// 补给孤儿 tool_call 的占位结果。
pub const MISSING_RESULT: &str = "（执行被中断，没有结果）";

/// 间隔达到多久才提示「距上一条消息 …」。
const GAP_HINT_THRESHOLD: Duration = Duration::minutes(30);

/// 装配一次请求的完整 `messages`。
///
/// `path` 需为根到当前叶的时间正序，即 `SessionStore::path_to_active_leaf`
/// 的输出。
pub fn build_messages(system_prompt: &str, path: &[MessageRecord]) -> Vec<ChatMessage> {
    let mut out = Vec::with_capacity(path.len() + 1);
    out.push(ChatMessage::system(system_prompt));

    let answered = answered_call_ids(path);
    let mut previous_at: Option<DateTime<Utc>> = None;

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
                let message = assistant_message(openai, payload.status);
                let calls = message.tool_calls.clone().unwrap_or_default();
                out.push(message);
                // 紧跟着补上缺失的结果，保证 assistant 之后每个 call 都有交代
                for call in calls {
                    if !answered.contains(&call.id) {
                        out.push(ChatMessage::tool_result(call.id, MISSING_RESULT));
                    }
                }
            }
            MessageRole::Tool => {
                let Some(call_id) = &openai.tool_call_id else {
                    continue;
                };
                out.push(ChatMessage::tool_result(
                    call_id,
                    format!("{stamp}{}", openai.content),
                ));
            }
            MessageRole::User => out.push(ChatMessage::user(format!("{stamp}{}", openai.content))),
            // 系统节点（如「用户切换了模式」）不加时间戳：它是系统在说话，
            // 不是对话的一拍。模式变更历史不进上下文——当前模式已在 system prompt 里。
            MessageRole::System => {
                if openai.content.starts_with("[模式变更]") {
                    continue;
                }
                out.push(ChatMessage::system(&openai.content));
            }
            MessageRole::Hitl => unreachable!("已在上面跳过"),
        }
    }

    out
}

/// 路径里已经拿到结果的 `tool_call_id` 集合。
fn answered_call_ids(path: &[MessageRecord]) -> Vec<String> {
    path.iter()
        .filter(|record| record.payload.role == MessageRole::Tool)
        .filter_map(|record| record.payload.openai.as_ref())
        .filter_map(|openai| openai.tool_call_id.clone())
        .collect()
}

/// 拼出 `[2026-04-26 14:23 +08]` 形式的时间前缀，必要时追加节奏提示。
///
/// 两个提示都只依赖相邻两条消息的创建时间，因此渲染结果是确定的。
fn time_prefix(at: DateTime<Utc>, previous: Option<DateTime<Utc>>) -> String {
    let local = at.with_timezone(&Local);
    let mut prefix = format!(
        "[{} {}]",
        local.format("%Y-%m-%d %H:%M"),
        offset_label(local)
    );

    if let Some(previous) = previous {
        let gap = at - previous;
        if gap >= GAP_HINT_THRESHOLD {
            prefix.push_str(&format!("（距上一条消息 {}）", format_gap(gap)));
        }
        let previous_local = previous.with_timezone(&Local);
        if previous_local.date_naive() != local.date_naive() {
            prefix.push_str(&format!("（日期已变更：{}）", local.format("%Y-%m-%d")));
        }
    }

    prefix.push(' ');
    prefix
}

/// 时区偏移：整点写 `+08`，带分钟写 `+08:30`。
fn offset_label(local: DateTime<Local>) -> String {
    let seconds = local.offset().local_minus_utc();
    let sign = if seconds < 0 { '-' } else { '+' };
    let total = seconds.abs() / 60;
    let (hours, minutes) = (total / 60, total % 60);
    if minutes == 0 {
        format!("{sign}{hours:02}")
    } else {
        format!("{sign}{hours:02}:{minutes:02}")
    }
}

/// 把间隔说成人话。
fn format_gap(gap: Duration) -> String {
    let minutes = gap.num_minutes();
    if minutes >= 60 * 24 {
        format!("{} 天", minutes / (60 * 24))
    } else if minutes >= 60 {
        format!("{} 小时", minutes / 60)
    } else if minutes >= 1 {
        format!("{minutes} 分钟")
    } else {
        "不到 1 分钟".into()
    }
}

/// 助手消息：带上 tool_calls，中断时追加标注。
fn assistant_message(openai: &OpenAiMessage, status: MessageStatus) -> ChatMessage {
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

    match &openai.tool_calls {
        Some(calls) if !calls.is_empty() => {
            let calls = calls
                .iter()
                .map(|call| ToolCall {
                    id: call.id.clone(),
                    name: call.function.name.clone(),
                    arguments: call.function.arguments.clone(),
                })
                .collect();
            ChatMessage::assistant_tool_calls(content, calls)
        }
        _ => ChatMessage::assistant(content),
    }
}

#[cfg(test)]
mod tests {
    use lya_llm::Role;
    use lya_session::{
        HitlBlock, LyaExtras, MessageKind, MessagePayload, OpenAiFunction, OpenAiToolCall,
    };

    use super::*;

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

    fn assistant_with_call(id: i64, call_id: &str) -> MessageRecord {
        let mut payload = MessagePayload::assistant_text("我查一下", MessageStatus::Complete);
        payload.kind = MessageKind::ToolCall;
        payload.openai.as_mut().unwrap().tool_calls = Some(vec![OpenAiToolCall {
            id: call_id.into(),
            kind: "function".into(),
            function: OpenAiFunction {
                name: "file_read".into(),
                arguments: "{}".into(),
            },
        }]);
        record(id, payload)
    }

    #[test]
    fn maps_plain_conversation() {
        let path = vec![
            record(1, MessagePayload::user_text("你好")),
            record(
                2,
                MessagePayload::assistant_text("你好喵~", MessageStatus::Complete),
            ),
        ];
        let messages = build_messages("SYSTEM", &path);

        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0].role, Role::System);
        assert_eq!(messages[0].content, "SYSTEM");
        assert_eq!(messages[1].role, Role::User);
        assert_eq!(messages[2].content, "你好喵~");
        assert!(messages[2].tool_calls.is_none());
    }

    #[test]
    fn skips_hitl_nodes() {
        let path = vec![
            record(1, MessagePayload::user_text("删这个")),
            record(
                2,
                MessagePayload::hitl_pending(
                    MessageKind::ToolConfirm,
                    HitlBlock::ModeChange {
                        to_mode: "edit".into(),
                        reason: "要改文件".into(),
                    },
                ),
            ),
        ];
        let messages = build_messages("S", &path);
        assert_eq!(messages.len(), 2, "HITL 节点不该进模型上下文");
    }

    #[test]
    fn pairs_tool_call_with_its_result() {
        let path = vec![
            record(1, MessagePayload::user_text("看看配置")),
            assistant_with_call(2, "call_1"),
            record(3, MessagePayload::tool_result("call_1", "文件内容")),
        ];
        let messages = build_messages("S", &path);

        assert_eq!(messages.len(), 4);
        assert_eq!(messages[2].role, Role::Assistant);
        assert_eq!(messages[2].tool_calls.as_ref().unwrap().len(), 1);
        assert_eq!(messages[3].role, Role::Tool);
        assert_eq!(messages[3].tool_call_id.as_deref(), Some("call_1"));
        assert!(messages[3].content.ends_with("文件内容"));
    }

    #[test]
    fn synthesizes_result_for_orphan_tool_call() {
        // 崩溃留下的状态：发了 tool_calls，结果没写回
        let path = vec![
            record(1, MessagePayload::user_text("看看配置")),
            assistant_with_call(2, "call_1"),
        ];
        let messages = build_messages("S", &path);

        assert_eq!(messages.len(), 4, "缺的结果要补上，否则 API 会拒绝");
        assert_eq!(messages[3].role, Role::Tool);
        assert_eq!(messages[3].tool_call_id.as_deref(), Some("call_1"));
        assert_eq!(messages[3].content, MISSING_RESULT);
    }

    #[test]
    fn synthesizes_only_the_missing_one() {
        let mut payload = MessagePayload::assistant_text("", MessageStatus::Complete);
        payload.kind = MessageKind::ToolCall;
        payload.openai.as_mut().unwrap().tool_calls = Some(vec![
            OpenAiToolCall {
                id: "a".into(),
                kind: "function".into(),
                function: OpenAiFunction {
                    name: "t".into(),
                    arguments: "{}".into(),
                },
            },
            OpenAiToolCall {
                id: "b".into(),
                kind: "function".into(),
                function: OpenAiFunction {
                    name: "t".into(),
                    arguments: "{}".into(),
                },
            },
        ]);
        let path = vec![
            record(1, payload),
            record(2, MessagePayload::tool_result("a", "有结果")),
        ];
        let messages = build_messages("S", &path);

        let tools: Vec<_> = messages.iter().filter(|m| m.role == Role::Tool).collect();
        assert_eq!(tools.len(), 2);
        assert!(tools.iter().any(|m| m.content.ends_with("有结果")));
        assert!(tools.iter().any(|m| m.content == MISSING_RESULT));
    }

    #[test]
    fn marks_interrupted_assistant() {
        let path = vec![record(
            1,
            MessagePayload::assistant_text("我正要说", MessageStatus::Interrupted),
        )];
        let messages = build_messages("S", &path);
        assert_eq!(messages[1].content, format!("我正要说\n{INTERRUPTED_MARK}"));
    }

    #[test]
    fn marks_streaming_leftover_as_interrupted() {
        // 进程崩溃留下的 Streaming 残留，等同于中断
        let path = vec![record(
            1,
            MessagePayload::assistant_text("", MessageStatus::Streaming),
        )];
        let messages = build_messages("S", &path);
        assert_eq!(messages[1].content, INTERRUPTED_MARK);
    }

    /// 造一条指定时刻的用户消息。
    fn user_at(id: i64, text: &str, at: DateTime<Utc>) -> MessageRecord {
        let mut record = record(id, MessagePayload::user_text(text));
        record.created_at = at;
        record
    }

    fn at(text: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(text)
            .unwrap()
            .with_timezone(&Utc)
    }

    #[test]
    fn user_and_tool_messages_carry_a_timestamp() {
        let path = vec![
            user_at(1, "你好", at("2026-04-26T06:23:00Z")),
            record(
                2,
                MessagePayload::assistant_text("你好喵~", MessageStatus::Complete),
            ),
        ];
        let messages = build_messages("S", &path);

        // 前缀格式 [YYYY-MM-DD HH:MM ±TZ]，本机时区
        assert!(
            messages[1].content.starts_with('[') && messages[1].content.ends_with("你好"),
            "实际是 {:?}",
            messages[1].content
        );
        assert!(messages[1].content.contains("2026-04-26"));
        // 助手消息不带前缀
        assert_eq!(messages[2].content, "你好喵~");
    }

    #[test]
    fn long_gap_and_date_change_are_hinted() {
        let path = vec![
            user_at(1, "晚安", at("2026-04-26T15:00:00Z")),
            user_at(2, "早", at("2026-04-27T02:00:00Z")),
        ];
        let messages = build_messages("S", &path);

        assert!(
            !messages[1].content.contains("距上一条"),
            "第一条没有上一条"
        );
        assert!(messages[2].content.contains("（距上一条消息 11 小时）"));
        assert!(messages[2].content.contains("（日期已变更："));
    }

    #[test]
    fn short_gap_is_not_hinted() {
        let path = vec![
            user_at(1, "一", at("2026-04-26T06:00:00Z")),
            user_at(2, "二", at("2026-04-26T06:10:00Z")),
        ];
        let messages = build_messages("S", &path);
        assert!(!messages[2].content.contains("距上一条"));
        assert!(!messages[2].content.contains("日期已变更"));
    }

    #[test]
    fn timestamps_are_deterministic() {
        // 同一批记录渲染两次必须完全一致，否则前缀缓存会每轮失效
        let path = vec![
            user_at(1, "一", at("2026-04-26T06:00:00Z")),
            user_at(2, "二", at("2026-04-27T09:00:00Z")),
        ];
        assert_eq!(build_messages("S", &path), build_messages("S", &path));
    }

    #[test]
    fn drops_reasoning() {
        let mut payload = MessagePayload::assistant_text("答案", MessageStatus::Complete);
        payload.lya = LyaExtras {
            reasoning: Some("很长的思考过程".into()),
            ..Default::default()
        };
        let messages = build_messages("S", &[record(1, payload)]);
        assert_eq!(messages[1].content, "答案");
        assert!(!messages[1].content.contains("思考"));
    }
}
