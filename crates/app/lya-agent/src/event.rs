//! 一轮对话对外抛出的事件。
//!
//! 取消标志本身定义在 [`lya_tool`]——真正需要观察它的是工具（一条跑了半天的
//! 命令得能被叫停），而工具层看不见 agent。这里只是再导出，方便调用方少写一个
//! 依赖。

pub use lya_session::MessageRecord;
pub use lya_tool::CancelToken;

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
    /// 工具连续失败太多次，判定为原地打转。
    ToolFailureLoop {
        /// 连续失败次数。
        count: u32,
        /// 最后一次失败的工具名，方便直接定位。
        last_tool: String,
    },
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

    /// 新消息已落库。
    ///
    /// 带完整记录而不只是 id，这样订阅者拿快照起步之后，光靠事件流就能维护
    /// 一份完整状态，不必每次落库都回拉一遍。回拉除了多一次往返，还会引入
    /// 竞态：拉取在路上时新的增量到了，就可能被回来的旧快照覆盖掉。
    MessageCommitted {
        /// 落库后的完整记录。
        record: Box<MessageRecord>,
    },

    /// 已有消息的内容被改写。
    ///
    /// 流式的助手消息先落一条空占位（好让界面有 id 可挂增量），说完之后才把
    /// 正文写回去；中断时也走这里，把状态改成 `interrupted`。少了这个事件，
    /// 订阅者手里就一直是那条空占位。
    MessageUpdated {
        /// 改写后的完整记录。
        record: Box<MessageRecord>,
    },

    /// 消息被删掉了。
    ///
    /// 模型一个字都没产出时，占位消息会被清掉，免得历史里留个空壳。而它的
    /// `MessageCommitted` 已经发出去了——不补这条，界面上就会留下一个永远
    /// 抹不掉的幽灵。
    MessageDeleted {
        /// 被删节点的 id。
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

    /// 同一条 assistant 消息里的 tool_calls 成组开始处理。
    ToolBatchStarted {
        /// 组 id，与 assistant `lya.meta.tool_batch.id` 一致。
        batch_id: String,
        /// 承载这批 tool_calls 的助手消息 id。
        message_id: i64,
        /// 组内各 call 的摘要。
        calls: Vec<BatchCallInfo>,
    },

    /// 需要用户介入，HITL 节点已入树。
    AwaitHuman {
        /// HITL 消息节点 id。
        message_id: i64,
        /// 所属调用组；非批内 HITL（如单独 form）时为 `None`。
        batch_id: Option<String>,
        /// 在 needs_review 子集里的序号（从 1 起）；无批上下文时为 `None`。
        review_index: Option<u32>,
        /// needs_review 总数；无批上下文时为 `None`。
        review_total: Option<u32>,
    },

    /// Responses 原生联网状态（不是 lya-tool 的 `web_search`）。
    ProviderSearch {
        /// provider 侧 call id，仅 UI 用。
        call_id: String,
        /// 当前阶段。
        phase: ProviderSearchPhase,
        /// 搜索词（completed 时可能有）。
        query: Option<String>,
    },

    /// 本轮结束。
    TurnEnd {
        /// 结束原因。
        reason: TurnEndReason,
    },
}

/// Responses 原生联网的阶段。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderSearchPhase {
    /// 准备中。
    InProgress,
    /// 正在搜索。
    Searching,
    /// 已完成。
    Completed,
    /// 失败。
    Failed,
}

impl ProviderSearchPhase {
    /// wire / SSE 字符串。
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InProgress => "in_progress",
            Self::Searching => "searching",
            Self::Completed => "completed",
            Self::Failed => "failed",
        }
    }
}

/// 调用组里一条 call 的摘要。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchCallInfo {
    /// 调用 id。
    pub call_id: String,
    /// 函数名。
    pub name: String,
    /// 是否需要用户确认后才执行。
    pub needs_review: bool,
}
