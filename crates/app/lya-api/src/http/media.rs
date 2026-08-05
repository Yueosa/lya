//! 会话媒体端点：缓存并 Serving 聊天里的本地/远程图片、视频、音频。

use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use lya_llm::LlmClient;
use lya_media::{CategoryLimits, MediaBytes, MediaCacheError, MediaCategory};
use serde::Serialize;

use lya_hub::SessionHub;

use super::media_limits::{audio_limits, image_limits, video_limits};
use super::media_serve::{serve_ranged_bytes, serve_ranged_file};

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
///
/// 来源和落盘位置分开给：远程媒体的来源是 URL，而它在盘上留没留、留在哪儿、是硬链接
/// 还是独立拷贝，只有这里说了前端才看得见。
#[derive(Debug, Serialize)]
pub struct MediaMeta {
    kind: &'static str,
    filename: String,
    /// 本地媒体的源文件路径。
    source_path: Option<String>,
    /// 远程媒体的原始 URL。
    origin_url: Option<String>,
    /// 我们留的那一份在哪；没留则为空。
    retained_path: Option<String>,
    /// `hardlink`（与源文件共用空间）或 `copy`。
    retained_kind: Option<&'static str>,
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

/// 一类媒体的端点参数。
struct MediaKind {
    category: MediaCategory,
    /// URL 里的路径段，回显 `display_url` 用。
    segment: &'static str,
    limits: CategoryLimits,
    /// 播放器要拖进度条的类型走 Range。
    ranged: bool,
}

async fn session_media(
    State(hub): Hub,
    Path(session_id): Path<String>,
    Query(query): Query<MediaQuery>,
    headers: HeaderMap,
    MediaKind {
        category,
        segment,
        limits,
        ranged,
    }: MediaKind,
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
        let display_url = display_url(&session_id, segment, &query);
        let (retained_path, retained_kind) = match &cached.retained {
            Some(retained) => (
                Some(retained.path.to_string_lossy().into_owned()),
                Some(retained.kind.as_str()),
            ),
            None => (None, None),
        };
        return Json(MediaMeta {
            kind: cached.kind,
            filename: cached.filename,
            source_path: cached.source_path,
            origin_url: cached.origin_url,
            retained_path,
            retained_kind,
            display_url,
        })
        .into_response();
    }

    match cached.bytes {
        MediaBytes::File(path) if ranged => serve_ranged_file(&path, cached.mime, &headers).await,
        MediaBytes::File(path) => match std::fs::read(&path) {
            Ok(bytes) => whole_body(bytes, cached.mime),
            Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response(),
        },
        // 没留存的远程媒体只在内存里，Range 也在内存里切
        MediaBytes::Memory(bytes) if ranged => serve_ranged_bytes(bytes, cached.mime, &headers),
        MediaBytes::Memory(bytes) => whole_body(bytes, cached.mime),
    }
}

fn whole_body(bytes: Vec<u8>, mime: &'static str) -> Response {
    (
        [
            (header::CONTENT_TYPE, mime),
            (header::CACHE_CONTROL, "private, max-age=86400"),
        ],
        bytes,
    )
        .into_response()
}

/// 读取或留存一张会话图片。
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
        MediaKind {
            category: MediaCategory::Image,
            segment: "image",
            limits: image_limits(),
            ranged: false,
        },
    )
    .await
}

/// 读取或留存一段会话视频（支持 Range）。
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
        MediaKind {
            category: MediaCategory::Video,
            segment: "video",
            limits: video_limits(),
            ranged: true,
        },
    )
    .await
}

/// 读取或留存一段会话音频（支持 Range）。
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
        MediaKind {
            category: MediaCategory::Audio,
            segment: "audio",
            limits: audio_limits(),
            ranged: true,
        },
    )
    .await
}
