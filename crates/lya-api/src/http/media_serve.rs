//! 带 HTTP Range 的文件响应（视频/音频播放需要）。

use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use axum::body::Body;
use axum::http::header::{
    ACCEPT_RANGES, CACHE_CONTROL, CONTENT_LENGTH, CONTENT_RANGE, CONTENT_TYPE, RANGE,
};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};

/// 解析 `bytes=` Range 头；非法或越界返回 `None`。
fn parse_range(header: &str, size: u64) -> Option<(u64, u64)> {
    let spec = header.strip_prefix("bytes=")?.trim();
    let (start_raw, end_raw) = spec.split_once('-')?;

    if start_raw.is_empty() {
        let suffix: u64 = end_raw.parse().ok()?;
        if suffix == 0 {
            return None;
        }
        let start = size.saturating_sub(suffix);
        return Some((start, size.saturating_sub(1)));
    }

    let start: u64 = start_raw.parse().ok()?;
    let end = if end_raw.is_empty() {
        size.saturating_sub(1)
    } else {
        end_raw.parse().ok()?
    };
    if start > end || end >= size {
        return None;
    }
    Some((start, end))
}

fn cache_control() -> HeaderValue {
    HeaderValue::from_static("private, max-age=86400")
}

/// 以整文件或 Range 片段响应；始终带 `Accept-Ranges: bytes`。
pub async fn serve_ranged_file(path: &Path, mime: &str, headers: &HeaderMap) -> Response {
    let file = match tokio::fs::File::open(path).await {
        Ok(file) => file,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return (StatusCode::NOT_FOUND, "不存在").into_response();
        }
        Err(err) => return (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response(),
    };

    let size = match file.metadata().await {
        Ok(meta) => meta.len(),
        Err(err) => return (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response(),
    };

    if let Some(range_header) = headers.get(RANGE).and_then(|value| value.to_str().ok()) {
        if let Some((start, end)) = parse_range(range_header, size) {
            let length = end - start + 1;
            let mut std_file = match std::fs::File::open(path) {
                Ok(file) => file,
                Err(err) => return (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response(),
            };
            if std_file.seek(SeekFrom::Start(start)).is_err() {
                return (StatusCode::INTERNAL_SERVER_ERROR, "seek 失败").into_response();
            }
            let mut buf = vec![0u8; length as usize];
            if std_file.read_exact(&mut buf).is_err() {
                return (StatusCode::INTERNAL_SERVER_ERROR, "read 失败").into_response();
            }

            let content_range = format!("bytes {start}-{end}/{size}");
            return match Response::builder()
                .status(StatusCode::PARTIAL_CONTENT)
                .header(CONTENT_TYPE, mime)
                .header(ACCEPT_RANGES, "bytes")
                .header(CONTENT_RANGE, content_range)
                .header(CONTENT_LENGTH, length)
                .header(CACHE_CONTROL, cache_control())
                .body(Body::from(buf))
            {
                Ok(response) => response,
                Err(err) => return (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response(),
            };
        }
    }

    match tokio::fs::read(path).await {
        Ok(bytes) => match Response::builder()
            .status(StatusCode::OK)
            .header(CONTENT_TYPE, mime)
            .header(ACCEPT_RANGES, "bytes")
            .header(CONTENT_LENGTH, size)
            .header(CACHE_CONTROL, cache_control())
            .body(Body::from(bytes))
        {
            Ok(response) => response,
            Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response(),
        },
        Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_common_ranges() {
        assert_eq!(parse_range("bytes=0-99", 1000), Some((0, 99)));
        assert_eq!(parse_range("bytes=500-", 1000), Some((500, 999)));
        assert_eq!(parse_range("bytes=-100", 1000), Some((900, 999)));
        assert_eq!(parse_range("bytes=2000-3000", 1000), None);
    }
}
