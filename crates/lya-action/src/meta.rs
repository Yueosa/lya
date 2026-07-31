//! 动作元信息与执行结果。

use lya_session::HitlBlock;
use lya_tool::ToolResult;

/// 动作执行后 agent 循环该怎么走。
///
/// 工具只有一种走法（执行完回灌、继续下一轮），动作有两种，所以这个标记
/// 是「内部把 action 和 tool 当两类东西」的核心区别之一。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionFlow {
    /// 结果回灌给模型，继续本轮。
    Continue,
    /// 挂起本轮等用户；用户答复后**继续**本轮，而不是重新开始。
    AwaitHuman,
}

impl ActionFlow {
    /// 提示词里的标注。
    pub const fn label(self) -> &'static str {
        match self {
            Self::Continue => "继续",
            Self::AwaitHuman => "等待用户",
        }
    }
}

/// 动作静态元信息。
///
/// 与 [`lya_tool::ToolMeta`] 对称，只是把 `prmt`（RWX 权限）换成了
/// [`ActionFlow`]：动作不受模式权限约束，但有流转差异。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionMeta {
    /// 内部名：注册键，也是 LLM 看到的 `function.name`。
    pub name: String,
    /// 展示名：界面与日志用。
    pub raw_name: String,
    /// 短描述，映射到 `function.description`。
    pub desc: String,
    /// 执行后的流转方式。
    pub flow: ActionFlow,
}

impl ActionMeta {
    /// 构造 meta。
    pub fn new(
        name: impl Into<String>,
        raw_name: impl Into<String>,
        desc: impl Into<String>,
        flow: ActionFlow,
    ) -> Self {
        Self {
            name: name.into(),
            raw_name: raw_name.into(),
            desc: desc.into(),
            flow,
        }
    }
}

/// 一次动作执行的结果。
#[derive(Debug, Clone, PartialEq)]
pub enum ActionOutcome {
    /// 直接产出结果，回灌给模型。
    ///
    /// 复用 [`ToolResult`]：动作结果和工具结果一样都写成 `role=tool` 消息，
    /// agent 不需要为两者分别写一套回灌逻辑。参数校验失败也走这里
    /// （`ToolResult::err`），让模型看见错误自己改。
    Continue(ToolResult),

    /// 需要人介入，交出一个待入树的 HITL 块。
    ///
    /// 本 crate **不碰** `SessionStore`：由 agent 负责把它落成 `role=hitl`
    /// 的 pending 节点、挂起本轮，并在用户答复后 resolve。
    AwaitHuman(Box<HitlBlock>),
}

impl ActionOutcome {
    /// 成功并回灌。
    pub fn ok(content: impl Into<String>) -> Self {
        Self::Continue(ToolResult::ok(content))
    }

    /// 失败并回灌（模型可据此重试）。
    pub fn err(content: impl Into<String>) -> Self {
        Self::Continue(ToolResult::err(content))
    }

    /// 请求人工介入。
    pub fn await_human(block: HitlBlock) -> Self {
        Self::AwaitHuman(Box::new(block))
    }
}
