//! agent 错误。
//!
//! 只有**装配期**和**调用方用错**的错误走这里。一轮跑起来之后出的问题
//! （模型报错、工具失败）不返回 `Err`，而是作为
//! [`crate::AgentEvent::TurnEnd`] 的结束原因或回灌给模型的失败结果，
//! 因为那时事件流已经开始了，调用方需要的是一个完整收尾而不是一个异常。

use lya_memory::MemoryError;
use lya_session::SessionError;

/// `lya-agent` 错误。
#[derive(Debug, thiserror::Error)]
pub enum AgentError {
    /// 会话层错误。
    #[error(transparent)]
    Session(#[from] SessionError),

    /// 记忆层错误。
    #[error(transparent)]
    Memory(#[from] MemoryError),

    /// 工具与动作重名——它们会合并进同一个 `tools[]`，模型没法区分。
    #[error("name collision between tool and action: {0}")]
    NameCollision(String),

    /// 当前状态不允许该操作。
    #[error("{0}")]
    Invalid(String),
}
