//! 主题的本地素材：`~/.lya/theme/{主题}/{分类}/`。
//!
//! ## 为什么不内嵌
//!
//! 蔚蓝档案主题要用游戏的加载图和记忆大厅 CG，后者是视频，一个几十 MB。内嵌进
//! 二进制的话每加一张图都要重新构建、分发包也跟着涨——`web/dist` 那种几百 KB 的
//! 静态资源可以内嵌，几百 MB 的素材不行。
//!
//! 所以素材放数据目录，用户自己往里丢文件，服务端只负责**列出来**和**发出去**。
//! 目录不存在就当空的，不报错：主题本来就该在没有素材时也能用。
//!
//! ## 两道闸门
//!
//! - **令牌**：和本地图片端点同一个，进程启动时随机生成。泄露出去的链接活不过一次重启
//! - **路径**：`kind` 与 `name` 都只允许一段安全字符，且取到的真实路径必须仍在素材目录
//!   之内。先 `canonicalize` 再比前缀——只做字符串检查的话，目录里放一个软链就能指到
//!   任何地方

use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use lya_llm::LlmClient;
use lya_hub::SessionHub;
use serde::{Deserialize, Serialize};

use super::media_serve::serve_ranged_file;

type Hub = State<Arc<SessionHub<LlmClient>>>;

/// 认得的素材分类。写成白名单而不是「随便一段路径」，省掉一整类穿越问题。
const KINDS: &[&str] = &["home", "cg"];

/// 能发出去的扩展名与 MIME。不在表里的一律不列也不发——素材目录是用户的，
/// 里面可能有 `.psd`、`.txt` 甚至别的东西，没必要经由 HTTP 暴露。
const TYPES: &[(&str, &str)] = &[
    ("png", "image/png"),
    ("jpg", "image/jpeg"),
    ("jpeg", "image/jpeg"),
    ("webp", "image/webp"),
    ("avif", "image/avif"),
    ("gif", "image/gif"),
    ("mp4", "video/mp4"),
    ("webm", "video/webm"),
];

fn mime_of(path: &Path) -> Option<&'static str> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    TYPES
        .iter()
        .find(|(name, _)| *name == ext)
        .map(|(_, mime)| *mime)
}

/// 主题 id / 分类 / 文件名允许的字符。
///
/// 不含 `/`、`\` 和 `.`，所以 `..` 和绝对路径都过不来。文件名那一段要留 `.` 给扩展名，
/// 单独放宽。
fn safe_segment(value: &str, allow_dot: bool) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || (allow_dot && c == '.'))
        && !value.contains("..")
}

/// `~/.lya/theme/{theme}/{kind}/`。
fn kind_dir(theme: &str, kind: &str) -> Option<PathBuf> {
    if !safe_segment(theme, false) || !KINDS.contains(&kind) {
        return None;
    }
    let root = lya_base::data_root().ok()?;
    Some(root.join("theme").join(theme).join(kind))
}

/// 列表项。前端拿到名字后自己拼取文件的地址。
#[derive(Debug, Serialize)]
pub struct ThemeAsset {
    /// 文件名，含扩展名。
    name: String,
    /// `image` 或 `video`——前端要据此决定用 `<img>` 还是 `<video>`。
    media: &'static str,
    /// 字节数，给界面提示用。
    bytes: u64,
}

/// `GET /api/theme/{theme}/assets?kind=home` 的响应。
#[derive(Debug, Serialize)]
pub struct ThemeAssetList {
    /// 素材目录的绝对路径，照原样给出来，好让用户知道该往哪儿丢文件。
    dir: String,
    /// 目录当前存不存在。
    exists: bool,
    assets: Vec<ThemeAsset>,
}

/// 列目录参数。
#[derive(Debug, Deserialize)]
pub struct ListQuery {
    /// `home` 或 `cg`。
    kind: String,
}

/// 列出某个分类下的素材。
///
/// 不需要令牌：这里只有文件名和大小，且路径固定在数据目录之内，泄露出去也拿不到内容。
/// 真正要闸门的是取文件。
pub async fn list(
    axum::extract::Path(theme): axum::extract::Path<String>,
    Query(query): Query<ListQuery>,
) -> Response {
    let Some(dir) = kind_dir(&theme, &query.kind) else {
        return (StatusCode::BAD_REQUEST, "主题或分类不合法").into_response();
    };

    let mut assets = Vec::new();
    let exists = dir.is_dir();
    if exists {
        let mut entries = match std::fs::read_dir(&dir) {
            Ok(entries) => entries.flatten().collect::<Vec<_>>(),
            Err(err) => return (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response(),
        };
        // 按名字排序：用户靠文件名控制顺序（01_、02_…），随机顺序会让轮播每次都不一样
        entries.sort_by_key(|entry| entry.file_name());

        for entry in entries {
            let path = entry.path();
            let Some(mime) = mime_of(&path) else { continue };
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            let Ok(meta) = entry.metadata() else { continue };
            if !meta.is_file() {
                continue;
            }
            assets.push(ThemeAsset {
                name: name.to_string(),
                media: if mime.starts_with("video/") {
                    "video"
                } else {
                    "image"
                },
                bytes: meta.len(),
            });
        }
    }

    Json(ThemeAssetList {
        dir: dir.display().to_string(),
        exists,
        assets,
    })
    .into_response()
}

/// 取文件参数。
#[derive(Debug, Deserialize)]
pub struct AssetQuery {
    /// `home` 或 `cg`。
    kind: String,
    /// 文件名，来自列表接口。
    name: String,
    /// 启动时下发的令牌。
    token: String,
}

/// 发一个素材文件；走 Range，视频才拖得动进度条。
pub async fn asset(
    State(hub): Hub,
    axum::extract::Path(theme): axum::extract::Path<String>,
    Query(query): Query<AssetQuery>,
    headers: HeaderMap,
) -> Response {
    if query.token != hub.image_token() {
        return (StatusCode::FORBIDDEN, "令牌不对").into_response();
    }
    let Some(dir) = kind_dir(&theme, &query.kind) else {
        return (StatusCode::BAD_REQUEST, "主题或分类不合法").into_response();
    };
    if !safe_segment(&query.name, true) {
        return (StatusCode::BAD_REQUEST, "文件名不合法").into_response();
    }

    // 先解析软链再比对：目录里放一个链接就能指到家目录外，光看字符串是拦不住的
    let Ok(real) = std::fs::canonicalize(dir.join(&query.name)) else {
        return (StatusCode::NOT_FOUND, "不存在").into_response();
    };
    let Ok(real_dir) = std::fs::canonicalize(&dir) else {
        return (StatusCode::NOT_FOUND, "素材目录不存在").into_response();
    };
    if !real.starts_with(&real_dir) {
        return (StatusCode::FORBIDDEN, "只能读素材目录内的文件").into_response();
    }
    let Some(mime) = mime_of(&real) else {
        return (StatusCode::UNSUPPORTED_MEDIA_TYPE, "不支持的格式").into_response();
    };

    serve_ranged_file(&real, mime, &headers).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segments_reject_traversal_and_separators() {
        assert!(safe_segment("ba", false));
        assert!(safe_segment("01_rain.jpg", true));
        // 不允许分隔符与父目录
        assert!(!safe_segment("..", true));
        assert!(!safe_segment("../etc", true));
        assert!(!safe_segment("a/b", true));
        assert!(!safe_segment("a\\b", true));
        assert!(!safe_segment("", true));
        // 主题那一段不留 `.`，否则 `.` 与 `..` 都要另外排除
        assert!(!safe_segment("a.b", false));
        // 藏在中间的 `..` 也不行
        assert!(!safe_segment("a..b", true));
    }

    #[test]
    fn only_known_kinds_resolve() {
        assert!(kind_dir("ba", "home").is_some());
        assert!(kind_dir("ba", "cg").is_some());
        assert!(kind_dir("ba", "etc").is_none());
        assert!(kind_dir("../..", "home").is_none());
    }

    #[test]
    fn mime_follows_extension_case_insensitively() {
        assert_eq!(mime_of(Path::new("a.PNG")), Some("image/png"));
        assert_eq!(mime_of(Path::new("a.mp4")), Some("video/mp4"));
        assert_eq!(mime_of(Path::new("a.psd")), None);
        assert_eq!(mime_of(Path::new("noext")), None);
    }
}
