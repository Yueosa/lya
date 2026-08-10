//! 跨站防护。
//!
//! lya 在 `127.0.0.1` 上不带鉴权地跑着，而模型手里有 `web_fetch` 和 `bash`。
//! 只要浏览器允许，**任何网页都能让你的浏览器向 `localhost:51616` 发请求**
//! （CSRF / DNS rebinding），从而借你的手操纵 lya 读文件、跑命令。
//!
//! 本地应用不该逼用户登录，所以这里不做鉴权，而是校验来源：
//!
//! - 带了 `Origin` 的，必须是本机回环地址，或在 `core.toml` 的 `trusted_hosts` 里
//! - **没带 `Origin` 的写请求一律拒绝**（浏览器发起的跨站表单提交就属于这类；
//!   `curl` 想用得自己加一个头，这点不便换来的是明确的边界）
//! - `GET` 不带 `Origin` 放行，方便直接用浏览器地址栏或 `curl` 查看

use std::sync::Arc;

use axum::extract::Request;
use axum::http::{Method, StatusCode, header};
use axum::middleware::Next;
use axum::response::Response;

/// 启动时从 `core.toml` 读入的 Origin 白名单补充项。
#[derive(Clone, Default)]
pub struct Policy {
    trusted_hosts: Arc<[String]>,
}

impl Policy {
    /// 由装配处根据 `[server].trusted_hosts` 构造。
    pub fn new(trusted_hosts: Vec<String>) -> Self {
        Self {
            trusted_hosts: trusted_hosts.into(),
        }
    }
}

/// 校验请求来源。
pub async fn same_origin(
    request: Request,
    next: Next,
    policy: Policy,
) -> Result<Response, StatusCode> {
    let origin = request
        .headers()
        .get(header::ORIGIN)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);

    let safe = match origin.as_deref() {
        Some(origin) => is_allowed_origin(origin, &policy.trusted_hosts),
        // 只读请求允许没有来源信息；写请求必须自报家门
        None => matches!(*request.method(), Method::GET | Method::HEAD),
    };
    if !safe {
        return Err(StatusCode::FORBIDDEN);
    }
    Ok(next.run(request).await)
}

/// 判断来源是否在白名单内。
fn is_allowed_origin(origin: &str, trusted_hosts: &[String]) -> bool {
    let Some(host) = origin_host(origin) else {
        return false;
    };
    if is_loopback_host(host) {
        return true;
    }
    trusted_hosts.iter().any(|trusted| trusted == host)
}

/// 从 `http(s)://host:port` 取出主机名（IPv6 含方括号，不含端口）。
fn origin_host(origin: &str) -> Option<&str> {
    let rest = origin
        .strip_prefix("http://")
        .or_else(|| origin.strip_prefix("https://"))?;
    if rest.starts_with('[') {
        let end = rest.find(']')?;
        Some(&rest[1..end])
    } else {
        Some(rest.split(':').next().unwrap_or(rest))
    }
}

/// 判断主机名是不是本机回环。
fn is_loopback_host(host: &str) -> bool {
    matches!(host, "127.0.0.1" | "localhost" | "::1")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loopback_origins_pass() {
        assert!(is_allowed_origin("http://127.0.0.1:51616", &[]));
        assert!(is_allowed_origin("http://localhost:5173", &[]));
        assert!(is_allowed_origin("http://[::1]:51616", &[]));
        assert!(is_allowed_origin("http://localhost", &[]));
    }

    #[test]
    fn trusted_hosts_pass() {
        let trusted = vec!["lya.lian.love".into()];
        assert!(is_allowed_origin("http://lya.lian.love", &trusted));
        assert!(is_allowed_origin("https://lya.lian.love:443", &trusted));
    }

    #[test]
    fn outside_origins_are_rejected() {
        assert!(!is_allowed_origin("https://evil.example.com", &[]));
        // 这类拼接是常见的绕过尝试
        assert!(!is_allowed_origin("http://127.0.0.1.evil.com", &[]));
        assert!(!is_allowed_origin("http://localhost.evil.com", &[]));
        assert!(!is_allowed_origin("http://lya.lian.love.evil.com", &["lya.lian.love".into()]));
    }
}
