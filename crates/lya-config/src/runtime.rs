//! 运行时默认值（`runtime.toml`）：重新加载即生效的那些。
//!
//! 这一层只提供**默认**。会话自己设过的字段（工作模式、启用工具、人设）存在
//! `sessions` 表里，以会话为准；本文件不重复存储它们。

use lya_mode::Mode;
use serde::{Deserialize, Serialize};

/// `runtime.toml` 的内容。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct RuntimeConfig {
    /// agent 循环相关。
    pub agent: AgentSettings,
    /// 工具默认启用范围。
    pub tools: ToolSettings,
    /// 记忆索引体积。
    pub memory: MemorySettings,
}

/// agent 循环设置。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AgentSettings {
    /// 单次用户输入内，LLM 与工具/动作最多来回几轮；防死循环。
    pub max_tool_rounds: u32,
    /// 新会话的默认工作模式。
    pub default_work_mode: Mode,
    /// 默认模型 id，指向 `models.toml` 里的某一条。
    pub default_model: Option<String>,
}

impl Default for AgentSettings {
    fn default() -> Self {
        Self {
            max_tool_rounds: 32,
            // 与 sessions 表的默认值保持一致；Mode::default() 是 Ask，
            // 那个默认服务于「凭空造一个 Mode」，不适合当新会话默认
            default_work_mode: Mode::Agent,
            default_model: None,
        }
    }
}

/// 工具默认启用范围。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ToolSettings {
    /// 新会话默认启用的工具内部名。
    ///
    /// `None`（键缺省）= 启用全部；`Some(list)` = 只启用列出的；
    /// `Some(vec![])` = 一个都不启用。正好对上
    /// [`lya_tool::ToolRegistry::bundle`] 的 `names` 语义，不必额外发明
    /// 「all」这种特殊值。
    pub enabled: Option<Vec<String>>,
}

/// 记忆索引体积上限。
///
/// 对应 `lya_memory::IndexBudget`；同样为了不依赖 `lya-memory`（会拖进
/// rusqlite）而存成朴素数值，由装配方映射。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MemorySettings {
    /// 索引最多列出几条。
    pub max_index_entries: usize,
    /// 索引整段最多多少字符。
    pub max_index_chars: usize,
    /// 索引里每条摘要截断到多少字符。
    pub index_summary_chars: usize,
}

impl Default for MemorySettings {
    fn default() -> Self {
        Self {
            max_index_entries: 100,
            max_index_chars: 4000,
            index_summary_chars: 120,
        }
    }
}
