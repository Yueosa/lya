//! 会话媒体端点：缓存并Serving 聊天里的本地/远程图片。

use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use lya_llm::LlmClient;
use serde::Serialize;

use lya_hub::SessionHub;
use lya_media::MediaCacheError;

use super::media_limits::image_limits;

type Hub = State<Arc<SessionHub<LlmClient>>>;

/// 查询参数。
#[derive(Debug, serde::Deserialize)]
pub struct MediaQuery {
    /// `local` 或 `web`。
    pub kind: String,
    /// 本地绝对路径或远程 URL。
    pub src: String,
    /// 与 bootstrap 一致的图片令牌。
    pub token: String,
    /// 为 `1` 时返回 JSON 元数据。
    #[serde(default)]
    pub meta: Option<String>,
}

/// 元数据 JSON。
#[derive(Debug, Serialize)]
pub struct MediaMeta {
    kind: &'static str,
    filename: String,
    copy_path: Option<String>,
    copy_url: Option<String>,
    display_url: String,
}

fn map_error(err: MediaCacheError) -> Response {
    match err {
        MediaCacheError::NotFound => (StatusCode::NOT_FOUND, "不存在").into_response(),
        MediaCacheError::Forbidden => (StatusCode::FORBIDDEN, "不允许").into_response(),
        MediaCacheError::Unsupported => {
            (StatusCode::UNSUPPORTED_MEDIA_TYPE, "不支持的格式").into_response()
        }
        MediaCacheError::TooLarge => (StatusCode::PAYLOAD_TOO_LARGE, "文件过大").into_response(),
        MediaCacheError::Invalid(message) => (StatusCode::BAD_REQUEST, message).into_response(),
        MediaCacheError::Io(err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response(),
        MediaCacheError::Network(message) => {
            (StatusCode::BAD_GATEWAY, message).into_response()
        }
    }
}

/// 读取或缓存一张会话图片。
pub async fn session_image(
    State(hub): Hub,
    Path(session_id): Path<String>,
    Query(query): Query<MediaQuery>,
) -> Response {
    if query.token != hub.image_token() {
        return (StatusCode::FORBIDDEN, "令牌不对").into_response();
    }

    // 会话必须存在
    if hub.snapshot(&session_id).is_err() {
        return (StatusCode::NOT_FOUND, "会话不存在").into_response();
    }

    let limits = image_limits();
    let cached = match query.kind.as_str() {
        "local" => lya_media::ensure_local(&session_id, &query.src, limits),
        "web" => {
            lya_media::ensure_web(
                &session_id,
                &query.src,
                hub.http(),
                hub.self_port(),
                limits,
            )
            .await
        }
        _ => return (StatusCode::BAD_REQUEST, "kind 必须是 local 或 web").into_response(),
    };

    let cached = match cached {
        Ok(value) => value,
        Err(err) => return map_error(err),
    };

    if query.meta.as_deref() == Some("1") {
        use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
        let display_url = format!(
            "/api/sessions/{session_id}/media/image?kind={}&src={}&token={}",
            utf8_percent_encode(&query.kind, NON_ALPHANUMERIC),
            utf8_percent_encode(&query.src, NON_ALPHANUMERIC),
            utf8_percent_encode(&query.token, NON_ALPHANUMERIC),
        );
        return Json(MediaMeta {
            kind: cached.kind,
            filename: cached.filename,
            copy_path: cached.copy_path,
            copy_url: cached.copy_url,
            display_url,
        })
        .into_response();
    }

    match std::fs::read(&cached.path) {
        Ok(bytes) => (
            [
                (header::CONTENT_TYPE, cached.mime),
                (header::CACHE_CONTROL, "private, max-age=86400"),
            ],
            bytes,
        )
            .into_response(),
        Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response(),
    }
}
