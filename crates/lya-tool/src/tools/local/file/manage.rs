//! `file_manage`：删除、移动、复制、查看信息、建目录。
//!
//! 这几件事都作用在「路径」上、参数形状接近、使用频率也远低于读写，合成一个
//! 工具比拆成五个更省提示词预算。读和写才值得各自独立。
//!
//! 对文件和目录一视同仁——`delete` / `move` / `copy` 两者都能处理。

use std::fs;
use std::path::Path;

use serde_json::{json, Value};

use crate::meta::{ToolMeta, ToolResult};
use lya_base::Permission;
use crate::tools::local::file::write::describe_path_error;
use crate::tools::local::path::{resolve_path, ResolvedPath};
use crate::context::ToolCtx;
use crate::traits::{Tool, ToolCallFuture};

/// `file_manage` 工具。
pub struct FileManageTool {
    /// 静态 meta。
    meta: ToolMeta,
    /// OpenAI `parameters` JSON Schema。
    parameters: Value,
    /// 用法说明。
    prompt_hint: &'static str,
}

impl FileManageTool {
    /// 构造工具实例。
    pub fn new() -> Self {
        Self {
            meta: ToolMeta::new(
                "file_manage",
                "管理文件",
                "对文件或目录做删除、移动、复制、查看信息、新建目录",
                // 删除与移动是不可逆的，按执行级权限管，ask/edit 都拿不到
                Permission::READ_WRITE_EXEC,
            ),
            parameters: json!({
                "type": "object",
                "properties": {
                    "operation": {
                        "type": "string",
                        "enum": ["info", "mkdir", "delete", "move", "copy"],
                        "description": "要做什么。info/mkdir/delete 用 path；move/copy 用 source 与 target。"
                    },
                    "path": {
                        "type": "string",
                        "description": "info / mkdir / delete 的目标路径。"
                    },
                    "source": {
                        "type": "string",
                        "description": "move / copy 的来源路径。"
                    },
                    "target": {
                        "type": "string",
                        "description": "move / copy 的目标路径。"
                    },
                    "recursive": {
                        "type": "boolean",
                        "description": "delete 非空目录时必须显式设为 true。默认 false。"
                    },
                    "overwrite": {
                        "type": "boolean",
                        "description": "move / copy 时目标已存在是否覆盖。默认 false。"
                    }
                },
                "required": ["operation"],
                "additionalProperties": false
            }),
            prompt_hint: concat!(
                "使用 file_manage 处理文件与目录：\n",
                "1) **删除不可撤销**。删之前先确认路径没写错；不确定这个目录里有什么就先 dir_list 看一眼。\n",
                "2) 删非空目录要显式设 recursive=true，这是让你停下来想一秒的闸门，不是走过场。\n",
                "3) move / copy 默认不覆盖已有目标，确实要盖掉才设 overwrite=true。\n",
                "4) 家目录本身和文件系统根目录一律拒绝操作。\n",
                "5) info 只看大小与修改时间，不读内容；要看内容用 file_read。"
            ),
        }
    }
}

impl Default for FileManageTool {
    fn default() -> Self {
        Self::new()
    }
}

impl Tool for FileManageTool {
    fn meta(&self) -> &ToolMeta {
        &self.meta
    }

    fn parameters(&self) -> &Value {
        &self.parameters
    }

    fn prompt_hint(&self) -> &str {
        self.prompt_hint
    }

    fn call(&self, _ctx: ToolCtx, args: Value) -> ToolCallFuture<'_> {
        Box::pin(async move { run_file_manage(&args) })
    }
}

/// 解析参数并分发到具体操作。
fn run_file_manage(args: &Value) -> ToolResult {
    let Some(operation) = args.get("operation").and_then(Value::as_str) else {
        return ToolResult::err("缺少必填参数 `operation`");
    };

    match operation {
        "info" | "mkdir" | "delete" => {
            let resolved = match resolve_arg(args, "path") {
                Ok(resolved) => resolved,
                Err(msg) => return ToolResult::err(msg),
            };
            if let Some(msg) = guard_protected(&resolved) {
                return ToolResult::err(msg);
            }
            match operation {
                "info" => info(&resolved.absolute),
                "mkdir" => mkdir(&resolved.absolute),
                _ => delete(
                    &resolved.absolute,
                    args.get("recursive").and_then(Value::as_bool).unwrap_or(false),
                ),
            }
        }
        "move" | "copy" => {
            let source = match resolve_arg(args, "source") {
                Ok(resolved) => resolved,
                Err(msg) => return ToolResult::err(msg),
            };
            let target = match resolve_arg(args, "target") {
                Ok(resolved) => resolved,
                Err(msg) => return ToolResult::err(msg),
            };
            if let Some(msg) = guard_protected(&source) {
                return ToolResult::err(msg);
            }
            let overwrite = args
                .get("overwrite")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            transfer(
                &source.absolute,
                &target.absolute,
                overwrite,
                operation == "move",
            )
        }
        other => ToolResult::err(format!(
            "未知的 operation {other:?}，应为 info / mkdir / delete / move / copy"
        )),
    }
}

/// 取出并解析一个路径参数。
fn resolve_arg(args: &Value, key: &str) -> Result<ResolvedPath, String> {
    let raw = args
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("该 operation 需要参数 `{key}`"))?;
    resolve_path(raw).map_err(|err| describe_path_error(&err))
}

/// 拦住家目录本身与文件系统根。
///
/// 模型偶尔会把「清理一下」理解成删掉整个目录；这两处一旦删掉就不是「重来一次」
/// 能解决的了。
fn guard_protected(resolved: &ResolvedPath) -> Option<String> {
    let path = &resolved.absolute;
    if path == &resolved.home {
        return Some("拒绝操作家目录本身。".into());
    }
    if path.parent().is_none() {
        return Some("拒绝操作文件系统根目录。".into());
    }
    None
}

fn info(path: &Path) -> ToolResult {
    let meta = match fs::symlink_metadata(path) {
        Ok(meta) => meta,
        Err(err) => return ToolResult::err(format!("{} 不可访问：{err}", path.display())),
    };

    let kind = if meta.is_dir() {
        "目录"
    } else if meta.file_type().is_symlink() {
        "符号链接"
    } else {
        "文件"
    };
    let mut out = format!("{}\n类型: {kind}\n", path.display());
    if meta.is_file() {
        out.push_str(&format!("大小: {}\n", human_size(meta.len())));
    }
    if let Ok(modified) = meta.modified()
        && let Ok(time) = modified.duration_since(std::time::UNIX_EPOCH)
    {
        out.push_str(&format!("修改时间戳: {}\n", time.as_secs()));
    }
    ToolResult::ok(out)
}

fn mkdir(path: &Path) -> ToolResult {
    if path.exists() {
        return if path.is_dir() {
            ToolResult::ok(format!("{} 已经存在", path.display()))
        } else {
            ToolResult::err(format!("{} 已存在且不是目录", path.display()))
        };
    }
    match fs::create_dir_all(path) {
        Ok(()) => ToolResult::ok(format!("已创建目录 {}", path.display())),
        Err(err) => ToolResult::err(format!("创建 {} 失败：{err}", path.display())),
    }
}

fn delete(path: &Path, recursive: bool) -> ToolResult {
    let meta = match fs::symlink_metadata(path) {
        Ok(meta) => meta,
        Err(err) => return ToolResult::err(format!("{} 不存在或不可访问：{err}", path.display())),
    };

    if meta.is_dir() {
        let empty = fs::read_dir(path)
            .map(|mut entries| entries.next().is_none())
            .unwrap_or(false);
        if !empty && !recursive {
            return ToolResult::err(format!(
                "{} 不是空目录。确认要连同内容一起删除，请设 recursive=true。",
                path.display()
            ));
        }
        return match fs::remove_dir_all(path) {
            Ok(()) => ToolResult::ok(format!("已删除目录 {}", path.display())),
            Err(err) => ToolResult::err(format!("删除 {} 失败：{err}", path.display())),
        };
    }

    match fs::remove_file(path) {
        Ok(()) => ToolResult::ok(format!("已删除 {}", path.display())),
        Err(err) => ToolResult::err(format!("删除 {} 失败：{err}", path.display())),
    }
}

fn transfer(source: &Path, target: &Path, overwrite: bool, is_move: bool) -> ToolResult {
    let verb = if is_move { "移动" } else { "复制" };
    if !source.exists() {
        return ToolResult::err(format!("来源 {} 不存在", source.display()));
    }
    if target.exists() && !overwrite {
        return ToolResult::err(format!(
            "目标 {} 已存在。确认要覆盖请设 overwrite=true。",
            target.display()
        ));
    }
    if let Some(parent) = target.parent()
        && !parent.exists()
    {
        return ToolResult::err(format!("目标的父目录 {} 不存在", parent.display()));
    }

    if is_move {
        return match fs::rename(source, target) {
            Ok(()) => ToolResult::ok(format!(
                "已移动 {} → {}",
                source.display(),
                target.display()
            )),
            Err(err) => ToolResult::err(format!("{verb}失败：{err}")),
        };
    }

    if source.is_dir() {
        return match copy_dir(source, target) {
            Ok(count) => ToolResult::ok(format!(
                "已复制目录 {} → {}（{count} 个文件）",
                source.display(),
                target.display()
            )),
            Err(err) => ToolResult::err(format!("{verb}失败：{err}")),
        };
    }
    match fs::copy(source, target) {
        Ok(_) => ToolResult::ok(format!(
            "已复制 {} → {}",
            source.display(),
            target.display()
        )),
        Err(err) => ToolResult::err(format!("{verb}失败：{err}")),
    }
}

/// 递归复制目录，返回复制的文件数。
fn copy_dir(source: &Path, target: &Path) -> std::io::Result<usize> {
    fs::create_dir_all(target)?;
    let mut count = 0;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let to = target.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            count += copy_dir(&entry.path(), &to)?;
        } else {
            fs::copy(entry.path(), &to)?;
            count += 1;
        }
    }
    Ok(count)
}

/// 人类可读的字节数。
pub(crate) fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KiB", "MiB", "GiB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mkdir_then_info_then_delete() {
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join("新目录");

        assert!(mkdir(&sub).success);
        assert!(sub.is_dir());
        assert!(mkdir(&sub).success, "已存在应视为成功");

        let info = info(&sub);
        assert!(info.success);
        assert!(info.content.contains("目录"));

        assert!(delete(&sub, false).success, "空目录不需要 recursive");
        assert!(!sub.exists());
    }

    #[test]
    fn non_empty_dir_needs_recursive() {
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join("有东西");
        fs::create_dir(&sub).unwrap();
        fs::write(sub.join("a.txt"), "x").unwrap();

        let refused = delete(&sub, false);
        assert!(!refused.success);
        assert!(refused.content.contains("recursive"));
        assert!(sub.exists(), "拒绝时不能动");

        assert!(delete(&sub, true).success);
        assert!(!sub.exists());
    }

    #[test]
    fn move_and_copy_respect_overwrite() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.txt");
        let b = dir.path().join("b.txt");
        fs::write(&a, "内容").unwrap();
        fs::write(&b, "占位").unwrap();

        let refused = transfer(&a, &b, false, false);
        assert!(!refused.success);
        assert!(refused.content.contains("overwrite"));
        assert_eq!(fs::read_to_string(&b).unwrap(), "占位");

        assert!(transfer(&a, &b, true, false).success);
        assert_eq!(fs::read_to_string(&b).unwrap(), "内容");
        assert!(a.exists(), "copy 不该删掉来源");

        let c = dir.path().join("c.txt");
        assert!(transfer(&a, &c, false, true).success);
        assert!(!a.exists(), "move 之后来源应消失");
    }

    #[test]
    fn copy_directory_counts_files() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src");
        fs::create_dir_all(src.join("深")).unwrap();
        fs::write(src.join("a.txt"), "1").unwrap();
        fs::write(src.join("深/b.txt"), "2").unwrap();

        let result = transfer(&src, &dir.path().join("dst"), false, false);
        assert!(result.success, "{}", result.content);
        assert!(result.content.contains("2 个文件"));
        assert!(dir.path().join("dst/深/b.txt").exists());
    }

    #[test]
    fn unknown_operation_lists_valid_ones() {
        let result = run_file_manage(&json!({ "operation": "chmod" }));
        assert!(!result.success);
        assert!(result.content.contains("mkdir"));
    }

    #[test]
    fn missing_path_argument_is_explained() {
        let result = run_file_manage(&json!({ "operation": "delete" }));
        assert!(!result.success);
        assert!(result.content.contains("`path`"));
    }

    #[test]
    fn sizes_are_human_readable() {
        assert_eq!(human_size(512), "512 B");
        assert_eq!(human_size(2048), "2.0 KiB");
    }
}
