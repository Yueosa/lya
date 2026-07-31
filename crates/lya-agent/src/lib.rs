//! # lya-agent
//!
//! 一轮对话的驱动器，把其余模块串起来：读会话路径装配上下文 → 组 prompt →
//! 调 LLM → 分发工具/动作 → 结果回写消息树 → HITL 挂起与恢复。
//!
//! 它是**唯一**知道「一轮怎么跑」的地方，其余 crate 保持无状态。
//!
//! ## 循环规则
//!
//! 只有一条：**assistant 消息带 `tool_calls` 就执行并回灌、继续下一轮；不带
//! 就结束本轮。** 「边说边干」靠 `content` 与 `tool_calls` 同时出现表达，
//! 所以没有 `done` 这类显式结束信号。
//!
//! ## 为什么 `run_turn` 不收用户输入
//!
//! 用户消息由调用方先 append 进树，agent 从树读当前状态。于是「发消息」
//! 「重新生成」「编辑重发」「HITL 答复后继续」都退化成同一套动作：改树，
//! 再跑一轮。
//!
//! 更要紧的是**HITL 不需要在内存里挂起**。表单发出去本轮就正常结束了，这边
//! 没有挂起的 future、没有 waiter 表、没有超时管理；用户什么时候答复都行，
//! 答复就是往树上追加一个 tool 结果再跑一轮，进程重启也能接上。上一代把
//! 状态放在内存里，就得为此付一整套阻塞与超时机制。
//!
//! ## 流式输出的分层
//!
//! [`Agent::run_turn`] 返回 [`Stream`](futures_core::Stream)，语义是「drop 就
//! 停」。直接拿去喂 SSE 会导致用户刷新页面就把对话掐了，所以上层
//! （`SessionHub`）应当 spawn 一个任务消费它并转发到广播，这样执行就不受
//! 订阅者来去影响。轮次串行（同会话同时只跑一轮）的锁也归上层。

#![deny(missing_docs)]

pub mod agent;
pub mod backend;
pub mod context;
pub mod error;
pub mod event;

pub use agent::{Agent, AgentParts};
pub use backend::ChatBackend;
pub use context::{INTERRUPTED_MARK, MISSING_RESULT, build_messages};
pub use error::AgentError;
pub use event::{AgentEvent, CallKind, CancelToken, TurnEndReason};
