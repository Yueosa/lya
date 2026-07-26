# lya-llm

OpenAI 兼容的 `chat/completions` 客户端（瘦模块）。

## 职责

- 按 endpoint（`base_url` / `api_key` / 额外 params）组装请求
- 非流式 `chat`：一次拿完整 assistant 输出
- 流式 `chat_stream`：解析 SSE，产出正文 / 思考 / tool_calls 增量
- 可选：把流式增量拼成完整 [`ChatCompletion`]

## 依赖

- [`lya-http`](../lya-http)：共享连接池

## 用法

```rust
use lya_http::HttpClient;
use lya_llm::{ChatMessage, LlmClient, LlmEndpoint, Role};

let http = HttpClient::with_defaults()?;
let llm = LlmClient::new(http);

let endpoint = LlmEndpoint::new(
    "https://api.deepseek.com/v1",
    "sk-...",
).with_param("model", serde_json::json!("deepseek-chat"));

let completion = llm
    .chat(
        &endpoint,
        &[ChatMessage::user("你好")],
        &[],
    )
    .await?;
```
