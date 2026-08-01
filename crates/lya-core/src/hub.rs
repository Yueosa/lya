//! [`SessionHub`]：把 agent 的执行和订阅者解耦。
//!
//! 存在的理由是 agent 刻意没做的四件事：
//!
//! 1. **让执行脱离订阅者。** `Agent::run_turn` 返回的流「drop 就停」。若 HTTP
//!    handler 直接消费它喂给 SSE，用户刷新页面就等于把对话掐断。这里 spawn 一个
//!    任务持有那个流，事件转发到广播，订阅者爱来来爱走走。
//! 2. **轮次串行。** 同一会话同时只跑一轮，否则两轮会往同一棵树上抢着追加消息。
//! 3. **实时缓冲。** 累积当前轮已产出的内容，供新订阅者做快照。
//! 4. **取消。** 持有本轮的 `CancelToken`。

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use futures_util::StreamExt;
use lya_agent::{Agent, AgentError, AgentEvent, CallKind, CancelToken, ChatBackend};
use lya_llm::LlmClient;
use lya_session::{MessageRecord, SessionError, SessionMeta};
use serde::Serialize;
use tokio::sync::broadcast;

use crate::event::{self, Envelope};

/// 广播通道容量。订阅者落后超过这么多条就会收到 `Lagged`，
/// 届时重发一次快照即可对齐。
const BROADCAST_CAPACITY: usize = 256;

/// Hub 层错误。
#[derive(Debug, thiserror::Error)]
pub enum HubError {
    /// 会话不存在。
    #[error("session not found: {0}")]
    NotFound(String),

    /// 该会话已经有一轮在跑。
    #[error("session {0} is already running a turn")]
    Busy(String),

    /// 会话层错误。
    #[error(transparent)]
    Session(#[from] SessionError),

    /// agent 层错误。
    #[error(transparent)]
    Agent(#[from] AgentError),
}

/// 当前轮次已经产出的内容。
///
/// 它不是另一份数据，就是**正在写的那条 assistant 消息的当前样子**——
/// 消息本身只在开始和结束落两次库，中间的增量攒在这里。
#[derive(Debug, Clone, Default, Serialize)]
pub struct TurnBuffer {
    /// 第几轮 LLM 调用。
    pub round: u32,
    /// 正在写的那条消息节点 id。
    pub message_id: Option<i64>,
    /// 已产出的正文。
    pub content: String,
    /// 已产出的思考。
    pub reasoning: String,
    /// 本轮的调用状态。
    pub calls: Vec<CallState>,
}

/// 一次工具或动作调用的状态。
#[derive(Debug, Clone, Serialize)]
pub struct CallState {
    /// 调用 id。
    pub call_id: String,
    /// 函数名。
    pub name: String,
    /// `tool` 或 `action`。
    pub kind: String,
    /// 是否成功；`None` 表示还在跑。
    pub success: Option<bool>,
}

/// 订阅或查询时给出的完整状态。
#[derive(Debug, Clone, Serialize)]
pub struct Snapshot {
    /// 会话元数据。
    pub session: SessionMeta,
    /// 当前分支从根到叶的消息。
    pub messages: Vec<MessageRecord>,
    /// 正在跑的那一轮；没有则为 `None`。
    pub running: Option<TurnBuffer>,
}

/// 一个会话的广播通道与运行状态。
struct SessionChannel {
    /// 广播给所有订阅者。
    tx: broadcast::Sender<Envelope>,
    /// 本轮的取消标志；`None` 表示当前没有轮次在跑。
    cancel: Option<CancelToken>,
    /// 本轮的实时缓冲。
    buffer: Option<TurnBuffer>,
}

/// 会话运行时。
///
/// 对后端泛型，默认就是真实的 [`LlmClient`]；测试可以换成脚本化的假后端，
/// 这样轮次串行、取消、缓冲这些逻辑不必联网也能验。
pub struct SessionHub<B: ChatBackend = LlmClient> {
    agent: Arc<Agent<B>>,
    /// 每个会话一个通道；外层锁只在增删会话时争用，
    /// 高频的事件转发只锁内层。
    channels: Mutex<HashMap<String, Arc<Mutex<SessionChannel>>>>,
    seq: AtomicU64,
}

impl<B: ChatBackend + 'static> SessionHub<B> {
    /// 用组装好的 agent 构造。
    pub fn new(agent: Arc<Agent<B>>) -> Arc<Self> {
        Arc::new(Self {
            agent,
            channels: Mutex::new(HashMap::new()),
            seq: AtomicU64::new(0),
        })
    }

    /// 底层 agent，供 HTTP 层做会话 CRUD 与 HITL 答复。
    pub fn agent(&self) -> &Agent<B> {
        &self.agent
    }

    /// 该会话是否有轮次在跑。
    pub fn is_running(&self, session_id: &str) -> bool {
        self.channel_if_present(session_id)
            .is_some_and(|channel| channel.lock().unwrap().cancel.is_some())
    }

    /// 读取一份快照。
    pub fn snapshot(&self, session_id: &str) -> Result<Snapshot, HubError> {
        let sessions = self.agent.sessions();
        let session = sessions
            .get_session(session_id)?
            .ok_or_else(|| HubError::NotFound(session_id.to_string()))?;
        let messages = sessions.path_to_active_leaf(session_id)?;
        let running = self
            .channel_if_present(session_id)
            .and_then(|channel| channel.lock().unwrap().buffer.clone());
        Ok(Snapshot {
            session,
            messages,
            running,
        })
    }

    /// 订阅：先给一份快照，再给增量流。
    ///
    /// 快照与订阅在**同一把锁内**完成，所以不会漏事件、也不会把已经算进快照的
    /// 增量再收一遍。首次打开和断线重连走的是同一条路。
    pub fn subscribe(
        &self,
        session_id: &str,
    ) -> Result<(Snapshot, broadcast::Receiver<Envelope>), HubError> {
        let sessions = self.agent.sessions();
        let session = sessions
            .get_session(session_id)?
            .ok_or_else(|| HubError::NotFound(session_id.to_string()))?;
        let messages = sessions.path_to_active_leaf(session_id)?;

        let channel = self.channel(session_id);
        let guard = channel.lock().unwrap();
        let running = guard.buffer.clone();
        let rx = guard.tx.subscribe();
        drop(guard);

        Ok((
            Snapshot {
                session,
                messages,
                running,
            },
            rx,
        ))
    }

    /// 跑一轮。用户消息应当已经由调用方追加进树。
    ///
    /// 已经有一轮在跑时返回 [`HubError::Busy`]——排队或打断都会让「我刚才发的
    /// 那条到底算不算数」变得难以预期，不如直接说清楚。
    pub fn start_turn(self: &Arc<Self>, session_id: &str) -> Result<(), HubError> {
        if self.agent.sessions().get_session(session_id)?.is_none() {
            return Err(HubError::NotFound(session_id.to_string()));
        }

        let channel = self.channel(session_id);
        let cancel = CancelToken::new();
        {
            let mut guard = channel.lock().unwrap();
            if guard.cancel.is_some() {
                return Err(HubError::Busy(session_id.to_string()));
            }
            guard.cancel = Some(cancel.clone());
            guard.buffer = Some(TurnBuffer::default());
        }

        let hub = Arc::clone(self);
        let agent = Arc::clone(&self.agent);
        let id = session_id.to_string();
        tokio::spawn(async move {
            let stream = agent.run_turn(id.clone(), cancel);
            futures_util::pin_mut!(stream);
            while let Some(agent_event) = stream.next().await {
                hub.publish(&id, &agent_event);
            }
            hub.finish(&id);
        });
        Ok(())
    }

    /// 停止当前轮；没有轮次在跑时返回 false。
    pub fn stop(&self, session_id: &str) -> bool {
        let Some(channel) = self.channel_if_present(session_id) else {
            return false;
        };
        let guard = channel.lock().unwrap();
        match &guard.cancel {
            Some(cancel) => {
                cancel.cancel();
                true
            }
            None => false,
        }
    }

    /// 更新缓冲并广播。
    ///
    /// 两件事必须在同一把锁里做：否则一个刚好在中间订阅进来的客户端，会既在快照
    /// 里看到这段增量、又收到它的事件，界面上就重了一段。
    fn publish(&self, session_id: &str, event: &AgentEvent) {
        let Some(channel) = self.channel_if_present(session_id) else {
            return;
        };
        let mut guard = channel.lock().unwrap();
        if let Some(buffer) = guard.buffer.as_mut() {
            apply(buffer, event);
        }
        let seq = self.seq.fetch_add(1, Ordering::Relaxed);
        if let Some(envelope) = event::from_agent(session_id, seq, event) {
            // 没有订阅者时发送失败是正常的，执行照常继续
            let _ = guard.tx.send(envelope);
        }
    }

    /// 一轮结束：清掉运行状态，没人订阅就把通道也回收掉。
    fn finish(&self, session_id: &str) {
        let mut channels = self.channels.lock().unwrap();
        let Some(channel) = channels.get(session_id).cloned() else {
            return;
        };
        let idle = {
            let mut guard = channel.lock().unwrap();
            guard.cancel = None;
            guard.buffer = None;
            guard.tx.receiver_count() == 0
        };
        if idle {
            channels.remove(session_id);
        }
    }

    /// 取通道，没有就建一个。
    fn channel(&self, session_id: &str) -> Arc<Mutex<SessionChannel>> {
        let mut channels = self.channels.lock().unwrap();
        Arc::clone(channels.entry(session_id.to_string()).or_insert_with(|| {
            let (tx, _) = broadcast::channel(BROADCAST_CAPACITY);
            Arc::new(Mutex::new(SessionChannel {
                tx,
                cancel: None,
                buffer: None,
            }))
        }))
    }

    /// 只取已有通道，不创建。
    fn channel_if_present(&self, session_id: &str) -> Option<Arc<Mutex<SessionChannel>>> {
        self.channels.lock().unwrap().get(session_id).cloned()
    }
}

/// 把一条事件累加进缓冲。
fn apply(buffer: &mut TurnBuffer, event: &AgentEvent) {
    match event {
        AgentEvent::RoundStarted { round } => {
            buffer.round = *round;
            // 新一轮的正文从头开始；上一轮的已经落库了
            buffer.content.clear();
            buffer.reasoning.clear();
            buffer.message_id = None;
        }
        AgentEvent::Delta(text) => buffer.content.push_str(text),
        AgentEvent::Reasoning(text) => buffer.reasoning.push_str(text),
        AgentEvent::MessageCommitted { id } => {
            if buffer.message_id.is_none() {
                buffer.message_id = Some(*id);
            }
        }
        AgentEvent::CallStarted {
            call_id,
            name,
            kind,
        } => buffer.calls.push(CallState {
            call_id: call_id.clone(),
            name: name.clone(),
            kind: match kind {
                CallKind::Tool => "tool".into(),
                CallKind::Action => "action".into(),
            },
            success: None,
        }),
        AgentEvent::CallFinished {
            call_id, success, ..
        } => {
            if let Some(call) = buffer.calls.iter_mut().find(|c| &c.call_id == call_id) {
                call.success = Some(*success);
            }
        }
        AgentEvent::AwaitHuman { .. } | AgentEvent::TurnEnd { .. } => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn buffer_after(events: &[AgentEvent]) -> TurnBuffer {
        let mut buffer = TurnBuffer::default();
        for event in events {
            apply(&mut buffer, event);
        }
        buffer
    }

    #[test]
    fn deltas_accumulate() {
        let buffer = buffer_after(&[
            AgentEvent::RoundStarted { round: 1 },
            AgentEvent::MessageCommitted { id: 7 },
            AgentEvent::Reasoning("想".into()),
            AgentEvent::Delta("喵".into()),
            AgentEvent::Delta("~".into()),
        ]);
        assert_eq!(buffer.round, 1);
        assert_eq!(buffer.message_id, Some(7));
        assert_eq!(buffer.content, "喵~");
        assert_eq!(buffer.reasoning, "想");
    }

    #[test]
    fn new_round_resets_text_but_keeps_calls() {
        let buffer = buffer_after(&[
            AgentEvent::RoundStarted { round: 1 },
            AgentEvent::Delta("第一轮".into()),
            AgentEvent::CallStarted {
                call_id: "c1".into(),
                name: "ls".into(),
                kind: CallKind::Tool,
            },
            AgentEvent::CallFinished {
                call_id: "c1".into(),
                name: "ls".into(),
                success: true,
            },
            AgentEvent::RoundStarted { round: 2 },
            AgentEvent::Delta("第二轮".into()),
        ]);
        assert_eq!(buffer.content, "第二轮", "上一轮正文已落库，不该重复堆着");
        assert_eq!(buffer.calls.len(), 1, "调用记录跨轮保留，界面要连着看");
        assert_eq!(buffer.calls[0].success, Some(true));
    }

    #[test]
    fn call_state_tracks_completion() {
        let buffer = buffer_after(&[AgentEvent::CallStarted {
            call_id: "c1".into(),
            name: "bash".into(),
            kind: CallKind::Tool,
        }]);
        assert_eq!(buffer.calls[0].success, None, "还在跑");
        assert_eq!(buffer.calls[0].kind, "tool");
    }
}
