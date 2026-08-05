//! HTTP 接口：REST 写操作、SSE 订阅、静态 WebUI。
//!
//! ## 职责
//!
//! - 路由与 wire 类型：请求体、响应体、错误码的映射
//! - SSE 端点：会话事件流与全局事件流
//! - 内嵌前端产物（`web/dist`），非 API 路径回退到 `index.html`
//! - 两道闸门：同源守卫，以及媒体/本地图片端点的启动令牌
//! - 媒体的 Range 分片响应（播放器要拖进度条）
//!
//! ## 非职责
//!
//! - 不含业务判断：能不能发、要不要确认、下一轮做什么，一律转给 `lya-hub` / `lya-agent`
//! - 不直接碰库
//!
//! 路由表与各 handler 的分工见 [`http`] 的模块注释。

#![deny(missing_docs)]

pub mod http;

pub use http::router;
