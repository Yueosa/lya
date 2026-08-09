//! 分块计数。

use std::sync::LazyLock;

use lya_llm::{ChatMessage, Role};
use serde::Serialize;
use serde_json::Value;
use tokenizers::Tokenizer;

static TOKENIZER: LazyLock<Tokenizer> = LazyLock::new(|| {
    Tokenizer::from_bytes(include_bytes!("../assets/deepseek_v4/tokenizer.json"))
        .expect("deepseek_v4 tokenizer.json 无效")
});

/// 一项占用分类。
#[derive(Debug, Clone, Serialize)]
pub struct UsageCategory {
    /// 分类 id。
    pub id: String,
    /// 展示名。
    pub label: String,
    /// token 数（估算）。
    pub tokens: u64,
    /// 是否计入上下文上限；`false` 表示仅落库、未进 wire。
    #[serde(default = "default_in_context")]
    pub in_context: bool,
}

#[allow(dead_code)]
fn default_in_context() -> bool {
    true
}

/// 上下文占用汇总。
#[derive(Debug, Clone, Serialize)]
pub struct ContextUsageReport {
    /// 词表 id。
    pub tokenizer_id: String,
    /// 合计 token（各分类之和）。
    pub total: u64,
    /// 上下文上限（来自 models.toml context_window）。
    pub limit: u64,
    /// 占用百分比 0–100。
    pub pct: f32,
    /// 分块。
    pub categories: Vec<UsageCategory>,
}

/// 对一段文本计 token。
pub fn count_text(text: &str) -> u64 {
    if text.is_empty() {
        return 0;
    }
    TOKENIZER
        .encode(text, false)
        .map(|encoding| encoding.get_ids().len() as u64)
        .unwrap_or(0)
}

/// 工具 / 动作 schema JSON 的计数字符串。
pub fn serialize_tool_schemas(schemas: &[Value]) -> String {
    serde_json::to_string(schemas).unwrap_or_default()
}

/// 对话消息（不含 system）拼成计数字符串。
pub fn serialize_messages_for_count(messages: &[ChatMessage]) -> String {
    let mut out = String::new();
    for message in messages {
        if message.role == Role::System {
            continue;
        }
        out.push_str(message.role.as_str());
        out.push('\n');
        out.push_str(&message.content);
        if let Some(calls) = &message.tool_calls {
            for call in calls {
                out.push_str("\n$tool$");
                out.push_str(&call.name);
                out.push('\n');
                out.push_str(&call.arguments);
            }
        }
        if let Some(id) = &message.tool_call_id {
            out.push_str("\n$tool_id$");
            out.push_str(id);
        }
        out.push('\n');
    }
    out
}

/// Responses `input` items 的计数字符串。
pub fn serialize_responses_input(input: &[Value]) -> String {
    serde_json::to_string(input).unwrap_or_default()
}

/// 组装报告。
pub fn build_report(limit: u64, categories: Vec<UsageCategory>) -> ContextUsageReport {
    let total: u64 = categories
        .iter()
        .filter(|item| item.in_context)
        .map(|item| item.tokens)
        .sum();
    let pct = if limit > 0 {
        ((total as f64 / limit as f64) * 1000.0).round() as f32 / 10.0
    } else {
        0.0
    };
    ContextUsageReport {
        tokenizer_id: super::TOKENIZER_ID.into(),
        total,
        limit,
        pct,
        categories,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn count_is_nonzero_for_text() {
        assert!(count_text("Hello 你好") > 0);
    }

    #[test]
    fn build_report_sums_categories() {
        let report = build_report(
            1000,
            vec![
                UsageCategory {
                    id: "a".into(),
                    label: "A".into(),
                    tokens: 100,
                    in_context: true,
                },
                UsageCategory {
                    id: "b".into(),
                    label: "B".into(),
                    tokens: 50,
                    in_context: true,
                },
            ],
        );
        assert_eq!(report.total, 150);
        assert_eq!(report.pct, 15.0);
    }
}
