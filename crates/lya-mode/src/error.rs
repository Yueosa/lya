//! 模式解析错误。

/// 无法把字符串解析为 [`crate::Mode`]。
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("unknown lya mode `{value}`; expected ask, edit, or agent")]
pub struct ModeParseError {
    /// 收到的原始模式字符串。
    pub value: String,
}
