//! `file_write`：整文件写入或追加。
//!
//! 只负责「把这段内容放进去」。要在原文里改一小段请用 `file_edit`——让模型
//! 为了改一行而重新吐出整个文件，既费 token 又容易在复述过程中改坏别处。

use std::fs;
use std::path::Path;

use serde_json::{json, Value};

use crate::context::ToolCtx;
use crate::limits::file_write::MAX_WRITE_BYTES;
use crate::meta::{ToolMeta, ToolResult};
use lya_base::Permission;
use crate::tools::local::path::{resolve_path, PathError};
use crate::traits::{Tool, ToolCallFuture};

/// `file_write` 工具。
pub struct FileWriteTool {
    /// 静态 meta。
    meta: ToolMeta,
    /// OpenAI `parameters` JSON Schema。
    parameters: Value,
    /// 用法说明。数值从 `limits.rs` 取，别在文案里手写。
    prompt_hint: String,
}

impl FileWriteTool {
    /// 构造工具实例。
    pub fn new() -> Self {
        Self {
            meta: ToolMeta::new(
                "file_write",
                "写入文件",
                "把内容写入文件：覆盖整个文件或追加到末尾；文件不存在则创建",
                Permission::READ_WRITE,
            ),
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "文件路径。~/ 或相对路径基于家目录；以 / 开头为绝对路径。禁止用 ../ 从家目录相对路径逃逸。"
                    },
                    "content": {
                        "type": "string",
                        "description": "要写入的完整内容。"
                    },
                    "mode": {
                        "type": "string",
                        "enum": ["overwrite", "append"],
                        "description": "overwrite 覆盖整个文件（默认），append 追加到末尾。"
                    },
                    "create_dirs": {
                        "type": "boolean",
                        "description": "父目录不存在时是否自动创建。默认 false。"
                    }
                },
                "required": ["path", "content"],
                "additionalProperties": false
            }),
            prompt_hint: format!(
                concat!(
                    "使用 file_write 落盘内容：\n",
                    "1) **覆盖前先读**。overwrite 会丢掉原文件的全部内容，没读过就覆盖等于盲写。\n",
                    "2) 只改一小段时用 file_edit，不要把整个文件重写一遍——重写过程中很容易顺手改坏没打算动的地方。\n",
                    "3) 追加日志、往文件尾补内容用 mode=append。\n",
                    "4) 单次内容上限 {} MiB；父目录不存在会报错，确实需要新建时设 create_dirs。\n",
                    "5) 写完会返回行数变化，可据此确认改动规模是否符合预期。"
                ),
                MAX_WRITE_BYTES / (1024 * 1024)
            ),
        }
    }
}

impl Default for FileWriteTool {
    fn default() -> Self {
        Self::new()
    }
}

impl Tool for FileWriteTool {
    fn meta(&self) -> &ToolMeta {
        &self.meta
    }

    fn parameters(&self) -> &Value {
        &self.parameters
    }

    fn prompt_hint(&self) -> &str {
        &self.prompt_hint
    }

    fn call(&self, _ctx: ToolCtx, args: Value) -> ToolCallFuture<'_> {
        Box::pin(async move {
            let path = match args.get("path").and_then(Value::as_str) {
                Some(path) => path,
                None => return ToolResult::err("缺少必填参数 `path`"),
            };
            match resolve_path(path) {
                Ok(resolved) => run_file_write_at(&resolved.absolute, &args),
                Err(err) => ToolResult::err(describe_path_error(&err)),
            }
        })
    }
}

/// 路径已解析后的写入主体。
pub(crate) fn run_file_write_at(abs_path: &Path, args: &Value) -> ToolResult {
    let Some(content) = args.get("content").and_then(Value::as_str) else {
        return ToolResult::err("缺少必填参数 `content`");
    };
    if content.len() > MAX_WRITE_BYTES {
        return ToolResult::err(format!(
            "内容 {} 字节，超过单次写入上限 {MAX_WRITE_BYTES} 字节；请分批写或改用 file_edit",
            content.len()
        ));
    }

    let append = match args.get("mode").and_then(Value::as_str) {
        None | Some("overwrite") => false,
        Some("append") => true,
        Some(other) => {
            return ToolResult::err(format!("未知的 mode {other:?}，应为 overwrite 或 append"));
        }
    };
    let create_dirs = args
        .get("create_dirs")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    if abs_path.is_dir() {
        return ToolResult::err(format!("{} 是目录，不能当作文件写入", abs_path.display()));
    }

    if let Some(parent) = abs_path.parent()
        && !parent.exists()
    {
        if !create_dirs {
            return ToolResult::err(format!(
                "父目录 {} 不存在；确认要新建的话设 create_dirs=true",
                parent.display()
            ));
        }
        if let Err(err) = fs::create_dir_all(parent) {
            return ToolResult::err(format!("创建父目录 {} 失败：{err}", parent.display()));
        }
    }

    // 先记下原样子，写完才好说清楚改动规模
    let existed = abs_path.exists();
    let old_lines = if existed {
        fs::read_to_string(abs_path).ok().map(|text| count_lines(&text))
    } else {
        None
    };

    let outcome = if append {
        use std::io::Write;
        fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(abs_path)
            .and_then(|mut file| file.write_all(content.as_bytes()))
    } else {
        fs::write(abs_path, content)
    };
    if let Err(err) = outcome {
        return ToolResult::err(format!("写入 {} 失败：{err}", abs_path.display()));
    }

    let new_lines = fs::read_to_string(abs_path)
        .ok()
        .map(|text| count_lines(&text));
    let path = abs_path.display();
    ToolResult::ok(match (existed, old_lines, new_lines) {
        (false, _, Some(new)) => format!("已创建 {path}（{new} 行，{} 字节）", content.len()),
        (true, Some(old), Some(new)) if append => {
            format!("已追加到 {path}（{old} 行 → {new} 行）")
        }
        (true, Some(old), Some(new)) => format!("已覆盖 {path}（{old} 行 → {new} 行）"),
        _ => format!("已写入 {path}（{} 字节）", content.len()),
    })
}

/// 统计行数；末尾换行不额外算一行。
pub(crate) fn count_lines(text: &str) -> usize {
    if text.is_empty() {
        return 0;
    }
    text.lines().count()
}

/// 把路径错误翻译成给模型看的说明。
pub(crate) fn describe_path_error(err: &PathError) -> String {
    err.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(value: Value) -> Value {
        value
    }

    #[test]
    fn creates_file_and_reports_lines() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.txt");
        let result = run_file_write_at(&path, &args(json!({ "content": "一\n二\n" })));

        assert!(result.success, "{}", result.content);
        assert!(result.content.contains("已创建"));
        assert!(result.content.contains("2 行"));
        assert_eq!(fs::read_to_string(&path).unwrap(), "一\n二\n");
    }

    #[test]
    fn overwrite_reports_line_delta() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.txt");
        fs::write(&path, "1\n2\n3\n").unwrap();

        let result = run_file_write_at(&path, &args(json!({ "content": "只剩一行\n" })));
        assert!(result.success);
        assert!(result.content.contains("3 行 → 1 行"), "{}", result.content);
    }

    #[test]
    fn append_keeps_existing_content() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.txt");
        fs::write(&path, "旧\n").unwrap();

        let result = run_file_write_at(
            &path,
            &args(json!({ "content": "新\n", "mode": "append" })),
        );
        assert!(result.success);
        assert_eq!(fs::read_to_string(&path).unwrap(), "旧\n新\n");
        assert!(result.content.contains("已追加"));
    }

    #[test]
    fn missing_parent_needs_opt_in() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("深/一层/a.txt");

        let refused = run_file_write_at(&path, &args(json!({ "content": "x" })));
        assert!(!refused.success);
        assert!(refused.content.contains("create_dirs"));

        let ok = run_file_write_at(
            &path,
            &args(json!({ "content": "x", "create_dirs": true })),
        );
        assert!(ok.success, "{}", ok.content);
        assert!(path.exists());
    }

    #[test]
    fn refuses_directory_and_unknown_mode() {
        let dir = tempfile::tempdir().unwrap();
        let on_dir = run_file_write_at(dir.path(), &args(json!({ "content": "x" })));
        assert!(!on_dir.success);
        assert!(on_dir.content.contains("是目录"));

        let bad_mode = run_file_write_at(
            &dir.path().join("a.txt"),
            &args(json!({ "content": "x", "mode": "patch" })),
        );
        assert!(!bad_mode.success);
        assert!(bad_mode.content.contains("overwrite"));
    }

    #[test]
    fn oversized_content_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let huge = "x".repeat(MAX_WRITE_BYTES + 1);
        let result = run_file_write_at(&dir.path().join("a.txt"), &args(json!({ "content": huge })));
        assert!(!result.success);
        assert!(result.content.contains("上限"));
    }
}
