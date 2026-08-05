//! 动作 trait 与执行上下文。

use std::future::Future;
use std::pin::Pin;

use lya_base::Mode;
use serde_json::Value;

use crate::meta::{ActionMeta, ActionOutcome};

/// 异步调用返回类型（便于 `dyn Action`）。
pub type ActionCallFuture<'a> = Pin<Box<dyn Future<Output = ActionOutcome> + Send + 'a>>;

/// 执行动作时的只读上下文。
///
/// 刻意保持极简：有副作用的动作（如记忆读写）在构造时自己注入依赖，
/// 需要人介入的动作只返回意图。所以这里只放「本次调用发生在哪」的信息，
/// 不塞各种 store。
#[derive(Debug, Clone, Copy)]
pub struct ActionCtx<'a> {
    /// 当前会话 id。
    pub session_id: &'a str,
    /// 当前工作模式。
    pub mode: Mode,
}

impl<'a> ActionCtx<'a> {
    /// 构造上下文。
    pub fn new(session_id: &'a str, mode: Mode) -> Self {
        Self { session_id, mode }
    }
}

/// 一个元认知动作。
///
/// 结构与 [`lya_tool::Tool`] 对称，两处差异：
///
/// - [`Action::call`] 多一个 [`ActionCtx`]，且返回 [`ActionOutcome`]
///   而不是单一结果——动作可能要求挂起等人
/// - 没有 RWX 权限；能见与否由 [`Action::visible_in`] 按模式自行判断
pub trait Action: Send + Sync {
    /// 静态元信息。
    fn meta(&self) -> &ActionMeta;

    /// 参数 JSON Schema（OpenAI `function.parameters`）。
    fn parameters(&self) -> &Value;

    /// 用法说明，由注册中心拼进提示词段。
    fn prompt_hint(&self) -> &str;

    /// 该模式下是否暴露给模型。
    ///
    /// 默认全模式可见。少数动作有适用条件——比如「请求切换模式」在 agent
    /// 模式下没有意义，因为已经是最高权限了。这不是 RWX 过滤。
    fn visible_in(&self, _mode: Mode) -> bool {
        true
    }

    /// 执行动作。
    ///
    /// `args` 是模型给出的参数对象。参数校验失败不要返回 `Err`，而是用
    /// [`ActionOutcome::err`] 把原因回灌给模型，让它自己改。
    fn call<'a>(&'a self, ctx: ActionCtx<'a>, args: Value) -> ActionCallFuture<'a>;
}
