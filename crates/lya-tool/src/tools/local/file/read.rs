//! `file_read`：读取本地文本文件（行范围 / 关键词检索）。
//!
//! # 模式
//!
//! - **范围模式**（默认）：可选 `start`/`end`（1-based，含端点）；
//!   未指定时读取全文，但受行数/字节上限约束。
//! - **检索模式**：提供 `search` 后生效；支持子串或正则、正/反向、
//!   命中前后文行数。

use std::fs;
use std::io::{BufRead, BufReader};
use std::path::Path;

use regex::{Regex, RegexBuilder};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::context::ToolCtx;
use crate::limits::file_read::{HARD_MAX_BYTES, MAX_FULL_BYTES, MAX_FULL_LINES};
use crate::meta::{ToolMeta, ToolResult};
use crate::permission::Permission;
use crate::tools::local::path::{resolve_path, PathError};
use crate::traits::{Tool, ToolCallFuture};

/// `file_read` 工具。
pub struct FileReadTool {
    /// 静态 meta。
    meta: ToolMeta,
    /// OpenAI `parameters` JSON Schema。
    parameters: Value,
    /// 用法说明。数值从 `limits.rs` 取，别在文案里手写。
    prompt_hint: String,
}

impl FileReadTool {
    /// 构造工具实例。
    pub fn new() -> Self {
        Self {
            meta: ToolMeta::new(
                "file_read",
                "读取文件",
                "读取本地文本文件：可按行范围截取，或按关键词/正则检索并返回命中处上下文",
                Permission::READ,
            ),
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "文件路径。~/ 或相对路径基于家目录；以 / 开头为绝对路径（可访问非家目录）。禁止用 ../ 从家目录相对路径逃逸。"
                    },
                    "start": {
                        "type": "integer",
                        "minimum": 1,
                        "description": "范围模式：起始行号（从 1 起，含）。与 search 同时出现时忽略，走检索模式。"
                    },
                    "end": {
                        "type": "integer",
                        "minimum": 1,
                        "description": "范围模式：结束行号（含）。"
                    },
                    "search": {
                        "type": "string",
                        "description": "检索模式：要查找的关键词或正则。提供后优先走检索，忽略 start/end。"
                    },
                    "regex": {
                        "type": "boolean",
                        "description": "search 是否按正则解释。默认 false（纯子串）。"
                    },
                    "ignore_case": {
                        "type": "boolean",
                        "description": "检索是否忽略大小写。默认 false。"
                    },
                    "reverse": {
                        "type": "boolean",
                        "description": "是否从文件尾向前找。默认 false。"
                    },
                    "max_matches": {
                        "type": "integer",
                        "minimum": 1,
                        "description": "最多返回多少条命中。默认 5。"
                    },
                    "context_before": {
                        "type": "integer",
                        "minimum": 0,
                        "description": "命中行之前额外返回的行数。默认 3。"
                    },
                    "context_after": {
                        "type": "integer",
                        "minimum": 0,
                        "description": "命中行之后额外返回的行数。默认 3。"
                    }
                },
                "required": ["path"],
                "additionalProperties": false
            }),
            prompt_hint: format!(
                concat!(
                    "使用 file_read 读取本地文本：\n",
                    "1) 大文件先用 search（关键词/正则）或 start/end 行范围，不要无脑全量读取。\n",
                    "2) 未指定范围时最多返回约 {} 行 / {} KiB，超出会截断并标记 truncated。\n",
                    "3) 路径：相对路径与 ~/ 基于家目录；需要访问非家目录必须用 / 开头的绝对路径；",
                    "用 ../ 从家目录相对逃逸会被拒绝。\n",
                    "4) 二进制文件会被拒绝，只返回元信息说明。\n",
                    "5) 同一段内容没有变化就不要重复读；但你自己改过文件之后要重读确认结果。"
                ),
                MAX_FULL_LINES,
                MAX_FULL_BYTES / 1024
            ),
        }
    }
}

impl Default for FileReadTool {
    fn default() -> Self {
        Self::new()
    }
}

impl Tool for FileReadTool {
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
        Box::pin(async move { run_file_read(args) })
    }
}

/// LLM 传入的参数。
#[derive(Debug, Deserialize)]
struct FileReadArgs {
    /// 文件路径。
    path: String,
    /// 起始行（1-based）。
    #[serde(default)]
    start: Option<u32>,
    /// 结束行（含）。
    #[serde(default)]
    end: Option<u32>,
    /// 检索词 / 正则。
    #[serde(default)]
    search: Option<String>,
    /// 是否正则。
    #[serde(default)]
    regex: bool,
    /// 忽略大小写。
    #[serde(default)]
    ignore_case: bool,
    /// 反向检索。
    #[serde(default)]
    reverse: bool,
    /// 最多返回命中数。
    #[serde(default)]
    max_matches: Option<u32>,
    /// 上文行数。
    #[serde(default)]
    context_before: Option<u32>,
    /// 下文行数。
    #[serde(default)]
    context_after: Option<u32>,
}

/// 同步执行（文件 IO 不重，放在 async 里也可接受；以后可 `spawn_blocking`）。
fn run_file_read(args: Value) -> ToolResult {
    let args: FileReadArgs = match serde_json::from_value(args) {
        Ok(a) => a,
        Err(err) => return ToolResult::err(format!("invalid arguments: {err}")),
    };

    let resolved = match resolve_path(&args.path) {
        Ok(r) => r,
        Err(PathError::HomeEscape) => {
            return ToolResult::err(PathError::HomeEscape.to_string());
        }
        Err(err) => return ToolResult::err(err.to_string()),
    };

    let path = resolved.absolute;
    let meta_stat = match file_stat(&path) {
        Ok(m) => m,
        Err(err) => return ToolResult::err(err),
    };

    if meta_stat.is_dir {
        return ToolResult::err(format!("path is a directory: {}", path.display()));
    }
    if meta_stat.size > HARD_MAX_BYTES {
        return ToolResult::err(format!(
            "file too large ({} bytes > {} hard limit); use a smaller file or split",
            meta_stat.size, HARD_MAX_BYTES
        ));
    }

    let lines = match read_text_lines(&path) {
        Ok(lines) => lines,
        Err(err) => return ToolResult::err(err),
    };

    let file_meta = json!({
        "name": path.file_name().and_then(|s| s.to_str()).unwrap_or(""),
        "path": path.display().to_string(),
        "size": meta_stat.size,
        "lines": lines.len(),
    });

    if let Some(search) = args.search.as_ref().filter(|s| !s.is_empty()) {
        return search_mode(&args, search, &lines, file_meta);
    }
    range_mode(&args, &lines, file_meta)
}

/// 范围读取。
fn range_mode(args: &FileReadArgs, lines: &[String], mut file_meta: Value) -> ToolResult {
    let total = lines.len();
    let (mut start, mut end, mut truncated) = match (args.start, args.end) {
        (None, None) => {
            // 全文，受上限约束
            let mut end = total;
            let mut truncated = false;
            let mut bytes = 0usize;
            let mut cut = end;
            for (i, line) in lines.iter().enumerate() {
                bytes += line.len() + 1;
                if i + 1 > MAX_FULL_LINES || bytes > MAX_FULL_BYTES {
                    cut = i;
                    truncated = true;
                    break;
                }
            }
            if truncated {
                end = cut;
            }
            (1usize, end, truncated)
        }
        (Some(s), None) => {
            let start = s as usize;
            (start, total, false)
        }
        (None, Some(e)) => (1usize, e as usize, false),
        (Some(s), Some(e)) => (s as usize, e as usize, false),
    };

    if start == 0 {
        start = 1;
    }
    if end < start {
        return ToolResult::err(format!("invalid range: start={start} > end={end}"));
    }
    if start > total {
        return ToolResult::err(format!(
            "start line {start} beyond end of file ({total} lines)"
        ));
    }
    end = end.min(total);

    // 显式范围也做字节/行保护，避免一次 dump 过大
    let mut out_bytes = 0usize;
    let mut actual_end = end;
    for (idx, line) in lines.iter().enumerate().take(end).skip(start - 1) {
        out_bytes += line.len() + 1;
        let line_no = idx + 1;
        if line_no - start + 1 > MAX_FULL_LINES || out_bytes > MAX_FULL_BYTES {
            actual_end = line_no - 1;
            truncated = true;
            break;
        }
    }
    if actual_end < start {
        actual_end = start - 1;
    }

    let content = format_lines(lines, start, actual_end);
    if let Some(obj) = file_meta.as_object_mut() {
        obj.insert("truncated".into(), json!(truncated));
    }

    let body = json!({
        "meta": file_meta,
        "mode": "range",
        "range": { "start": start, "end": actual_end },
        "content": content,
    });
    ToolResult::ok(body.to_string())
}

/// 检索模式。
fn search_mode(
    args: &FileReadArgs,
    search: &str,
    lines: &[String],
    mut file_meta: Value,
) -> ToolResult {
    let max_matches = args.max_matches.unwrap_or(5).max(1) as usize;
    let before = args.context_before.unwrap_or(3) as usize;
    let after = args.context_after.unwrap_or(3) as usize;

    let matcher = match build_matcher(search, args.regex, args.ignore_case) {
        Ok(m) => m,
        Err(err) => return ToolResult::err(err),
    };

    let total = lines.len();
    let mut hit_indices: Vec<usize> = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        if matcher.is_match(line) {
            hit_indices.push(i);
        }
    }

    let match_count = hit_indices.len();
    if args.reverse {
        hit_indices.reverse();
    }
    hit_indices.truncate(max_matches);

    let mut matches = Vec::new();
    for &idx in &hit_indices {
        let line_no = idx + 1;
        let ctx_start = line_no.saturating_sub(before).max(1);
        let ctx_end = (line_no + after).min(total);
        matches.push(json!({
            "line": line_no,
            "text": lines[idx],
            "context": {
                "start": ctx_start,
                "end": ctx_end,
                "content": format_lines(lines, ctx_start, ctx_end),
            }
        }));
    }

    if let Some(obj) = file_meta.as_object_mut() {
        obj.insert("truncated".into(), json!(false));
    }

    let body = json!({
        "meta": file_meta,
        "mode": "search",
        "query": {
            "search": search,
            "regex": args.regex,
            "ignore_case": args.ignore_case,
            "reverse": args.reverse,
        },
        "match_count": match_count,
        "matches": matches,
    });
    ToolResult::ok(body.to_string())
}

/// 匹配器：子串或正则。
enum Matcher {
    /// 字面量子串。
    Substr {
        /// 模式。
        needle: String,
        /// 忽略大小写时预先小写化 needle。
        ignore_case: bool,
    },
    /// 正则。
    Regex(Regex),
}

impl Matcher {
    /// 行是否命中。
    fn is_match(&self, line: &str) -> bool {
        match self {
            Self::Substr {
                needle,
                ignore_case,
            } => {
                if *ignore_case {
                    line.to_lowercase().contains(needle)
                } else {
                    line.contains(needle)
                }
            }
            Self::Regex(re) => re.is_match(line),
        }
    }
}

/// 构造匹配器。
fn build_matcher(search: &str, is_regex: bool, ignore_case: bool) -> Result<Matcher, String> {
    if is_regex {
        let re = RegexBuilder::new(search)
            .case_insensitive(ignore_case)
            .build()
            .map_err(|err| format!("invalid regex: {err}"))?;
        Ok(Matcher::Regex(re))
    } else {
        let needle = if ignore_case {
            search.to_lowercase()
        } else {
            search.to_string()
        };
        Ok(Matcher::Substr {
            needle,
            ignore_case,
        })
    }
}

/// 文件粗略元信息。
struct FileStat {
    /// 字节大小。
    size: u64,
    /// 是否目录。
    is_dir: bool,
}

/// 取 size / 是否目录。
fn file_stat(path: &Path) -> Result<FileStat, String> {
    let meta = fs::metadata(path).map_err(|err| format!("stat {}: {err}", path.display()))?;
    Ok(FileStat {
        size: meta.len(),
        is_dir: meta.is_dir(),
    })
}

/// 按行读取文本；含 NUL 则视为二进制拒绝。
fn read_text_lines(path: &Path) -> Result<Vec<String>, String> {
    let file = fs::File::open(path).map_err(|err| format!("open {}: {err}", path.display()))?;
    let reader = BufReader::new(file);
    let mut lines = Vec::new();
    for (i, item) in reader.split(b'\n').enumerate() {
        let bytes = item.map_err(|err| format!("read {}: {err}", path.display()))?;
        if bytes.contains(&0) {
            return Err(format!(
                "binary file refused (NUL at/near line {}); only text is supported",
                i + 1
            ));
        }
        let line = String::from_utf8_lossy(&bytes).into_owned();
        // 去掉可能残留的 `\r`
        let line = line.strip_suffix('\r').unwrap_or(&line).to_string();
        lines.push(line);
    }
    Ok(lines)
}

/// 格式化 `[start, end]` 行（1-based，含），带行号前缀。
fn format_lines(lines: &[String], start: usize, end: usize) -> String {
    if end < start || start == 0 || start > lines.len() {
        return String::new();
    }
    let end = end.min(lines.len());
    let mut out = String::new();
    for (idx, line) in lines.iter().enumerate().take(end).skip(start - 1) {
        let n = idx + 1;
        out.push_str(&format!("{n:6}|{line}\n"));
    }
    out
}

/// 测试入口：用绝对路径直接测读写逻辑。
#[cfg(test)]
fn run_file_read_at(abs_path: &Path, mut args: Value) -> ToolResult {
    if let Some(obj) = args.as_object_mut() {
        obj.insert("path".into(), json!(abs_path.display().to_string()));
    }
    run_file_read(args)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::local::path::{resolve_path_with_home, PathError};
    use std::sync::Arc;

    #[test]
    fn range_and_search() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("demo.txt");
        fs::write(
            &file,
            "alpha\nbeta keyword here\ngamma\nkeyword again\nomega\n",
        )
        .unwrap();

        let range = run_file_read_at(&file, json!({ "start": 2, "end": 3 }));
        assert!(range.success, "{}", range.content);
        let v: Value = serde_json::from_str(&range.content).unwrap();
        assert_eq!(v["mode"], "range");
        assert!(v["content"].as_str().unwrap().contains("beta"));

        let search = run_file_read_at(
            &file,
            json!({
                "search": "keyword",
                "context_before": 1,
                "context_after": 1,
                "max_matches": 10
            }),
        );
        assert!(search.success, "{}", search.content);
        let v: Value = serde_json::from_str(&search.content).unwrap();
        assert_eq!(v["mode"], "search");
        assert_eq!(v["match_count"], 2);
        assert_eq!(v["matches"][0]["line"], 2);

        let rev = run_file_read_at(
            &file,
            json!({
                "search": "keyword",
                "reverse": true,
                "max_matches": 1
            }),
        );
        let v: Value = serde_json::from_str(&rev.content).unwrap();
        assert_eq!(v["matches"][0]["line"], 4);
    }

    #[test]
    fn regex_search() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("r.txt");
        fs::write(&file, "foo1\nbar2\nfoo99\n").unwrap();
        let r = run_file_read_at(
            &file,
            json!({
                "search": r"foo\d+",
                "regex": true
            }),
        );
        assert!(r.success, "{}", r.content);
        let v: Value = serde_json::from_str(&r.content).unwrap();
        assert_eq!(v["match_count"], 2);
    }

    #[test]
    fn home_escape_message() {
        let err = resolve_path_with_home("../x", Path::new("/home/demo")).unwrap_err();
        assert_eq!(err, PathError::HomeEscape);
        assert!(err.to_string().contains("家目录逃逸"));
    }

    #[test]
    fn tool_meta() {
        let tool = Arc::new(FileReadTool::new());
        assert_eq!(tool.meta().name, "file_read");
        assert_eq!(tool.meta().raw_name, "读取文件");
        assert_eq!(tool.meta().prmt, Permission::READ);
    }
}
