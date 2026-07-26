# lya-http

lya 的出站 HTTP 基础设施 crate。

## 职责

- 维护长期存活的 `reqwest::Client`（底层 Hyper 连接池）
- 统一超时 / keep-alive / 空闲连接配置
- 提供可 clone、零成本共享的 `HttpClient`
- 暴露字节流接口，避免大响应整块缓冲
- 将传输错误归类为稳定的 `HttpError`

## 用法

```rust
use lya_http::{HttpClient, HttpConfig};

let http = HttpClient::new(&HttpConfig::default())?;
let resp = http.get("https://example.com").send().await?;
```

多个模块 `clone` 同一客户端时，共享同一连接池：

```rust
let llm = LlmClient::new(http.clone());
let web = WebClient::new(http.clone());
```
