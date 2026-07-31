//! 动作错误。
//!
//! 注意区分两类失败：
//!
//! - **模型可以自己修正的**（参数填错、记忆不存在）不走这里，而是作为
//!   [`crate::ActionOutcome::Continue`] 里的失败结果回灌，让模型看到错误
//!   信息后重试
//! - **调用方用错了 API**（动作名不存在、重复注册）才走这里

/// `lya-action` 错误。
#[derive(Debug, thiserror::Error)]
pub enum ActionError {
    /// 注册了同名动作。
    #[error("duplicate action name: {0}")]
    DuplicateName(String),

    /// 动作不存在。
    #[error("action not found: {0}")]
    NotFound(String),
}
