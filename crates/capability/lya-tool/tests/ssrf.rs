//! 端到端验证 `web_fetch` 的内网防护。
//!
//! 单测里判定逻辑已经覆盖了，这里补的是**真的发请求**这一层：起一个本地服务假扮
//! lya 的 API 与用户自己的开发服务，确认「拒绝」「确认」「重定向绕行」三条路径
//! 的行为都对得上。

use std::sync::Arc;
use std::sync::atomic::AtomicU16;

use lya_http::HttpClient;
use lya_tool::context::ToolCtx;
use lya_tool::tools::web::WebFetchTool;
use lya_tool::traits::Tool;
use serde_json::json;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

/// 起一个只会照本宣科回一段响应的服务，返回它的端口。
///
/// 不用 axum：这里要构造 302 这种手写更省事的响应。
async fn serve(response: &'static str) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        while let Ok((mut stream, _)) = listener.accept().await {
            let mut buf = [0u8; 1024];
            let _ = stream.read(&mut buf).await;
            let _ = stream.write_all(response.as_bytes()).await;
            let _ = stream.flush().await;
        }
    });
    port
}

fn tool(self_port: u16) -> WebFetchTool {
    WebFetchTool::new(
        HttpClient::with_defaults().unwrap(),
        Arc::new(AtomicU16::new(self_port)),
    )
}

async fn fetch(tool: &WebFetchTool, url: String) -> lya_tool::meta::ToolResult {
    tool.call(ToolCtx::new(Default::default()), json!({ "url": url }))
        .await
}

const SECRET_BODY: &str = concat!(
    "HTTP/1.1 200 OK\r\n",
    "content-type: text/plain\r\n",
    "content-length: 24\r\n\r\n",
    "api_key = LEAKED-SECRET"
);

#[tokio::test]
async fn reaching_lya_itself_never_returns_a_body() {
    // 这个服务假扮 lya 的 /api/config/raw/models.toml
    let port = serve(SECRET_BODY).await;
    let result = fetch(
        &tool(port),
        format!("http://127.0.0.1:{port}/api/config/raw/models.toml"),
    )
    .await;

    assert!(!result.success);
    assert!(
        !result.content.contains("LEAKED-SECRET"),
        "密钥不能出现在给模型的结果里：{}",
        result.content
    );
    assert!(result.content.contains("lya 自己的接口"));
}

#[tokio::test]
async fn a_users_own_dev_server_still_works_after_confirmation() {
    // 同一台机器上的别的端口只是普通内网服务：确认过就照常读
    let port = serve(concat!(
        "HTTP/1.1 200 OK\r\n",
        "content-type: text/plain\r\n",
        "content-length: 2\r\n\r\n",
        "ok"
    ))
    .await;
    // lya 自己在别的端口上
    let tool = tool(port + 1);

    // 先确认它确实会请示用户
    let request = tool
        .confirm_request(&json!({ "url": format!("http://127.0.0.1:{port}/health") }))
        .unwrap();
    assert!(request.summary.contains(&port.to_string()));

    // 用户放行后 agent 会原样再调一次，这时要真的读到内容
    let result = fetch(&tool, format!("http://127.0.0.1:{port}/health")).await;
    assert!(result.success, "{}", result.content);
    assert!(result.content.contains("ok"));
}

#[tokio::test]
async fn a_redirect_into_lya_is_caught_after_the_hop() {
    // 外网页面 302 到内网是绕过「请求前校验」的经典手法，必须对落地地址再判一次
    let secret_port = serve(SECRET_BODY).await;
    let redirect = format!(
        "HTTP/1.1 302 Found\r\nlocation: http://127.0.0.1:{secret_port}/api/config\r\ncontent-length: 0\r\n\r\n"
    );
    // 需要 'static，泄漏一个小字符串换测试可读性
    let redirect: &'static str = Box::leak(redirect.into_boxed_str());
    let entry_port = serve(redirect).await;

    let result = fetch(
        &tool(secret_port),
        format!("http://127.0.0.1:{entry_port}/start"),
    )
    .await;

    assert!(!result.success);
    assert!(
        !result.content.contains("LEAKED-SECRET"),
        "跳转过去拿到的内容同样不能回给模型：{}",
        result.content
    );
}
