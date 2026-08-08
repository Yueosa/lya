//! 具体工具实现。
//!
//! - [`local`]：本机文件系统等本地工具
//! - 启动时调用 [`register_builtins`] 把内置工具挂进注册中心

pub mod local;
pub mod shell;
pub mod web;

use std::sync::Arc;

use lya_http::HttpClient;

use crate::error::ToolError;
use crate::registry::ToolRegistry;

/// 注册全部内置工具。
///
/// 由进程启动组装处调用一次即可。网络工具共用调用方传进来的
/// [`HttpClient`]，避免各自建连接池。
///
/// `self_port` 让 `web_fetch` 认出「访问 lya 自己」并拒绝。注册发生在监听之前，
/// 所以传的是个共享原子量，绑定成功后由启动流程回填；一直是 0 也安全，那样
/// 所有本机地址都按内网走确认。
pub fn register_builtins(
    registry: &mut ToolRegistry,
    http: HttpClient,
    shell_confirm: impl Into<lya_base::Live<shell::ConfirmPolicy>>,
    self_port: web::SelfPort,
) -> Result<(), ToolError> {
    registry.register(Arc::new(local::FileReadTool::new()))?;
    registry.register(Arc::new(local::FileWriteTool::new()))?;
    registry.register(Arc::new(local::FileEditTool::new()))?;
    registry.register(Arc::new(local::FileManageTool::new()))?;
    registry.register(Arc::new(local::DirListTool::new()))?;
    registry.register(Arc::new(local::SystemInfoTool::new()))?;
    registry.register(Arc::new(local::ImageScanTool::new()))?;
    registry.register(Arc::new(web::WebSearchTool::new(http.clone())))?;
    registry.register(Arc::new(web::WebFetchTool::new(http, self_port)))?;
    registry.register(Arc::new(shell::BashTool::new(shell_confirm)))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use lya_base::Permission;

    use super::*;

    fn names(permission: Permission) -> Vec<String> {
        let http = HttpClient::with_defaults().unwrap();
        let mut registry = ToolRegistry::new();
        register_builtins(
            &mut registry,
            http,
            shell::ConfirmPolicy::default(),
            Default::default(),
        )
        .unwrap();
        registry.bundle(None, permission, &[]).names
    }

    /// 钉住「哪个模式看得见哪些工具」。
    ///
    /// 权限标错是很难在别处发现的错误——把写工具标成 `-R-`，ask 模式就能改文件，
    /// 而一切看起来都正常。
    #[test]
    fn builtin_tools_are_graded_by_permission() {
        assert_eq!(
            names(Permission::READ_ONLY),
            vec![
                "dir_list",
                "file_read",
                "image_scan",
                "system_info",
                "web_fetch",
                "web_search"
            ],
            "ask 模式只该看到只读工具；上网查资料也算只读"
        );
        assert_eq!(
            names(Permission::READ_WRITE),
            vec![
                "dir_list",
                "file_edit",
                "file_read",
                "file_write",
                "image_scan",
                "system_info",
                "web_fetch",
                "web_search"
            ],
            "edit 模式多出改内容的能力，但拿不到删除与移动"
        );
        assert_eq!(
            names(Permission::READ_WRITE_EXEC),
            vec![
                "bash",
                "dir_list",
                "file_edit",
                "file_manage",
                "file_read",
                "file_write",
                "image_scan",
                "system_info",
                "web_fetch",
                "web_search"
            ],
            "agent 模式才有不可逆操作与命令执行"
        );
    }
}
