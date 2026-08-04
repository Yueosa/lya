# lya-llm

OpenAI 兼容 LLM 客户端：支持 **Completions** 与 **Responses** 双栈。

## 职责

- 按 endpoint（`base_url` / `api_key` / per-mode params）组装请求
- **Completions**：`POST …/chat/completions`，SSE → 正文 / 思考 / tool_calls
- **Responses**：`POST …/responses`，SSE → 同上 + 原生 `web_search_call` 状态
- 非流式 `chat`、流式 `chat_stream`；可选 [`CompletionAssembler`] 拼完整结果

双栈规格见仓库 [`docs/responses-api.md`](../../docs/responses-api.md)。

## 依赖

- [`lya-http`](../lya-http)：共享连接池

## 用法

```rust
use lya_http::HttpClient;
use lya_llm::{ApiMode, ChatMessage, ChatStreamRequest, LlmClient, LlmEndpoint, Role};

let http = HttpClient::with_defaults()?;
let llm = LlmClient::new(http);

let endpoint = LlmEndpoint::new("https://api.deepseek.com/v1", "sk-…")
    .with_id("flash")
    .with_mode_param(ApiMode::Completions, "model", serde_json::json!("deepseek-chat"))
    .with_mode_param(ApiMode::Responses, "model", serde_json::json!("deepseek-chat"));

// Completions（默认栈）
let _ = llm
    .chat_stream(
        ApiMode::Completions,
        &endpoint,
        ChatStreamRequest::Completions(vec![ChatMessage {
            role: Role::User,
            content: "你好".into(),
            ..Default::default()
        }]),
        &[],
    )
    .await?;

// Responses：instructions + input items；native_web_search 由 agent 按能力注入
let (instructions, input) = ("SYSTEM".into(), vec![]);
let _ = llm
    .chat_stream(
        ApiMode::Responses,
        &endpoint,
        ChatStreamRequest::Responses {
            instructions,
            input,
            native_web_search: true,
        },
        &[],
    )
    .await?;
```

`models.toml` 里每个模型用 `[models.modes.completions]` / `[models.modes.responses]` 声明 params 与 capabilities；未声明的栈不可用于锁定该栈的会话。
