//! [`SessionHub`]：把 agent 的执行和订阅者解耦。
//!
//! ## 职责
//!
//! - **并发闸门**：一个会话同时只允许跑一轮，多开的标签页也不会撞在一起
//! - 把 agent 的事件流**广播**给任意多个订阅者，谁来晚了都不影响别人
//! - 持有正在进行那一轮的缓冲（[`TurnBuffer`]），新订阅者一连上就能拿到
//!   [`Snapshot`] 而不是从半截开始
//! - 会话树与分支视图（[`SessionTree`] / [`BranchInfo`]）
//!
//! ## 非职责
//!
//! - 不编排一轮对话——装配提示词、跑工具、决定何时停都在 `lya-agent`
//! - 不做 HTTP，也不定义 wire 格式，那是 `lya-api`
//! - 不直接写库；落库由它调用的 agent 完成

#![deny(missing_docs)]

pub mod event;
mod hub;

pub use hub::{
    BranchInfo, CallState, HubError, ProviderSearchState, SessionHub, SessionTree, Snapshot,
    TurnBuffer,
};
