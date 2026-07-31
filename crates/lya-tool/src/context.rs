//! 工具执行上下文。
//!
//! 做成结构体而不是直接传一个取消标志，是为了以后加东西不用再改一遍所有工具的
//! 签名——加字段是兼容的，加参数不是。

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// 取消标志。
///
/// 定义在本 crate 而不是 `lya-agent`：真正需要**观察**它的是工具（一条跑了半天
/// 的命令得能被叫停），而工具层看不见 agent。
#[derive(Debug, Clone, Default)]
pub struct CancelToken(Arc<AtomicBool>);

impl CancelToken {
    /// 新建一个未取消的标志。
    pub fn new() -> Self {
        Self::default()
    }

    /// 请求取消。
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Relaxed);
    }

    /// 是否已被取消。
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}

/// 一次工具调用的上下文。
#[derive(Debug, Clone, Default)]
pub struct ToolCtx {
    /// 取消标志；长时间运行的工具应当定期检查，并尽快收手。
    pub cancel: CancelToken,
}

impl ToolCtx {
    /// 用给定的取消标志构造。
    pub fn new(cancel: CancelToken) -> Self {
        Self { cancel }
    }

    /// 是否已被取消。
    pub fn is_cancelled(&self) -> bool {
        self.cancel.is_cancelled()
    }
}
