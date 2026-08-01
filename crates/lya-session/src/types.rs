//! 会话元数据与消息行。

use chrono::{DateTime, Utc};
use lya_mode::Mode;
use serde::{Deserialize, Serialize};

use crate::message::MessagePayload;

/// 会话状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SessionStatus {
    /// 活跃。
    Active,
    /// 已归档。
    Archived,
}

impl SessionStatus {
    /// 存库字符串。
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Archived => "archived",
        }
    }

    /// 解析。
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "active" => Some(Self::Active),
            "archived" => Some(Self::Archived),
            _ => None,
        }
    }
}

/// 创建会话参数。
#[derive(Debug, Clone, Default)]
pub struct CreateSession {
    /// 标题；默认空。
    pub title: String,
    /// 初始模式；默认 agent。
    pub work_mode: Mode,
    /// 会话人设覆盖；`None` 表示用全局。
    pub persona: Option<String>,
    /// 使用哪个模型；`None` 表示用配置里的默认模型。
    pub model_id: Option<String>,
    /// 启用的工具内部名。
    ///
    /// `None` = 启用全部；`Some(list)` = 只启用列出的；`Some(vec![])` = 全禁。
    pub enabled_tools: Option<Vec<String>>,
}

/// 会话元数据快照。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionMeta {
    /// 会话 id（uuid）。
    pub id: String,
    /// 标题。
    pub title: String,
    /// 状态。
    pub status: SessionStatus,
    /// 当前分支叶节点；空会话为 `None`。
    pub active_leaf_id: Option<i64>,
    /// 工作模式。
    pub work_mode: Mode,
    /// 会话人设。
    pub persona: Option<String>,
    /// 使用哪个模型；`None` 表示用配置里的默认模型。
    pub model_id: Option<String>,
    /// 用户启用的工具名；`None` 表示全部启用。
    pub enabled_tools: Option<Vec<String>>,
    /// 创建时间。
    pub created_at: DateTime<Utc>,
    /// 更新时间。
    pub updated_at: DateTime<Utc>,
}

/// 持久化后的消息行。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MessageRecord {
    /// 数据库 id。
    pub id: i64,
    /// 所属会话。
    pub session_id: String,
    /// 父节点；根为 `None`。
    pub parent_id: Option<i64>,
    /// 会话内递增序号。
    pub sort_key: i64,
    /// 自研 message JSON。
    pub payload: MessagePayload,
    /// 创建时间。
    pub created_at: DateTime<Utc>,
}
