//! 内嵌 WebUI 静态资源（`web/dist`）。

use axum::body::Body;
use axum::extract::Request;
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use rust_embed::Embed;

// 相对本 crate 的目录。crate 按层分了组之后是 crates/app/lya-api/，所以要退三级
#[derive(Embed)]
#[folder = "../../../web/dist/"]
struct UiAssets;

/// 非 API 路径：返回内嵌的前端文件；未知路径回退到 `index.html`（SPA）。
pub async fn serve_ui(request: Request) -> Response {
    let path = request.uri().path().trim_start_matches('/');
    if path.starts_with("api/") {
        return StatusCode::NOT_FOUND.into_response();
    }
    serve_path(if path.is_empty() { "index.html" } else { path })
}

fn serve_path(path: &str) -> Response {
    match UiAssets::get(path) {
        Some(file) => asset_response(path, &file),
        None if path.contains('.') => StatusCode::NOT_FOUND.into_response(),
        None => UiAssets::get("index.html")
            .map(|file| asset_response("index.html", &file))
            .unwrap_or_else(|| StatusCode::NOT_FOUND.into_response()),
    }
}

fn asset_response(path: &str, file: &rust_embed::EmbeddedFile) -> Response {
    let mime = mime_type(path);
    Response::builder()
        .header(header::CONTENT_TYPE, mime)
        .header(header::CACHE_CONTROL, cache_control(path))
        .body(Body::from(file.data.to_vec()))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

fn mime_type(path: &str) -> &'static str {
    if path.ends_with(".html") {
        "text/html; charset=utf-8"
    } else if path.ends_with(".js") {
        "text/javascript; charset=utf-8"
    } else if path.ends_with(".css") {
        "text/css; charset=utf-8"
    } else if path.ends_with(".png") {
        "image/png"
    } else if path.ends_with(".jpg") || path.ends_with(".jpeg") {
        "image/jpeg"
    } else if path.ends_with(".svg") {
        "image/svg+xml"
    } else if path.ends_with(".woff2") {
        "font/woff2"
    } else if path.ends_with(".ico") {
        "image/x-icon"
    } else {
        "application/octet-stream"
    }
}

fn cache_control(path: &str) -> &'static str {
    if path.starts_with("assets/") {
        "public, max-age=31536000, immutable"
    } else {
        "no-cache"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dist_index_is_embedded() {
        assert!(UiAssets::get("index.html").is_some());
    }
}
