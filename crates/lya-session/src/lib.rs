//! # lya-session
//!
//! 会话与消息树（SQLite 持久化）。
//!
//! - 会话元数据：`work_mode`、`enabled_tools`、`persona`、`active_leaf_id`
//! - 消息树：`append` / `switch_leaf` / `fork_at` / `delete_leaf` / 路径遍历
//! - HITL 独立节点（`role=hitl`）与 pending 查询
//! - `message_json`：自研外壳 + 内嵌 OpenAI 结构
//!
//! ## 分支约定
//!
//! - 当前路径由 `sessions.active_leaf_id` 决定
//! - `switch_leaf` / `fork_at` 只改指针；编辑重发 = `fork_at` 后再 `append`
//! - **只允许删除没有子节点的节点**（叶节点）

#![deny(missing_docs)]

pub mod error;
pub mod message;
pub mod store;
pub mod types;

pub use error::SessionError;
pub use lya_db::Migration;
pub use message::{
    ConfirmStepBlock, FormOption, FormQuestion, FormQuestionKind, HitlBlock, HitlKind, LyaExtras,
    MessageKind, MessagePayload, MessageRole, MessageStatus, OpenAiFunction, OpenAiMessage,
    OpenAiToolCall,
};
pub use store::SessionStore;
pub use types::{CreateSession, MessageRecord, SessionMeta, SessionStatus};

/// 会话相关表的迁移序列。
pub const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        sql: include_str!("../migrations/001_init.sql"),
    },
    Migration {
        version: 2,
        sql: include_str!("../migrations/002_model_id.sql"),
    },
];

/// 迁移台账里用的归属名。
pub const MIGRATION_SCOPE: &str = "session";
