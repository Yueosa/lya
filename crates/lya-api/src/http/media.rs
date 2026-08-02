//! 会话媒体端点：缓存并 Serving 聊天里的本地/远程图片、视频、音频。

use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use lya_llm::LlmClient;
use lya_media::{CategoryLimits, MediaCacheError, MediaCategory};
use serde::Serialize;

use lya_hub::SessionHub;

use super::media_limits::{audio_limits, image_limits, video_limits};
use super::media_serve::serve_ranged_file;

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

fn display_url(session_id: &str, segment: &str, query: &MediaQuery) -> String {
    use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
    format!(
        "/api/sessions/{session_id}/media/{segment}?kind={}&src={}&token={}",
        utf8_percent_encode(&query.kind, NON_ALPHANUMERIC),
        utf8_percent_encode(&query.src, NON_ALPHANUMERIC),
        utf8_percent_encode(&query.token, NON_ALPHANUMERIC),
    )
}

async fn session_media(
    State(hub): Hub,
    Path(session_id): Path<String>,
    Query(query): Query<MediaQuery>,
    headers: HeaderMap,
    category: MediaCategory,
    segment: &'static str,
    limits: CategoryLimits,
    ranged: bool,
) -> Response {
    if query.token != hub.image_token() {
        return (StatusCode::FORBIDDEN, "令牌不对").into_response();
    }

    if hub.snapshot(&session_id).is_err() {
        return (StatusCode::NOT_FOUND, "会话不存在").into_response();
    }

    let cached = match query.kind.as_str() {
        "local" => lya_media::ensure_local(&session_id, &query.src, category, limits),
        "web" => {
            lya_media::ensure_web(
                &session_id,
                &query.src,
                category,
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
        return Json(MediaMeta {
            kind: cached.kind,
            filename: cached.filename,
            copy_path: cached.copy_path,
            copy_url: cached.copy_url,
            display_url: display_url(&session_id, segment, &query),
        })
        .into_response();
    }

    if ranged {
        return serve_ranged_file(&cached.path, cached.mime, &headers).await;
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

/// 读取或缓存一张会话图片。
pub async fn session_image(
    hub: Hub,
    Path(session_id): Path<String>,
    query: Query<MediaQuery>,
    headers: HeaderMap,
) -> Response {
    session_media(
        hub,
        Path(session_id),
        query,
        headers,
        MediaCategory::Image,
        "image",
        image_limits(),
        false,
    )
    .await
}

/// 读取或缓存一段会话视频（支持 Range）。
pub async fn session_video(
    hub: Hub,
    Path(session_id): Path<String>,
    query: Query<MediaQuery>,
    headers: HeaderMap,
) -> Response {
    session_media(
        hub,
        Path(session_id),
        query,
        headers,
        MediaCategory::Video,
        "video",
        video_limits(),
        true,
    )
    .await
}

/// 读取或缓存一段会话音频（支持 Range）。
pub async fn session_audio(
    hub: Hub,
    Path(session_id): Path<String>,
    query: Query<MediaQuery>,
    headers: HeaderMap,
) -> Response {
    session_media(
        hub,
        Path(session_id),
        query,
        headers,
        MediaCategory::Audio,
        "audio",
        audio_limits(),
        true,
    )
    .await
}
