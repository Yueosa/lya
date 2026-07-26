//! # lya-mode
//!
//! lya 的工作模式模块，定义 `ask` / `edit` / `agent` 三种模式。
//!
//! ## 职责
//!
//! - 把模式映射到 RWX 权限上限
//! - 提供当前模式的 system prompt 段
//! - 将会话传入的启用工具名交给 [`lya_tool::ToolRegistry`]，再叠加权限过滤
//! - 返回同一筛选结果生成的提示词与 OpenAI `tools[]` schema
//!
//! ## 不负责
//!
//! - 不持有 session，也不持久化当前模式
//! - 不直接调用 `lya-prompt`；上层把 [`ModeBundle::mode_prompt`] 与
//!   [`ModeBundle::tools`] 传给提示词及 LLM
//! - 不执行工具，不决定 action 集合
//!
//! 运行时调用链为：
//!
//! ```text
//! session(mode, enabled_tools)
//!   → lya-mode::resolve
//!   → ToolRegistry::bundle(names, permission)
//!   → mode prompt + tool prompt + tool schemas
//!   → 上层交给 lya-prompt / lya-llm
//! ```

#![deny(missing_docs)]

mod error;
mod mode;

pub use error::ModeParseError;
pub use mode::{Mode, ModeBundle};
