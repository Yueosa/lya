//! # lya-memory
//!
//! 跨会话的长期记忆：显式笔记的仓储 + 常驻索引渲染。
//!
//! ## 职责
//!
//! - 记忆的增删改查，正文直接存 SQLite
//! - 标签规整（去重排序）与写入长度校验
//! - 把全部记忆渲染成一段索引提示词，供 `lya-prompt` 注入
//!
//! ## 非职责
//!
//! - 不定义暴露给模型的 action/tool，也不写「什么时候该记」的指导文案，
//!   那些属于 `lya-action`
//! - 不做向量检索
//!
//! ## 为什么是「常驻索引」而不是检索
//!
//! 上一代实现（lianclaw）建了 embedding 表和 hybrid 检索，实际三个月只攒了
//! 8 条记忆、正文合计 8.7 KB，embedding 一次都没启用过。这个量级下把索引
//! （标题 + 标签 + 摘要）整个放进 system prompt 比任何检索都准：模型看得见
//! 全部条目，要正文再按编号读，没有漏检。
//!
//! 索引会随条数增长，所以有 [`IndexBudget`] 兜底——超预算就只留最近更新的
//! 若干条并说明还有多少没列出。真撑爆那天再加检索，那时也已经知道真实的
//! 查询模式了。
//!
//! ## 与上一代的差异
//!
//! - **没有分类/namespace**：旧实现给了 5 个建议分类加一个 `general` 兜底，
//!   结果 8 条里 5 条躺在 `general`。有兜底选项模型就一直选兜底，索性只留标签。
//! - **正文入库**：旧实现 SQLite 存索引、Markdown 文件存正文，一致性要自己
//!   维护。这点数据量不值得拆两处。
//! - **自增 id 而非标题主键**：旧实现拿标题当主键，改名就得删旧建新。

#![deny(missing_docs)]

pub mod error;
pub mod index;
pub mod store;
pub mod types;

pub use error::MemoryError;
pub use index::{IndexBudget, MEMORY_SECTION_TITLE, render_index};
pub use store::MemoryStore;
pub use types::{MatchField, Memory, MemoryHit, MemoryLimits, MemoryPatch, NewMemory};

/// 记忆相关表迁移 SQL。
pub const MIGRATION_SQL: &str = include_str!("../migrations/001_init.sql");
