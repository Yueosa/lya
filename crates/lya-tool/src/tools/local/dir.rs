//! `dir_list`：列目录，可按深度展开成树。
//!
//! 列表和树是同一件事的两种深度，没必要拆成两个工具：`depth=1` 就是平铺列表，
//! 更大就是树。

use std::fs;
use std::path::Path;

use serde_json::{json, Value};

use crate::meta::{ToolMeta, ToolResult};
use crate::permission::Permission;
use crate::tools::local::file::manage::human_size;
use crate::tools::local::file::write::describe_path_error;
use crate::tools::local::path::resolve_path;
use crate::context::ToolCtx;
use crate::traits::{Tool, ToolCallFuture};

/// 默认展开深度。
const DEFAULT_DEPTH: usize = 1;
/// 最大展开深度。
const MAX_DEPTH: usize = 8;
/// 默认返回条目上限。
const DEFAULT_LIMIT: usize = 300;
/// 条目上限的硬顶。
const MAX_LIMIT: usize = 2000;

/// `dir_list` 工具。
pub struct DirListTool {
    /// 静态 meta。
    meta: ToolMeta,
    /// OpenAI `parameters` JSON Schema。
    parameters: Value,
    /// 用法说明。
    prompt_hint: &'static str,
}

impl DirListTool {
    /// 构造工具实例。
    pub fn new() -> Self {
        Self {
            meta: ToolMeta::new(
                "dir_list",
                "列出目录",
                "列出目录内容，可指定深度展开成树、按名称过滤",
                Permission::READ,
            ),
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "目录路径，默认家目录。~/ 或相对路径基于家目录；以 / 开头为绝对路径。"
                    },
                    "depth": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": MAX_DEPTH,
                        "description": "展开几层。1 为平铺列出当前层（默认），更大则展开成树。"
                    },
                    "pattern": {
                        "type": "string",
                        "description": "只保留名字包含该子串的条目（不区分大小写）。目录始终展开以便继续深入。"
                    },
                    "include_hidden": {
                        "type": "boolean",
                        "description": "是否包含以 . 开头的隐藏项。默认 false。"
                    },
                    "only": {
                        "type": "string",
                        "enum": ["files", "dirs"],
                        "description": "只列文件或只列目录；默认两者都列。"
                    },
                    "limit": {
                        "type": "integer",
                        "minimum": 1,
                        "description": "最多返回多少条，默认 300，上限 2000。"
                    }
                },
                "additionalProperties": false
            }),
            prompt_hint: concat!(
                "使用 dir_list 摸清目录结构：\n",
                "1) 先用默认 depth=1 看一层，确认方向对了再往下钻，别一上来就 depth=8——大目录会瞬间吃满预算。\n",
                "2) 找特定文件用 pattern 过滤，比拉全量列表再自己筛省得多。\n",
                "3) 条目超过 limit 会截断并标注，说明你该缩小范围而不是加大 limit。\n",
                "4) 不进入符号链接指向的目录，避免绕圈。\n",
                "5) 要看文件内容用 file_read，这里只给名字和大小。"
            ),
        }
    }
}

impl Default for DirListTool {
    fn default() -> Self {
        Self::new()
    }
}

impl Tool for DirListTool {
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
        Box::pin(async move {
            // 不给 path 就看家目录
            let raw = args.get("path").and_then(Value::as_str).unwrap_or("~");
            match resolve_path(raw) {
                Ok(resolved) => run_dir_list_at(&resolved.absolute, &args),
                Err(err) => ToolResult::err(describe_path_error(&err)),
            }
        })
    }
}

/// 一次列目录的选项。
struct Options {
    depth: usize,
    pattern: Option<String>,
    include_hidden: bool,
    only_files: bool,
    only_dirs: bool,
    limit: usize,
}

/// 路径已解析后的列目录主体。
pub(crate) fn run_dir_list_at(abs_path: &Path, args: &Value) -> ToolResult {
    if !abs_path.is_dir() {
        return ToolResult::err(format!("{} 不是目录", abs_path.display()));
    }

    let only = args.get("only").and_then(Value::as_str);
    if let Some(other) = only
        && other != "files"
        && other != "dirs"
    {
        return ToolResult::err(format!("未知的 only {other:?}，应为 files 或 dirs"));
    }

    let options = Options {
        depth: args
            .get("depth")
            .and_then(Value::as_u64)
            .map(|d| (d as usize).clamp(1, MAX_DEPTH))
            .unwrap_or(DEFAULT_DEPTH),
        pattern: args
            .get("pattern")
            .and_then(Value::as_str)
            .filter(|p| !p.trim().is_empty())
            .map(|p| p.to_lowercase()),
        include_hidden: args
            .get("include_hidden")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        only_files: only == Some("files"),
        only_dirs: only == Some("dirs"),
        limit: args
            .get("limit")
            .and_then(Value::as_u64)
            .map(|n| (n as usize).min(MAX_LIMIT))
            .unwrap_or(DEFAULT_LIMIT),
    };

    let mut lines = Vec::new();
    let mut truncated = false;
    walk(abs_path, 0, &options, &mut lines, &mut truncated);

    if lines.is_empty() {
        return ToolResult::ok(format!("{}（空，或没有符合条件的条目）", abs_path.display()));
    }

    let mut out = format!("{}\n", abs_path.display());
    out.push_str(&lines.join("\n"));
    if truncated {
        out.push_str(&format!(
            "\n… 超过 {} 条已截断，请用 pattern 缩小范围或降低 depth",
            options.limit
        ));
    }
    ToolResult::ok(out)
}

/// 递归收集条目。
fn walk(dir: &Path, level: usize, options: &Options, lines: &mut Vec<String>, truncated: &mut bool) {
    if level >= options.depth || *truncated {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };

    // 排序让输出稳定：目录在前，同类按名字
    let mut items: Vec<_> = entries.flatten().collect();
    items.sort_by_key(|entry| {
        let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
        (!is_dir, entry.file_name())
    });

    for entry in items {
        if *truncated {
            return;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if !options.include_hidden && name.starts_with('.') {
            continue;
        }
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        let is_dir = file_type.is_dir();

        let kind_ok = if options.only_files {
            !is_dir
        } else if options.only_dirs {
            is_dir
        } else {
            true
        };
        // 目录即便被 pattern 过滤掉也要继续往下走，否则匹配项藏在子目录里就找不到
        let name_ok = options
            .pattern
            .as_ref()
            .is_none_or(|p| name.to_lowercase().contains(p));

        if kind_ok && name_ok {
            if lines.len() >= options.limit {
                *truncated = true;
                return;
            }
            let indent = "  ".repeat(level);
            lines.push(if is_dir {
                format!("{indent}{name}/")
            } else {
                let size = entry.metadata().map(|m| human_size(m.len())).unwrap_or_default();
                format!("{indent}{name}  {size}")
            });
        }

        // 不跟随符号链接，免得绕圈
        if is_dir && !file_type.is_symlink() {
            walk(&entry.path(), level + 1, options, lines, truncated);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("src/deep")).unwrap();
        fs::write(dir.path().join("a.txt"), "12345").unwrap();
        fs::write(dir.path().join(".hidden"), "x").unwrap();
        fs::write(dir.path().join("src/lib.rs"), "x").unwrap();
        fs::write(dir.path().join("src/deep/mod.rs"), "x").unwrap();
        dir
    }

    #[test]
    fn lists_one_level_by_default() {
        let dir = fixture();
        let result = run_dir_list_at(dir.path(), &json!({}));
        assert!(result.success);
        assert!(result.content.contains("src/"));
        assert!(result.content.contains("a.txt"));
        assert!(!result.content.contains("lib.rs"), "默认不展开子目录");
        assert!(!result.content.contains(".hidden"), "默认不列隐藏项");
    }

    #[test]
    fn depth_expands_into_a_tree() {
        let dir = fixture();
        let result = run_dir_list_at(dir.path(), &json!({ "depth": 3 }));
        assert!(result.content.contains("lib.rs"));
        assert!(result.content.contains("mod.rs"));
    }

    #[test]
    fn hidden_files_need_opt_in() {
        let dir = fixture();
        let result = run_dir_list_at(dir.path(), &json!({ "include_hidden": true }));
        assert!(result.content.contains(".hidden"));
    }

    #[test]
    fn pattern_filters_but_still_descends() {
        let dir = fixture();
        let result = run_dir_list_at(dir.path(), &json!({ "depth": 3, "pattern": "mod" }));
        assert!(result.content.contains("mod.rs"), "深处的匹配项要能找到");
        assert!(!result.content.contains("a.txt"));
    }

    #[test]
    fn only_filters_by_kind() {
        let dir = fixture();
        let dirs = run_dir_list_at(dir.path(), &json!({ "only": "dirs" }));
        assert!(dirs.content.contains("src/"));
        assert!(!dirs.content.contains("a.txt"));

        let files = run_dir_list_at(dir.path(), &json!({ "only": "files" }));
        assert!(files.content.contains("a.txt"));
        assert!(!files.content.contains("src/"));
    }

    #[test]
    fn limit_truncates_with_a_hint() {
        let dir = fixture();
        let result = run_dir_list_at(dir.path(), &json!({ "depth": 3, "limit": 1 }));
        assert!(result.content.contains("已截断"));
        assert!(result.content.contains("pattern"));
    }

    #[test]
    fn shows_file_size() {
        let dir = fixture();
        let result = run_dir_list_at(dir.path(), &json!({}));
        assert!(result.content.contains("5 B"), "{}", result.content);
    }

    #[test]
    fn rejects_non_directory() {
        let dir = fixture();
        let result = run_dir_list_at(&dir.path().join("a.txt"), &json!({}));
        assert!(!result.success);
        assert!(result.content.contains("不是目录"));
    }
}
