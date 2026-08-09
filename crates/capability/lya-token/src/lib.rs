//! DeepSeek V4 token 估算（离线、只读）。
//!
//! 用 HF 发布的 V4 BPE 词表对**将要发给 API 的文本**做 encode 长度统计。
//! 与线上下单时的 `usage` 会有偏差（消息编码格式、特殊 token 等），UI 标注为估算。

#![deny(missing_docs)]

mod count;

pub use count::{
    ContextUsageReport, UsageCategory, build_report, count_text, serialize_messages_for_count,
    serialize_responses_input, serialize_tool_schemas,
};

/// 当前 bundled 词表 id。
pub const TOKENIZER_ID: &str = "deepseek_v4";
