//! 订阅全局 SSE，调 `notify-send` 弹桌面通知。

use std::collections::HashSet;
use std::path::PathBuf;
use std::process::Command;
use std::sync::LazyLock;

use futures_util::StreamExt;
use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Deserialize)]
struct Envelope {
    scope: String,
    #[serde(rename = "type")]
    kind: String,
    payload: Value,
}

static ICON_PATH: LazyLock<PathBuf> = LazyLock::new(|| {
    let path = std::env::temp_dir().join("lya-tray-icon.png");
    if !path.exists() {
        let _ = std::fs::write(&path, include_bytes!("../../../web/public/icon.png"));
    }
    path
});

/// 订阅 `GET /api/events`，断线后自动重连。
pub async fn listen(port: u16) {
    let url = format!("http://127.0.0.1:{port}/api/events");
    let client = reqwest::Client::new();
    let mut seen_hitl = HashSet::new();

    loop {
        match stream_events(&client, &url, &mut seen_hitl).await {
            Ok(()) => eprintln!("桌面通知 SSE 连接结束，5 秒后重连"),
            Err(err) => eprintln!("桌面通知 SSE 异常：{err}，5 秒后重连"),
        }
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    }
}

async fn stream_events(
    client: &reqwest::Client,
    url: &str,
    seen_hitl: &mut HashSet<i64>,
) -> Result<(), String> {
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|err| err.to_string())?;
    if !response.status().is_success() {
        return Err(format!("HTTP {}", response.status()));
    }

    let mut buffer = String::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|err| err.to_string())?;
        buffer.push_str(&String::from_utf8_lossy(&chunk));
        while let Some(pos) = buffer.find("\n\n") {
            let block = buffer[..pos].to_string();
            buffer = buffer[pos + 2..].to_string();
            if let Some(envelope) = parse_sse_block(&block) {
                dispatch(&envelope, seen_hitl);
            }
        }
    }
    Ok(())
}

fn parse_sse_block(block: &str) -> Option<Envelope> {
    let data = block
        .lines()
        .find_map(|line| line.strip_prefix("data: "))
        .map(str::to_string)?;
    serde_json::from_str(&data).ok()
}

fn dispatch(envelope: &Envelope, seen_hitl: &mut HashSet<i64>) {
    if envelope.scope != "global" {
        return;
    }

    let title = envelope
        .payload
        .get("session_title")
        .and_then(Value::as_str)
        .unwrap_or("lya");

    match envelope.kind.as_str() {
        "notify_hitl" => {
            let Some(message_id) = envelope.payload.get("message_id").and_then(Value::as_i64) else {
                return;
            };
            if !seen_hitl.insert(message_id) {
                return;
            }
            send(
                &format!("需要确认 · {title}"),
                &hitl_body(&envelope.payload),
            );
        }
        "notify_completed" => {
            send(&format!("回复完成 · {title}"), "本轮对话已结束");
        }
        "notify_failed" => {
            let message = envelope
                .payload
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("请求失败");
            send(&format!("出错 · {title}"), message);
        }
        "notify_max_rounds" => {
            send(
                &format!("轮次用尽 · {title}"),
                "已达到工具调用轮次上限",
            );
        }
        _ => {}
    }
}

fn hitl_body(payload: &Value) -> String {
    match (
        payload.get("review_index").and_then(Value::as_u64),
        payload.get("review_total").and_then(Value::as_u64),
    ) {
        (Some(index), Some(total)) if total > 1 => format!("待确认 {index}/{total}"),
        _ => "需要你确认一项操作".into(),
    }
}

fn send(summary: &str, body: &str) {
    let available = Command::new("notify-send")
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success());
    if !available {
        return;
    }

    let icon = ICON_PATH.to_string_lossy();
    let _ = Command::new("notify-send")
        .args(["-a", "lya", "-i", icon.as_ref(), summary, body])
        .spawn();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hitl_body_shows_batch_progress() {
        let payload = serde_json::json!({
            "review_index": 2,
            "review_total": 3,
        });
        assert_eq!(hitl_body(&payload), "待确认 2/3");
    }

    #[test]
    fn hitl_dedup_skips_same_message_id() {
        let mut seen = HashSet::new();
        let envelope = Envelope {
            scope: "global".into(),
            kind: "notify_hitl".into(),
            payload: serde_json::json!({
                "session_title": "测试",
                "message_id": 42,
            }),
        };
        dispatch(&envelope, &mut seen);
        assert!(seen.contains(&42));
        // 第二次同 id 不应 panic；notify-send 在测试环境通常不存在
        dispatch(&envelope, &mut seen);
    }
}
