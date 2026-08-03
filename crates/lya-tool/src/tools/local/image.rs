//! `image_scan`：列出本地图片、查看详情、找出重复。
//!
//! **只读文件头，不解码。** 尺寸和格式从文件开头几十个字节就能读出来，扫一个
//! 上千张图的目录也很快；真去解码每一张会慢上两个数量级，而我们并不需要像素。
//!
//! 找重复只做 sha256 精确匹配。上一代还算了感知哈希来找「相似但不同」的图，
//! 但那必须完整解码加缩放，而且相似度阈值调松了误报、调紧了漏——精确重复
//! （同一张图存了两份）已经覆盖「清理家目录」的主要场景。

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::context::ToolCtx;
use crate::limits::image_scan::{DEFAULT_LIMIT, MAX_DEPTH, MAX_LIMIT};
use crate::meta::{ToolMeta, ToolResult};
use crate::permission::Permission;
use crate::tools::local::file::manage::human_size;
use crate::tools::local::file::write::describe_path_error;
use crate::tools::local::path::resolve_path;
use crate::traits::{Tool, ToolCallFuture};

/// 认得出的图片扩展名。先按扩展名筛一遍，省得对每个文件都去读头。
const EXTENSIONS: &[&str] = &[
    "jpg", "jpeg", "png", "gif", "webp", "bmp", "tiff", "tif", "ico", "avif", "heic", "heif",
];

/// `image_scan` 工具。
pub struct ImageScanTool {
    meta: ToolMeta,
    parameters: Value,
    prompt_hint: &'static str,
}

impl Default for ImageScanTool {
    fn default() -> Self {
        Self::new()
    }
}

impl ImageScanTool {
    /// 构造。
    pub fn new() -> Self {
        Self {
            meta: ToolMeta::new(
                "image_scan",
                "扫描图片",
                "列出目录下的图片或查看单张详情；可按内容找出完全重复的图",
                Permission::READ,
            ),
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "目录或单个图片文件。~/ 与相对路径基于家目录。"
                    },
                    "recursive": {
                        "type": "boolean",
                        "description": "是否递归子目录，默认 false。"
                    },
                    "find_duplicates": {
                        "type": "boolean",
                        "description": "按内容找出完全相同的图并分组，默认 false。会读取文件全部内容，目录大时较慢。"
                    },
                    "limit": {
                        "type": "integer",
                        "minimum": 1,
                        "description": "最多列出几张，默认 100，上限 1000。"
                    }
                },
                "required": ["path"],
                "additionalProperties": false
            }),
            prompt_hint: concat!(
                "使用 image_scan 了解本地图片：\n",
                "1) **返回的路径可以直接用 Markdown 图片语法引用**，例如 `![猫](/home/用户/图片/猫.jpg)`，",
                "界面会把它渲染出来。想给用户看图就这么写，不要只报路径。\n",
                "2) 只读文件头拿尺寸和格式，很快；find_duplicates 要读完整文件算哈希，大目录会慢，别顺手就开。\n",
                "3) 找重复只认**完全相同**的文件，改过尺寸或重新压缩过的算不同。\n",
                "4) 它不看图片内容——要知道图里画了什么，得用支持看图的模型。"
            ),
        }
    }
}

impl Tool for ImageScanTool {
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
            let Some(raw) = args.get("path").and_then(Value::as_str) else {
                return ToolResult::err("缺少必填参数 `path`");
            };
            match resolve_path(raw) {
                Ok(resolved) => run_image_scan_at(&resolved.absolute, &args),
                Err(err) => ToolResult::err(describe_path_error(&err)),
            }
        })
    }
}

/// 一张图的基本信息。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ImageEntry {
    pub path: PathBuf,
    pub width: usize,
    pub height: usize,
    pub format: String,
    pub bytes: u64,
}

/// 路径已解析后的主体。
pub(crate) fn run_image_scan_at(abs_path: &Path, args: &Value) -> ToolResult {
    if abs_path.is_file() {
        return match describe(abs_path) {
            Some(entry) => ToolResult::ok(render_single(&entry)),
            None => ToolResult::err(format!("{} 不是能识别的图片", abs_path.display())),
        };
    }
    if !abs_path.is_dir() {
        return ToolResult::err(format!("{} 不存在", abs_path.display()));
    }

    let recursive = args
        .get("recursive")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let limit = args
        .get("limit")
        .and_then(Value::as_u64)
        .map(|n| (n as usize).min(MAX_LIMIT))
        .unwrap_or(DEFAULT_LIMIT);

    let mut entries = Vec::new();
    collect(
        abs_path,
        recursive,
        if recursive { MAX_DEPTH } else { 1 },
        &mut entries,
    );
    entries.sort_by(|a, b| a.path.cmp(&b.path));

    if entries.is_empty() {
        return ToolResult::ok(format!("{} 下没有图片", abs_path.display()));
    }

    if args
        .get("find_duplicates")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return ToolResult::ok(render_duplicates(abs_path, &entries));
    }
    ToolResult::ok(render_list(abs_path, &entries, limit))
}

/// 递归收集图片。
fn collect(dir: &Path, recursive: bool, depth: usize, out: &mut Vec<ImageEntry>) {
    if depth == 0 {
        return;
    }
    let Ok(read) = fs::read_dir(dir) else {
        return;
    };
    for entry in read.flatten() {
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        // 不跟随符号链接，免得绕圈
        if file_type.is_dir() {
            if recursive && !file_type.is_symlink() {
                collect(&path, recursive, depth - 1, out);
            }
            continue;
        }
        if let Some(info) = describe(&path) {
            out.push(info);
        }
    }
}

/// 读一张图的头，拿尺寸与格式。
fn describe(path: &Path) -> Option<ImageEntry> {
    let extension = path.extension()?.to_str()?.to_lowercase();
    if !EXTENSIONS.contains(&extension.as_str()) {
        return None;
    }
    let size = imagesize::size(path).ok()?;
    let bytes = fs::metadata(path).ok()?.len();
    Some(ImageEntry {
        path: path.to_path_buf(),
        width: size.width,
        height: size.height,
        format: extension,
        bytes,
    })
}

fn render_single(entry: &ImageEntry) -> String {
    format!(
        "{}\n尺寸: {}×{}\n格式: {}\n大小: {}\n\n可以用 ![]({}) 让界面显示它。",
        entry.path.display(),
        entry.width,
        entry.height,
        entry.format,
        human_size(entry.bytes),
        entry.path.display()
    )
}

fn render_list(root: &Path, entries: &[ImageEntry], limit: usize) -> String {
    let shown = entries.len().min(limit);
    let mut out = format!(
        "{}（共 {} 张，列出 {shown} 张）\n",
        root.display(),
        entries.len()
    );
    for entry in entries.iter().take(limit) {
        let name = entry
            .path
            .strip_prefix(root)
            .unwrap_or(&entry.path)
            .display();
        out.push_str(&format!(
            "\n{name}  {}×{}  {}  {}",
            entry.width,
            entry.height,
            entry.format,
            human_size(entry.bytes)
        ));
    }
    if entries.len() > limit {
        out.push_str(&format!(
            "\n\n… 还有 {} 张未列出，缩小范围或调大 limit。",
            entries.len() - limit
        ));
    }
    out
}

/// 按内容分组，只报出现两次以上的。
fn render_duplicates(root: &Path, entries: &[ImageEntry]) -> String {
    let mut groups: BTreeMap<String, Vec<&ImageEntry>> = BTreeMap::new();
    for entry in entries {
        // 大小不同就不可能是同一份文件，先按大小分桶能省掉绝大多数哈希
        let Some(digest) = sha256_of(&entry.path) else {
            continue;
        };
        groups.entry(digest).or_default().push(entry);
    }
    let dupes: Vec<_> = groups.values().filter(|group| group.len() > 1).collect();

    if dupes.is_empty() {
        return format!(
            "{} 下的 {} 张图没有完全重复的。",
            root.display(),
            entries.len()
        );
    }
    let wasted: u64 = dupes
        .iter()
        .map(|group| group[0].bytes * (group.len() as u64 - 1))
        .sum();

    let mut out = format!(
        "{} 下发现 {} 组重复，多占了 {}：\n",
        root.display(),
        dupes.len(),
        human_size(wasted)
    );
    for (index, group) in dupes.iter().enumerate() {
        out.push_str(&format!(
            "\n第 {} 组（{} 份，各 {}）\n",
            index + 1,
            group.len(),
            human_size(group[0].bytes)
        ));
        for entry in group.iter() {
            out.push_str(&format!("  {}\n", entry.path.display()));
        }
    }
    out.push_str("\n删除多余的用 file_manage。");
    out
}

/// 算文件的 sha256。
fn sha256_of(path: &Path) -> Option<String> {
    let bytes = fs::read(path).ok()?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    Some(
        hasher
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 一张真实的 1×1 PNG，用来验证读头逻辑。
    const PNG_1X1: &[u8] = &[
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F,
        0x15, 0xC4, 0x89, 0x00, 0x00, 0x00, 0x0A, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x00,
        0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49,
        0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
    ];

    fn fixture() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.png"), PNG_1X1).unwrap();
        fs::write(dir.path().join("b.png"), PNG_1X1).unwrap();
        fs::write(dir.path().join("笔记.txt"), "不是图片").unwrap();
        fs::create_dir(dir.path().join("子目录")).unwrap();
        fs::write(dir.path().join("子目录/c.png"), PNG_1X1).unwrap();
        dir
    }

    #[test]
    fn lists_images_and_skips_other_files() {
        let dir = fixture();
        let result = run_image_scan_at(dir.path(), &json!({}));
        assert!(result.success, "{}", result.content);
        assert!(result.content.contains("a.png"));
        assert!(result.content.contains("1×1"));
        assert!(!result.content.contains("笔记.txt"));
        assert!(!result.content.contains("c.png"), "默认不进子目录");
    }

    #[test]
    fn recursive_reaches_subdirectories() {
        let dir = fixture();
        let result = run_image_scan_at(dir.path(), &json!({ "recursive": true }));
        assert!(result.content.contains("c.png"));
    }

    #[test]
    fn single_file_shows_details_and_markdown_hint() {
        let dir = fixture();
        let result = run_image_scan_at(&dir.path().join("a.png"), &json!({}));
        assert!(result.success);
        assert!(result.content.contains("尺寸: 1×1"));
        assert!(result.content.contains("格式: png"));
        // 提醒模型可以直接嵌进回复里
        assert!(result.content.contains("![]("));
    }

    #[test]
    fn duplicates_are_grouped_by_content() {
        let dir = fixture();
        let result = run_image_scan_at(dir.path(), &json!({ "find_duplicates": true }));
        assert!(result.content.contains("1 组重复"));
        assert!(result.content.contains("a.png"));
        assert!(result.content.contains("b.png"));
        assert!(result.content.contains("多占了"));
    }

    #[test]
    fn no_duplicates_says_so() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("only.png"), PNG_1X1).unwrap();
        let result = run_image_scan_at(dir.path(), &json!({ "find_duplicates": true }));
        assert!(result.content.contains("没有完全重复"));
    }

    #[test]
    fn limit_truncates_with_a_hint() {
        let dir = fixture();
        let result = run_image_scan_at(dir.path(), &json!({ "limit": 1 }));
        assert!(result.content.contains("还有 1 张未列出"));
    }

    #[test]
    fn non_image_file_is_reported() {
        let dir = fixture();
        let result = run_image_scan_at(&dir.path().join("笔记.txt"), &json!({}));
        assert!(!result.success);
        assert!(result.content.contains("不是能识别的图片"));
    }

    #[test]
    fn empty_directory_is_not_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let result = run_image_scan_at(dir.path(), &json!({}));
        assert!(result.success);
        assert!(result.content.contains("没有图片"));
    }
}
