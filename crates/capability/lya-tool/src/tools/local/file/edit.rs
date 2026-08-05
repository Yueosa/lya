//! `file_edit`：按原文片段做精确替换。
//!
//! 改一小段代码时比 `file_write` 安全得多：模型只需要给出「原来长什么样」和
//! 「改成什么样」，其余内容一个字节都不会动。整文件重写则要模型逐字复述全文，
//! 复述过程中改坏别处是很常见的事。
//!
//! 唯一性是这个工具的安全阀：`old_text` 在文件里必须**只出现一次**，否则拒绝
//! 执行并要求补更多上下文——模糊匹配改错地方比报错难查得多。

use std::fs;
use std::path::Path;

use serde_json::{json, Value};

use crate::context::ToolCtx;
use crate::limits::file_edit::MAX_EDIT_BYTES;
use crate::meta::{ToolMeta, ToolResult};
use lya_base::Permission;
use crate::tools::local::file::write::{count_lines, describe_path_error};
use crate::tools::local::path::resolve_path;
use crate::traits::{Tool, ToolCallFuture};

/// `file_edit` 工具。
pub struct FileEditTool {
    /// 静态 meta。
    meta: ToolMeta,
    /// OpenAI `parameters` JSON Schema。
    parameters: Value,
    /// 用法说明。
    prompt_hint: &'static str,
}

impl FileEditTool {
    /// 构造工具实例。
    pub fn new() -> Self {
        Self {
            meta: ToolMeta::new(
                "file_edit",
                "编辑文件",
                "把文件中的一段原文替换为新内容，其余部分保持不变",
                Permission::READ_WRITE,
            ),
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "文件路径。~/ 或相对路径基于家目录；以 / 开头为绝对路径。"
                    },
                    "old_text": {
                        "type": "string",
                        "description": "要被替换掉的原文，必须与文件内容逐字节一致（含缩进与换行）。默认要求全文只出现一次。"
                    },
                    "new_text": {
                        "type": "string",
                        "description": "替换成的新内容。传空字符串即为删除这段。"
                    },
                    "replace_all": {
                        "type": "boolean",
                        "description": "old_text 出现多次时是否全部替换。默认 false（多处命中会直接拒绝）。"
                    }
                },
                "required": ["path", "old_text", "new_text"],
                "additionalProperties": false
            }),
            prompt_hint: concat!(
                "使用 file_edit 做精确改动：\n",
                "1) **先 file_read 看过再改**。old_text 必须和文件里逐字节一致，包括缩进、空格和换行，凭印象写几乎一定对不上。\n",
                "2) old_text 要带足上下文，保证全文唯一。命中多处会被拒绝并告诉你有几处——这时把范围扩大，别改用 replace_all 蒙混。\n",
                "3) 确实要批量替换同一个串（改名之类）时才设 replace_all=true。\n",
                "4) new_text 传空字符串就是删掉这段。\n",
                "5) 改完会返回替换处数与行数变化；和预期不符就说明匹配到了别处，先读回来确认。"
            ),
        }
    }
}

impl Default for FileEditTool {
    fn default() -> Self {
        Self::new()
    }
}

impl Tool for FileEditTool {
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
            let path = match args.get("path").and_then(Value::as_str) {
                Some(path) => path,
                None => return ToolResult::err("缺少必填参数 `path`"),
            };
            match resolve_path(path) {
                Ok(resolved) => run_file_edit_at(&resolved.absolute, &args),
                Err(err) => ToolResult::err(describe_path_error(&err)),
            }
        })
    }
}

/// 路径已解析后的编辑主体。
pub(crate) fn run_file_edit_at(abs_path: &Path, args: &Value) -> ToolResult {
    let Some(old_text) = args.get("old_text").and_then(Value::as_str) else {
        return ToolResult::err("缺少必填参数 `old_text`");
    };
    let Some(new_text) = args.get("new_text").and_then(Value::as_str) else {
        return ToolResult::err("缺少必填参数 `new_text`");
    };
    if old_text.is_empty() {
        return ToolResult::err("`old_text` 不能为空；要往文件里加内容请用 file_write 的 append");
    }
    let replace_all = args
        .get("replace_all")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    match fs::metadata(abs_path) {
        Ok(meta) if meta.is_dir() => {
            return ToolResult::err(format!("{} 是目录", abs_path.display()));
        }
        Ok(meta) if meta.len() > MAX_EDIT_BYTES => {
            return ToolResult::err(format!(
                "{} 有 {} 字节，超过可编辑上限 {MAX_EDIT_BYTES} 字节",
                abs_path.display(),
                meta.len()
            ));
        }
        Ok(_) => {}
        Err(err) => return ToolResult::err(format!("读取 {} 失败：{err}", abs_path.display())),
    }

    let original = match fs::read_to_string(abs_path) {
        Ok(text) => text,
        Err(err) => {
            return ToolResult::err(format!(
                "{} 不是可编辑的文本文件：{err}",
                abs_path.display()
            ));
        }
    };

    let hits = original.matches(old_text).count();
    if hits == 0 {
        return ToolResult::err(format!(
            "在 {} 里没找到 old_text。先用 file_read 看一眼实际内容——缩进、空格和换行都必须逐字节一致。",
            abs_path.display()
        ));
    }
    if hits > 1 && !replace_all {
        return ToolResult::err(format!(
            "old_text 在 {} 里出现了 {hits} 次，无法确定改哪一处。请扩大 old_text 带上足够的上下文；确实要全改才设 replace_all=true。",
            abs_path.display()
        ));
    }

    let updated = if replace_all {
        original.replace(old_text, new_text)
    } else {
        original.replacen(old_text, new_text, 1)
    };

    if let Err(err) = fs::write(abs_path, &updated) {
        return ToolResult::err(format!("写回 {} 失败：{err}", abs_path.display()));
    }

    let replaced = if replace_all { hits } else { 1 };
    ToolResult::ok(format!(
        "已修改 {}（替换 {replaced} 处，{} 行 → {} 行）",
        abs_path.display(),
        count_lines(&original),
        count_lines(&updated)
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replaces_single_occurrence() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.rs");
        fs::write(&path, "fn main() {\n    println!(\"旧\");\n}\n").unwrap();

        let result = run_file_edit_at(
            &path,
            &json!({ "old_text": "println!(\"旧\")", "new_text": "println!(\"新\")" }),
        );
        assert!(result.success, "{}", result.content);
        assert!(result.content.contains("替换 1 处"));
        assert!(fs::read_to_string(&path).unwrap().contains("新"));
    }

    #[test]
    fn ambiguous_match_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.txt");
        fs::write(&path, "x\nx\nx\n").unwrap();

        let result = run_file_edit_at(&path, &json!({ "old_text": "x", "new_text": "y" }));
        assert!(!result.success);
        assert!(result.content.contains("出现了 3 次"), "{}", result.content);
        assert_eq!(fs::read_to_string(&path).unwrap(), "x\nx\nx\n", "拒绝时不能动文件");
    }

    #[test]
    fn replace_all_opts_into_bulk_change() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.txt");
        fs::write(&path, "x\nx\nx\n").unwrap();

        let result = run_file_edit_at(
            &path,
            &json!({ "old_text": "x", "new_text": "y", "replace_all": true }),
        );
        assert!(result.success);
        assert!(result.content.contains("替换 3 处"));
        assert_eq!(fs::read_to_string(&path).unwrap(), "y\ny\ny\n");
    }

    #[test]
    fn missing_text_tells_the_model_to_read_first() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.txt");
        fs::write(&path, "实际内容\n").unwrap();

        let result = run_file_edit_at(&path, &json!({ "old_text": "凭印象写的", "new_text": "x" }));
        assert!(!result.success);
        assert!(result.content.contains("没找到"));
        assert!(result.content.contains("file_read"));
    }

    #[test]
    fn empty_new_text_deletes_the_segment() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.txt");
        fs::write(&path, "保留\n删掉这行\n保留\n").unwrap();

        let result = run_file_edit_at(&path, &json!({ "old_text": "删掉这行\n", "new_text": "" }));
        assert!(result.success);
        assert_eq!(fs::read_to_string(&path).unwrap(), "保留\n保留\n");
    }

    #[test]
    fn empty_old_text_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.txt");
        fs::write(&path, "x").unwrap();

        let result = run_file_edit_at(&path, &json!({ "old_text": "", "new_text": "y" }));
        assert!(!result.success);
        assert!(result.content.contains("file_write"));
    }
}
