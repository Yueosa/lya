//! 单个工具的 trait 约定。
//!
//! 实现与提示词放在同一类型内；注册中心只认 [`Tool`]。

use std::future::Future;
use std::pin::Pin;

use serde_json::Value;

use crate::meta::{ToolMeta, ToolResult};

/// 异步调用返回类型（便于 `dyn Tool`）。
pub type ToolCallFuture<'a> = Pin<Box<dyn Future<Output = ToolResult> + Send + 'a>>;

/// 可注册的工具。
///
/// # 字段职责回顾
///
/// - [`Tool::meta`]：`name` / `raw_name` / `desc` / `prmt`
/// - [`Tool::parameters`]：JSON Schema，告诉模型应返回的参数结构
/// - [`Tool::prompt_hint`]：用法说明 / 技巧（进 system prompt，不进 `tools[]`）
/// - [`Tool::call`]：执行；调用前审查、调用后钩子也统一写在这里
pub trait Tool: Send + Sync {
    /// 静态元信息。
    fn meta(&self) -> &ToolMeta;

    /// 参数 JSON Schema（OpenAI `function.parameters`）。
    ///
    /// 一般是 `{ "type": "object", "properties": {...}, "required": [...] }`。
    fn parameters(&self) -> &Value;

    /// 用法说明，由注册中心拼进提示词段。
    fn prompt_hint(&self) -> &str;

    /// 执行工具。
    ///
    /// `args` 是模型给出的参数对象（已从 `arguments` JSON 字符串解析）。
    /// Schema 校验、权限外的业务预检、后置钩子都可在此完成。
    fn call(&self, args: Value) -> ToolCallFuture<'_>;
}
