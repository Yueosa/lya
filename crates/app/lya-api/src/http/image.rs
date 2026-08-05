//! 本地图片端点：让浏览器能显示模型引用的本地图片。
//!
//! # 为什么要一个令牌
//!
//! 跨站守卫对「没带 `Origin` 的 GET」放行，因为 curl 和地址栏要能用。但
//! **`<img src>` 请求按规范就不带 `Origin`**，于是任何网页都能写
//! `<img src="http://127.0.0.1:51616/api/local-image?path=/home/你/照片/x.jpg">`。
//! 它读不到像素（跨域），却能通过加载成败探测文件是否存在、并拿到图片尺寸。
//!
//! 所以这个端点额外要一个启动时随机生成的令牌。令牌只通过 JSON 端点下发：
//! 跨域 `fetch` 一定带 `Origin`，会被守卫挡掉；跨域 `<script>` / `<img>` 又读不到
//! JSON 内容。恶意页面因此拿不到它。堵上这个口之后，整个 HTTP 面就没有例外了。
//!
//! # 路径规则比工具更严
//!
//! `file_read` 允许用 `/` 开头的绝对路径读到家目录之外，那是给模型用的、有模式
//! 权限兜底。这里是**浏览器能直接访问的 URL**，一旦放开就等于任意文件读取，
//! 所以只认家目录内，而且**解析符号链接之后再校验**——否则 `~/link -> /etc/shadow`
//! 就绕过去了。

use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::extract::{Query, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use lya_llm::LlmClient;
use serde::Deserialize;

use lya_hub::SessionHub;

use super::media_limits::image_limits;

type Hub = State<Arc<SessionHub<LlmClient>>>;

/// 请求参数。
#[derive(Debug, Deserialize)]
pub struct ImageQuery {
    /// 图片的绝对路径。
    pub path: String,
    /// 启动时下发的令牌。
    pub token: String,
}

/// 读取一张本地图片。
pub async fn local_image(State(hub): Hub, Query(query): Query<ImageQuery>) -> Response {
    if query.token != hub.image_token() {
        return (StatusCode::FORBIDDEN, "令牌不对").into_response();
    }

    let home = match std::env::var_os("HOME") {
        Some(home) => PathBuf::from(home),
        None => return (StatusCode::INTERNAL_SERVER_ERROR, "HOME 未设置").into_response(),
    };

    // 先解析符号链接再比对，否则家目录里放个链接就能指到任何地方
    let Ok(real) = std::fs::canonicalize(&query.path) else {
        return (StatusCode::NOT_FOUND, "文件不存在").into_response();
    };
    let Ok(real_home) = std::fs::canonicalize(&home) else {
        return (StatusCode::INTERNAL_SERVER_ERROR, "家目录不可访问").into_response();
    };
    if !real.starts_with(&real_home) {
        return (StatusCode::FORBIDDEN, "只能读取家目录内的图片").into_response();
    }

    let Some(mime) = mime_of(&real) else {
        return (StatusCode::UNSUPPORTED_MEDIA_TYPE, "不是支持的图片格式").into_response();
    };
    match std::fs::metadata(&real) {
        Ok(meta) if meta.len() > image_limits().max_bytes => {
            return (StatusCode::PAYLOAD_TOO_LARGE, "图片过大").into_response();
        }
        Ok(meta) if !meta.is_file() => {
            return (StatusCode::BAD_REQUEST, "不是文件").into_response();
        }
        Ok(_) => {}
        Err(_) => return (StatusCode::NOT_FOUND, "文件不存在").into_response(),
    }

    match std::fs::read(&real) {
        Ok(bytes) => (
            [
                (header::CONTENT_TYPE, mime),
                // 本地文件可能被改，别让浏览器长期缓存
                (header::CACHE_CONTROL, "no-cache"),
            ],
            bytes,
        )
            .into_response(),
        Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response(),
    }
}

/// 按扩展名给出 MIME；认不出就拒绝，不猜。
fn mime_of(path: &Path) -> Option<&'static str> {
    let extension = path.extension()?.to_str()?.to_lowercase();
    Some(match extension.as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        "tiff" | "tif" => "image/tiff",
        "ico" => "image/x-icon",
        "avif" => "image/avif",
        "svg" => "image/svg+xml",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_known_image_types_are_served() {
        assert_eq!(mime_of(Path::new("a.png")), Some("image/png"));
        assert_eq!(mime_of(Path::new("a.JPEG")), Some("image/jpeg"));
        // 认不出就拒绝，不去猜——否则等于开放任意文件下载
        assert_eq!(mime_of(Path::new("a.txt")), None);
        assert_eq!(mime_of(Path::new("a.so")), None);
        assert_eq!(mime_of(Path::new("noext")), None);
    }
}
