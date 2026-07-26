//! 具体工具实现。
//!
//! - [`local`]：本机文件系统等本地工具
//! - 启动时调用 [`register_builtins`] 把内置工具挂进注册中心

pub mod local;

use std::sync::Arc;

use crate::error::ToolError;
use crate::registry::ToolRegistry;

/// 注册全部内置工具。
///
/// 由进程启动组装处调用一次即可。
pub fn register_builtins(registry: &mut ToolRegistry) -> Result<(), ToolError> {
    registry.register(Arc::new(local::file::FileReadTool::new()))?;
    Ok(())
}
