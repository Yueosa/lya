//! HTTP 接口：REST 写操作、SSE 订阅、静态 WebUI。

#![deny(missing_docs)]

pub mod http;

pub use http::router;
