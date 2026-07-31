//! shell 工具：命令解析、风险判定与执行。

mod bash;
pub mod parse;
pub mod rules;

pub use bash::{BashTool, ConfirmPolicy};
