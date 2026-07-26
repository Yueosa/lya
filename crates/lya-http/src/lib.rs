//! # lya-http
//!
//! lya 出站 HTTP 基础设施。
//!
//! ## 设计目标
//!
//! - **共享连接池**：整个进程只建一个（或少数几个）长期存活的
//!   [`HttpClient`]。`Clone` 只增加内部 `Arc` 引用计数，不复制连接。
//! - **配置可调**：超时、空闲连接、TCP keepalive、每 host 空闲上限等
//!   通过 [`HttpConfig`] 暴露，避免调用方手搓 `reqwest::ClientBuilder`。
//! - **优先流式**：提供 [`HttpClient::send_bytes_stream`]，上层（LLM / SSE /
//!   大文件下载）按 chunk 消费，避免把整段 body 拉进内存。
//! - **错误可分类**：[`HttpError`] 区分超时、连接失败、HTTP 状态、解码失败等，
//!   方便上层决定是否重试、是否对用户展示。
//!
//! ## 明确不做什么
//!
//! - 不解析 OpenAI / DeepSeek chat completion、SSE `data:` 行等业务协议。
//! - 不做「对任意 POST 自动重试」——LLM 请求重试语义应由 `lya-llm` 决定。
//! - 不持有全局并发 semaphore；长流式请求若在 http 层占槽会死锁吞吐，
//!   限流留给各消费者自行做。
//!
//! ## 模块结构
//!
//! - [`config`] — 连接池与超时配置
//! - [`client`] — 对外主入口 [`HttpClient`]
//! - [`error`] — 错误类型
//! - [`stream`] — 字节流辅助类型与工具函数

#![deny(missing_docs)]

pub mod client;
pub mod config;
pub mod error;
pub mod stream;

pub use client::{HttpClient, HttpResponse, header};
pub use config::HttpConfig;
pub use error::HttpError;
pub use stream::{ByteStream, map_stream_error};
