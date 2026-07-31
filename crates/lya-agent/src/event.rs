//! 一轮对话对外抛出的事件，以及取消标志。

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// 被调用的是工具还是动作。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallKind {
    /// 外部环境（文件、命令、网络）。
    Tool,
    /// 自身状态（记忆、交互、模式）。
    Action,
}

/// 本轮为什么结束。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TurnEndReason {
    /// 模型给出了不带 tool_calls 的回复，正常收尾。
    Completed,
    /// 挂起等用户答复；答复后再跑一轮即可接上。
    AwaitingHuman,
    /// 达到 `max_tool_rounds` 上限。
    MaxRounds,
    /// 被调用方取消。
    Cancelled,
    /// 模型既没给正文也没给 tool_calls。
    EmptyResponse,
    /// 出错中止。
    Failed(String),
}

/// 一轮对话过程中抛出的事件。
///
/// **不变量**：一条事件流最后一定恰好有一条 [`AgentEvent::TurnEnd`]，
/// 调用方可以据此判断收尾，不必额外检测流是否结束。
#[derive(Debug, Clone, PartialEq)]
pub enum AgentEvent {
    /// 新一轮 LLM 调用开始（从 1 开始计数）。
    RoundStarted {
        /// 第几轮。
        round: u32,
    },

    /// 思考增量。落库但不回灌给模型，只用于展示。
    Reasoning(String),

    /// 助手正文增量。
    Delta(String),

    /// 某条消息已经落库；界面可据此定位到具体节点。
    MessageCommitted {
        /// 消息节点 id。
        id: i64,
    },

    /// 开始执行一次调用。
    CallStarted {
        /// 调用 id，对应模型给出的 `tool_call_id`。
        call_id: String,
        /// 函数名。
        name: String,
        /// 工具还是动作。
        kind: CallKind,
    },

    /// 一次调用执行完毕。
    CallFinished {
        /// 调用 id。
        call_id: String,
        /// 函数名。
        name: String,
        /// 是否成功；失败的结果同样会回灌给模型让它重试。
        success: bool,
    },

    /// 需要用户介入，HITL 节点已入树。
    AwaitHuman {
        /// HITL 消息节点 id。
        message_id: i64,
    },

    /// 本轮结束。
    TurnEnd {
        /// 结束原因。
        reason: TurnEndReason,
    },
}

/// 取消标志。
///
/// 调用方持有一份 clone，随时 [`CancelToken::cancel`]。agent 在每轮开始和
/// 每个流式分片之间检查；取消时会把正在写的那条助手消息标成 `Interrupted`，
/// 而不是留一条状态为「流式中」的僵尸记录。
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
