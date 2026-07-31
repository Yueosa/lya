//! # lya-session
//!
//! 会话与消息树（SQLite 持久化）。
//!
//! ## 职责
//!
//! - 会话元数据：`work_mode`、`enabled_tools`、`persona`、`active_leaf_id`
//! - 消息树：`append` / `switch_leaf` / `fork_at` / `delete_leaf` / 路径遍历
//! - HITL 独立节点（`role=hitl`）与 pending 查询
//! - `message_json`：自研外壳 + 内嵌 OpenAI 结构
//!
//! ## 非职责
//!
//! - 不按 RWX 筛选工具（交给 [`lya_mode::Mode::resolve`]）
//! - 不拼 prompt、不调 LLM、不执行 tool
//! - 底层连接/迁移基建在 [`lya_db`]
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
pub use message::{
    HitlBlock, HitlKind, LyaExtras, MessageKind, MessagePayload, MessageRole, MessageStatus,
    OpenAiMessage, OpenAiToolCall,
};
pub use store::SessionStore;
pub use types::{CreateSession, MessageRecord, SessionMeta, SessionStatus};

/// 会话相关表迁移 SQL。
pub const MIGRATION_SQL: &str = include_str!("../migrations/001_init.sql");
