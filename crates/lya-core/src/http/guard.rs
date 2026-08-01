//! 跨站防护。
//!
//! lya 在 `127.0.0.1` 上不带鉴权地跑着，而模型手里有 `web_fetch` 和 `bash`。
//! 只要浏览器允许，**任何网页都能让你的浏览器向 `localhost:51616` 发请求**
//! （CSRF / DNS rebinding），从而借你的手操纵 lya 读文件、跑命令。
//!
//! 本地应用不该逼用户登录，所以这里不做鉴权，而是校验来源：
//!
//! - 带了 `Origin` 的，必须是本机回环地址
//! - **没带 `Origin` 的写请求一律拒绝**（浏览器发起的跨站表单提交就属于这类；
//!   `curl` 想用得自己加一个头，这点不便换来的是明确的边界）
//! - `GET` 不带 `Origin` 放行，方便直接用浏览器地址栏或 `curl` 查看

use axum::extract::Request;
use axum::http::{Method, StatusCode, header};
use axum::middleware::Next;
use axum::response::Response;

/// 校验请求来源。
pub async fn same_origin(request: Request, next: Next) -> Result<Response, StatusCode> {
    let origin = request
        .headers()
        .get(header::ORIGIN)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);

    let safe = match origin.as_deref() {
        Some(origin) => is_loopback(origin),
        // 只读请求允许没有来源信息；写请求必须自报家门
        None => matches!(*request.method(), Method::GET | Method::HEAD),
    };
    if !safe {
        return Err(StatusCode::FORBIDDEN);
    }
    Ok(next.run(request).await)
}

/// 判断来源是不是本机回环。
fn is_loopback(origin: &str) -> bool {
    let Some(rest) = origin
        .strip_prefix("http://")
        .or_else(|| origin.strip_prefix("https://"))
    else {
        return false;
    };
    // 去掉端口再比对主机名，端口是几都行——用户可能因为占用而换过端口。
    // IPv6 的主机名裹在方括号里，不能按冒号切。
    let host = if rest.starts_with('[') {
        match rest.find(']') {
            Some(end) => &rest[1..end],
            None => return false,
        }
    } else {
        rest.split(':').next().unwrap_or(rest)
    };
    matches!(host, "127.0.0.1" | "localhost" | "::1")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loopback_origins_pass() {
        assert!(is_loopback("http://127.0.0.1:51616"));
        assert!(is_loopback("http://localhost:5173"));
        assert!(is_loopback("http://[::1]:51616"));
        assert!(is_loopback("http://localhost"));
    }

    #[test]
    fn outside_origins_are_rejected() {
        assert!(!is_loopback("https://evil.example.com"));
        // 这类拼接是常见的绕过尝试
        assert!(!is_loopback("http://127.0.0.1.evil.com"));
        assert!(!is_loopback("http://localhost.evil.com"));
        assert!(!is_loopback("file://"));
        assert!(!is_loopback("null"));
    }
}
