//! 本 crate 的两个错误。

use thiserror::Error;

/// 解析数据根失败。
///
/// 只有 `HOME` 没设或为空这一种情况。上层各自的错误类型用 `#[from]` 或
/// `map_err` 把它转成自己的。
#[derive(Debug, Error)]
pub enum BaseError {
    /// `HOME` 缺失或为空，算不出 `~/.lya`。
    #[error("解析数据目录失败：{0}")]
    Path(String),
}

/// 工作模式字符串不认识。
#[derive(Debug, Error, PartialEq, Eq)]
#[error("未知工作模式 {value:?}，可用：ask / edit / agent")]
pub struct ModeParseError {
    /// 原始输入，照抄进错误信息，好让用户看出自己打错在哪。
    pub value: String,
}
