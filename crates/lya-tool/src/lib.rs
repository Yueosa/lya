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
//! - 不实现具体业务工具（见 [`tools`] 占位模块）
//! - 不解析 LLM 响应里的 `tool_calls`（那是 `lya-llm`）
//! - 不做 HITL / 鉴权 UI（钩子以后可写在各工具的 [`Tool::call`] 内）
//!
//! ## 模块结构
//!
//! - [`permission`] — RWX 权限位与 `-R-W-X-` 文本格式
//! - [`meta`] — [`ToolMeta`] / [`ToolResult`]
//! - [`traits`] — [`Tool`] trait
//! - [`registry`] — [`ToolRegistry`] / [`ToolBundle`]
//! - [`error`] — [`ToolError`]
//! - [`tools`] — 具体工具占位（后续填充）

#![deny(missing_docs)]

pub mod error;
pub mod meta;
pub mod permission;
pub mod registry;
pub mod traits;
pub mod tools;

pub use error::ToolError;
pub use meta::{ToolMeta, ToolResult};
pub use permission::Permission;
pub use registry::{openai_tool_schema, ToolBundle, ToolRegistry};
pub use traits::Tool;
