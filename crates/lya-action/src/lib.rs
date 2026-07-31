//! # lya-action
//!
//! 元认知动作：模型用来操作**自己的状态**的一组函数。
//!
//! ## 与 lya-tool 的关系
//!
//! 对外**同一套协议**——动作和工具一起进 `tools[]`，模型用同样的 function
//! calling 语法调用，schema 由 [`lya_tool::openai_function_schema`] 统一生成。
//!
//! lya 内部当**两类东西**治理：
//!
//! | | 工具 | 动作 |
//! |---|---|---|
//! | 作用对象 | 外部环境（文件、命令、网络） | 自己的状态（记忆、与用户交互、模式） |
//! | 筛选依据 | 模式的 RWX 权限 ∩ 会话启用名单 | 只看动作自己的适用条件 |
//! | 用户可否禁用 | 可以 | 不可以 |
//! | 流转 | 只有「回灌后继续」 | 还可能挂起等人（[`ActionFlow`]） |
//!
//! ## 边界
//!
//! - 有副作用的动作构造时自己注入依赖（记忆动作持有 `MemoryStore`）
//! - 需要人介入的动作**只返回意图**（[`ActionOutcome::AwaitHuman`]），
//!   本 crate 不碰 `SessionStore`；入树、挂起、恢复都由 agent 负责
//! - 不实现 `done`：一条 assistant 消息不带 `tool_calls` 就是本轮结束，
//!   要边说边干就让 `content` 和 `tool_calls` 同时出现，不需要额外信号

#![deny(missing_docs)]

mod args;

pub mod actions;
pub mod error;
pub mod meta;
pub mod registry;
pub mod traits;

pub use actions::{
    FormAction, FormAnswer, FormAnswerItem, MemoryReadAction, MemoryWriteAction,
    RequestModeChangeAction, register_builtins, render_form_answer,
};
pub use error::ActionError;
pub use meta::{ActionFlow, ActionMeta, ActionOutcome};
pub use registry::{ActionBundle, ActionRegistry};
pub use traits::{Action, ActionCallFuture, ActionCtx};
