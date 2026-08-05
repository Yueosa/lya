//! 工具错误。

/// `lya-tool` 可返回的错误。
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ToolError {
    /// 注册时名称冲突。
    #[error("tool already registered: {0}")]
    DuplicateName(String),

    /// 调用了未注册的工具。
    #[error("tool not found: {0}")]
    NotFound(String),

    /// 工具执行失败（业务错误；也可直接放在 [`crate::ToolResult`] 里返回）。
    #[error("tool call failed: {0}")]
    Call(String),
}
