//! [`SessionHub`]：把 agent 的执行和订阅者解耦。

#![deny(missing_docs)]

pub mod event;
mod hub;

pub use hub::{
    BranchInfo, CallState, HubError, ProviderSearchState, SessionHub, SessionTree, Snapshot,
    TurnBuffer,
};
