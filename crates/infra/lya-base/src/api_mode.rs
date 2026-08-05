//! LLM 调用栈与能力键。
//!
//! 这两样是**配置文件与请求体之间的合约**：`models.toml` 里写 `modes.responses`、
//! `capabilities = ["vision"]`，`lya-llm` 拼请求时要按同样的字符串去查。所以
//! `lya-config` 和 `lya-llm` 都得认，而它们互不依赖——之前的结果是两边各写了一份
//! 逐字相同的定义。两份定义只要有一次改得不一样，配置里写的和请求里发的就对不上，
//! 编译器一声不响。

use serde::{Deserialize, Serialize};

/// 文本生成。
pub const CAPABILITY_TEXT: &str = "text";
/// 原生看图。
pub const CAPABILITY_VISION: &str = "vision";
/// Responses 原生联网（provider 侧 `web_search`）。
pub const CAPABILITY_WEB_SEARCH: &str = "web_search";

/// LLM 调用栈。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ApiMode {
    /// OpenAI Chat Completions（`/chat/completions`）。
    Completions,
    /// OpenAI Responses API（`/responses`）。
    Responses,
}

impl ApiMode {
    /// 配置 / 数据库里的字符串键。
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Completions => "completions",
            Self::Responses => "responses",
        }
    }

    /// 解析；非法值返回 `None`。
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_lowercase().as_str() {
            "completions" => Some(Self::Completions),
            "responses" => Some(Self::Responses),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_through_string_and_json() {
        for mode in [ApiMode::Completions, ApiMode::Responses] {
            assert_eq!(ApiMode::parse(mode.as_str()), Some(mode));
            // serde 的 rename_all 必须和 as_str 一致：库里存的是 as_str 的结果，
            // 而配置是 serde 读的，两者错开就会「配置能存进去但读不出来」
            let json = serde_json::to_string(&mode).unwrap();
            assert_eq!(json, format!("\"{}\"", mode.as_str()));
        }
        assert_eq!(ApiMode::parse(" RESPONSES "), Some(ApiMode::Responses));
        assert_eq!(ApiMode::parse("chat"), None);
    }
}
