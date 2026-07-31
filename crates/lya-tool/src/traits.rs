//! 单个工具的 trait 约定。
//!
//! 实现与提示词放在同一类型内；注册中心只认 [`Tool`]。

use std::future::Future;
use std::pin::Pin;

use serde_json::Value;

use crate::confirm::ConfirmRequest;
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

    /// 执行前是否要让用户过目；返回 `Some` 则挂起等放行。
    ///
    /// 默认不需要——绝大多数工具的作用范围由 RWX 权限就能框住。只有 `bash`
    /// 这种「参数本身就是另一门语言」的工具才需要逐次判断。
    ///
    /// 这必须是对参数的**纯函数**：只看要做什么，不产生任何副作用。
    fn confirm_request(&self, _args: &Value) -> Option<ConfirmRequest> {
        None
    }

    /// 执行工具。
    ///
    /// `args` 是模型给出的参数对象（已从 `arguments` JSON 字符串解析）。
    /// Schema 校验、权限外的业务预检、后置钩子都可在此完成。
    fn call(&self, args: Value) -> ToolCallFuture<'_>;
}
