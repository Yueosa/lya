//! # lya-core
//!
//! 启动层：读配置、开库、组装 Agent 与 Hub、挂载 HTTP 并监听。
//!
//! 只导出 [`start_server`]；Hub / API / 媒体 / 存储类型请直接从
//! `lya_hub`、`lya_api`、`lya_media`、`lya_storage` 引用。

#![deny(missing_docs)]

pub mod run;

pub use run::{RunError, ServerHandle, start as start_server};
