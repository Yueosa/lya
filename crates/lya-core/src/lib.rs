//! # lya-core
//!
//! 组装层：把 [`lya_agent`] 的一轮驱动接到 HTTP 上。
//!
//! ## 两条原则
//!
//! **写操作走 REST，结果一律从订阅流出来。** 发消息返回 202 就结束，正文不在响应
//! 体里。这样同一个会话在网页和手机上看到的是同一份流，而不是「谁发的谁才看得到
//! 响应」——这正是多端体验的前提。
//!
//! **订阅 = 先快照再增量。** 首次打开和断线重连走完全同一条路，因为流式文本是
//! 累积的，快照就是「到此刻为止的全部」，客户端收到直接整体替换。于是不需要
//! `Last-Event-ID`、不需要序号对齐，也不需要把事件落库重放。
//!
//! ## 为什么要有 SessionHub
//!
//! `Agent::run_turn` 返回的流「drop 就停」。若 HTTP handler 直接消费它喂给 SSE，
//! 用户刷新页面就等于把对话掐断——上一代「流式输出到一半刷新就丢渲染」的根子就在
//! 这里。[`SessionHub`] spawn 一个任务持有那个流，事件转发到广播，订阅者的来去
//! 不再影响执行。

#![deny(missing_docs)]

pub mod event;
pub mod http;
pub mod hub;

pub use event::{Envelope, Scope};
pub use http::router;
pub use hub::{BranchInfo, CallState, HubError, SessionHub, Snapshot, TurnBuffer};
