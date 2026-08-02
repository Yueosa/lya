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
    /// shell 命令的确认策略。
    pub shell: ShellSettings,
    /// 媒体缓存与 serving（`[media.*]`）。
    pub media: MediaSettings,
}

/// 什么时候要用户确认 shell 命令。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ShellConfirm {
    /// 每条命令都确认。
    Always,
    /// 已知只读的命令直接放行，其余都确认。
    ///
    /// 默认档。黑名单永远列不全，白名单漏了顶多多问一次。
    #[default]
    Unknown,
    /// 只有命中风险规则才确认。打断少，但漏网的多。
    Risky,
}

/// shell 相关设置。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ShellSettings {
    /// 确认策略。
    pub confirm: ShellConfirm,
}

/// agent 循环设置。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AgentSettings {
    /// 单次用户输入内，LLM 与工具/动作最多来回几轮；防死循环。
    pub max_tool_rounds: u32,
    /// 同一条 assistant 消息里 `tool_calls` 数量上限；超出则整组失败回灌。
    pub max_parallel_tools: u32,
    /// 新会话的默认工作模式。
    pub default_work_mode: Mode,
    /// 默认模型 id，指向 `models.toml` 里的某一条。
    pub default_model: Option<String>,
}

impl Default for AgentSettings {
    fn default() -> Self {
        Self {
            max_tool_rounds: 32,
            max_parallel_tools: 3,
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

/// `runtime.toml` 的 `[media.*]` 根表。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MediaSettings {
    /// 图片（`img_cache`）。
    pub image: ImageMediaSettings,
    /// 视频（`vdo_cache`）。
    pub video: VideoMediaSettings,
    /// 音频（`ado_cache`）。
    pub audio: AudioMediaSettings,
}

/// 图片缓存与大小限制。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ImageMediaSettings {
    /// 单张图片大小上限（字节）；`/api/local-image` 与会话 media 端点共用。
    pub max_bytes: u64,
    /// 是否写入 `img_cache/local`。
    pub cache_local: bool,
    /// 是否下载并写入 `img_cache/web`。
    pub cache_web: bool,
}

impl Default for ImageMediaSettings {
    fn default() -> Self {
        Self {
            max_bytes: 32 * 1024 * 1024,
            cache_local: true,
            cache_web: true,
        }
    }
}

/// 视频缓存与大小限制（`vdo_cache`）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct VideoMediaSettings {
    /// 单个视频大小上限（字节）。
    pub max_bytes: u64,
    /// 是否写入 `vdo_cache/local`。
    pub cache_local: bool,
    /// 是否下载并写入 `vdo_cache/web`。
    pub cache_web: bool,
}

impl Default for VideoMediaSettings {
    fn default() -> Self {
        Self {
            max_bytes: 512 * 1024 * 1024,
            cache_local: true,
            cache_web: true,
        }
    }
}

/// 音频缓存与大小限制（`ado_cache`）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioMediaSettings {
    /// 单个音频大小上限（字节）。
    pub max_bytes: u64,
    /// 是否写入 `ado_cache/local`。
    pub cache_local: bool,
    /// 是否下载并写入 `ado_cache/web`。
    pub cache_web: bool,
}

impl Default for AudioMediaSettings {
    fn default() -> Self {
        Self {
            max_bytes: 128 * 1024 * 1024,
            cache_local: true,
            cache_web: true,
        }
    }
}
