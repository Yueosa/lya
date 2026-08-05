//! # lya-tool
//!
//! 工具定义与注册中心。
//!
//! ## 设计目标
//!
//! - **工具自包含**：实现、JSON Schema、用法提示词写在同一工具里，
//!   不另搞 skills 体系。
//! - **注册中心只做目录 + 筛选导出**：按名字列表与 RWX 权限上限过滤，
//!   返回提示词段 + OpenAI `tools[]` schema。
//! - **会话层后接**：启用/禁用、ask/edit/agent 模式通过传入不同
//!   `names` / `permission` 完成，本 crate 不持有 session 状态。
//!
//! ## 明确不做什么
//!
//! - 不解析 LLM 响应里的 `tool_calls`（那是 `lya-llm`）
//! - 不做 HITL / 鉴权 UI（钩子以后可写在各工具的 [`Tool::call`] 内）
//!
//! ## 模块结构
//!
//! - [`meta`] — [`ToolMeta`] / [`ToolResult`]
//! - [`traits`] — [`Tool`] trait
//! - [`registry`] — [`ToolRegistry`] / [`ToolBundle`]
//! - [`error`] — [`ToolError`]
//! - [`confirm`] — 执行前的用户确认请求（判断与执行分两步）
//! - [`context`] — [`ToolCtx`]，带取消信号
//! - [`limits`] — 各工具的上限与默认值，**不走配置文件**
//! - [`tools`] — 内置工具（`file_read` / `bash` / `web_search` 等）

#![deny(missing_docs)]

pub mod confirm;
pub mod context;
pub mod error;
pub mod limits;
pub mod meta;
pub mod registry;
pub mod traits;
pub mod tools;

pub use confirm::{ConfirmRequest, ConfirmStep};
pub use context::{CancelToken, ToolCtx};
pub use error::ToolError;
pub use meta::{ToolMeta, ToolResult};
// 权限住在 lya-base：模式要把自己映射成权限上限，而模式在工具层之下。
// 这里转出来，是为了让工具的调用方不必为一个类型多写一条依赖
pub use lya_base::Permission;
pub use registry::{openai_function_schema, openai_tool_schema, ToolBundle, ToolRegistry};
pub use tools::register_builtins;
pub use traits::Tool;
