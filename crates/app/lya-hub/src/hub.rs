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
use std::sync::{Arc, Mutex, OnceLock};

use futures_util::StreamExt;
use lya_agent::{Agent, AgentError, AgentEvent, CallKind, CancelToken, ChatBackend, TurnEndReason};
use lya_http::HttpClient;
use lya_llm::LlmClient;
use lya_session::{MessagePayload, MessageRecord, MessageRole, MessageStatus, SessionError, SessionMeta};
use lya_token::ContextUsageReport;
use serde::Serialize;
use std::time::Duration;
use tokio::sync::broadcast;
use tokio::task::JoinHandle;

use crate::event::{self, Envelope, Scope};
use lya_tool::tools::web::SelfPort;

/// 配置重载钩子的类型。
type ReloadFn = Box<dyn Fn() -> Result<(), String> + Send + Sync>;

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

    /// 请求本身说不通。
    #[error("{0}")]
    Invalid(String),
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
    /// Responses 原生联网状态（当轮 UI 用；Phase 4 再落库）。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub provider_searches: Vec<ProviderSearchState>,
}

/// Responses 原生联网的当轮状态。
#[derive(Debug, Clone, Default, Serialize)]
pub struct ProviderSearchState {
    /// provider 侧 call id。
    pub call_id: String,
    /// `in_progress` / `searching` / `completed` / `failed`。
    pub phase: String,
    /// 搜索词（若有）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
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

/// 一个分支端点的概要，供界面做分支切换器。
#[derive(Debug, Clone, Serialize)]
pub struct BranchInfo {
    /// 叶节点 id，切换时用它。
    pub leaf_id: i64,
    /// 是否为当前所在分支。
    pub is_active: bool,
    /// 叶节点的角色。
    pub role: String,
    /// 叶节点内容摘要。
    pub preview: String,
    /// 叶节点创建时间。
    pub created_at: String,
}

/// 整棵消息树。
#[derive(Debug, Clone, Serialize)]
pub struct SessionTree {
    /// 当前所在的叶节点。
    pub active_leaf_id: Option<i64>,
    /// 所有分支端点。
    pub leaves: Vec<i64>,
    /// 全部节点，按时间正序；`parent_id` 描述父子关系。
    pub nodes: Vec<MessageRecord>,
}

/// 摘出一条消息的可读预览。
fn preview_of(record: &MessageRecord) -> String {
    const MAX: usize = 80;
    let text = match &record.payload.openai {
        Some(openai) if !openai.content.trim().is_empty() => openai.content.trim().to_string(),
        // HITL 节点没有 openai 体，用块类型交代一下
        _ => match &record.payload.lya.hitl {
            Some(_) => "（等待用户答复）".to_string(),
            None => "（无正文）".to_string(),
        },
    };
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.chars().count() <= MAX {
        flat
    } else {
        format!("{}…", flat.chars().take(MAX).collect::<String>())
    }
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
    /// HITL 放行后执行工具时的取消标志（与 LLM 轮次分开，Stop 也能中止）。
    operation: Option<CancelToken>,
    /// 本轮的实时缓冲。
    buffer: Option<TurnBuffer>,
    /// 持有 `run_turn` 的任务；Stop 超时后 abort，避免僵尸轮次占着界面。
    turn_task: Option<JoinHandle<()>>,
}

/// 会话运行时。
///
/// 对后端泛型，默认就是真实的 [`LlmClient`]；测试可以换成脚本化的假后端，
/// 这样轮次串行、取消、缓冲这些逻辑不必联网也能验。
pub struct SessionHub<B: ChatBackend = LlmClient> {
    agent: Arc<Agent<B>>,
    /// 配置重载钩子，由装配处安装；见 [`SessionHub::set_reload`]。
    reload: OnceLock<ReloadFn>,
    /// 共享出站客户端，供探测模型可用性之类的杂事使用。
    http: HttpClient,
    /// 每个会话一个通道；外层锁只在增删会话时争用，
    /// 高频的事件转发只锁内层。
    channels: Mutex<HashMap<String, Arc<Mutex<SessionChannel>>>>,
    /// 全局作用域的广播；桌面通知、配置变更等走这里。
    global: broadcast::Sender<Envelope>,
    /// 本地图片端点的令牌，进程启动时随机生成。
    image_token: String,
    /// HTTP 监听端口，供媒体抓取做 SSRF 判断。
    self_port: SelfPort,
    seq: AtomicU64,
}

impl<B: ChatBackend + 'static> SessionHub<B> {
    /// 用组装好的 agent 构造。
    pub fn new(agent: Arc<Agent<B>>, http: HttpClient, self_port: SelfPort) -> Arc<Self> {
        let (global, _) = broadcast::channel(BROADCAST_CAPACITY);
        Arc::new(Self {
            agent,
            reload: OnceLock::new(),
            http,
            channels: Mutex::new(HashMap::new()),
            global,
            image_token: uuid::Uuid::new_v4().to_string(),
            self_port,
            seq: AtomicU64::new(0),
        })
    }

    /// 共享 HTTP 客户端。
    pub fn http(&self) -> &HttpClient {
        &self.http
    }

    /// 安装配置重载钩子。只认第一次，重复调用会被忽略。
    ///
    /// hub 故意**不认识配置**：真正「读文件、推给各个组件」的逻辑写在装配处
    /// （`lya-core`），那里本来就依赖全部 crate。这里只留一个按钮，好让 HTTP 层
    /// 在写完配置文件后按一下，不必为此把 `lya-config` 拖进 hub 的依赖里。
    pub fn set_reload(&self, reload: impl Fn() -> Result<(), String> + Send + Sync + 'static) {
        let _ = self.reload.set(Box::new(reload));
    }

    /// 重新读配置并推给运行中的组件。
    ///
    /// 没装钩子时是空操作——测试里的 hub 不接配置，不该因此报错。
    pub fn reload_config(&self) -> Result<(), String> {
        match self.reload.get() {
            Some(reload) => reload(),
            None => Ok(()),
        }
    }

    /// 本地图片端点的令牌。
    ///
    /// 每次启动重新生成，所以旧页面上的图片链接在重启后会失效——重新加载即可。
    /// 这点不便换来的是：泄露出去的链接活不过一次重启。
    pub fn image_token(&self) -> &str {
        &self.image_token
    }

    /// 当前 HTTP 监听端口；绑定前为 0。
    pub fn self_port(&self) -> u16 {
        self.self_port.load(Ordering::Relaxed)
    }

    /// 订阅全局事件。
    pub fn subscribe_global(&self) -> broadcast::Receiver<Envelope> {
        self.global.subscribe()
    }

    /// 广播一条全局事件。
    ///
    /// 目前的产出方是配置变更；桌面通知、会话列表变化将来也走这里——事件信封
    /// 从一开始就带 `scope`，加新来源不用改客户端的分发逻辑。
    pub fn broadcast_global(&self, kind: &str, payload: serde_json::Value) {
        let seq = self.seq.fetch_add(1, Ordering::Relaxed);
        let _ = self.global.send(Envelope {
            scope: Scope::Global.as_wire(),
            kind: kind.to_string(),
            seq,
            payload,
        });
    }

    /// 底层 agent，供 HTTP 层做会话 CRUD 与 HITL 答复。
    pub fn agent(&self) -> &Agent<B> {
        &self.agent
    }

    /// 估算当前活跃分支的上下文占用（只读）。
    pub fn estimate_context_usage(&self, session_id: &str) -> Result<ContextUsageReport, HubError> {
        self.agent
            .estimate_context_usage(session_id)
            .map_err(HubError::Agent)
    }

    /// 该会话是否有轮次在跑，或正在执行挂起后放行的工具。
    pub fn is_running(&self, session_id: &str) -> bool {
        self.channel_if_present(session_id).is_some_and(|channel| {
            let guard = channel.lock().unwrap();
            guard.cancel.is_some() || guard.operation.is_some()
        })
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
        let cancel = self.reserve_turn(session_id)?;
        self.spawn_turn(session_id, cancel);
        Ok(())
    }

    /// 追加用户消息并开跑。先占轮次坑再写库，避免 Busy 后留下 orphan 消息。
    pub fn send_user_message_and_start(
        self: &Arc<Self>,
        session_id: &str,
        text: &str,
    ) -> Result<(), HubError> {
        if text.trim().is_empty() {
            return Err(HubError::Invalid("消息不能为空".into()));
        }
        if self.agent.sessions().get_session(session_id)?.is_none() {
            return Err(HubError::NotFound(session_id.to_string()));
        }

        let cancel = self.reserve_turn(session_id)?;
        let record = match self
            .agent
            .sessions()
            .append(session_id, MessagePayload::user_text(text), false)
        {
            Ok(record) => record,
            Err(err) => {
                self.release_turn(session_id);
                return Err(err.into());
            }
        };
        self.publish(
            session_id,
            &AgentEvent::MessageCommitted {
                record: Box::new(record),
            },
        );
        self.spawn_turn(session_id, cancel);
        Ok(())
    }

    /// 占住本轮坑位；失败时不改树。
    fn reserve_turn(&self, session_id: &str) -> Result<CancelToken, HubError> {
        let channel = self.channel(session_id);
        let mut guard = channel.lock().unwrap();
        if guard.cancel.is_some() || guard.operation.is_some() {
            return Err(HubError::Busy(session_id.to_string()));
        }
        let cancel = CancelToken::new();
        guard.cancel = Some(cancel.clone());
        guard.buffer = Some(TurnBuffer::default());
        Ok(cancel)
    }

    /// 追加消息失败时回滚占坑。
    fn release_turn(&self, session_id: &str) {
        let channel = self.channel(session_id);
        let mut guard = channel.lock().unwrap();
        guard.cancel = None;
        guard.buffer = None;
    }

    /// 占住 HITL 放行后的工具执行，供 Stop 取消。
    pub fn reserve_operation(&self, session_id: &str) -> Result<CancelToken, HubError> {
        let channel = self.channel(session_id);
        let mut guard = channel.lock().unwrap();
        if guard.cancel.is_some() || guard.operation.is_some() {
            return Err(HubError::Busy(session_id.to_string()));
        }
        let cancel = CancelToken::new();
        guard.operation = Some(cancel.clone());
        Ok(cancel)
    }

    /// 工具执行结束，释放 operation 坑位。
    pub fn release_operation(&self, session_id: &str) {
        if let Some(channel) = self.channel_if_present(session_id) {
            channel.lock().unwrap().operation = None;
        }
    }

    fn spawn_turn(self: &Arc<Self>, session_id: &str, cancel: CancelToken) {
        let hub = Arc::clone(self);
        let agent = Arc::clone(&self.agent);
        let id = session_id.to_string();
        let channel = self.channel(session_id);
        let handle = tokio::spawn(async move {
            let stream = agent.run_turn(id.clone(), cancel);
            futures_util::pin_mut!(stream);
            while let Some(agent_event) = stream.next().await {
                hub.publish(&id, &agent_event);
            }
            hub.finish(&id);
        });
        channel.lock().unwrap().turn_task = Some(handle);
    }

    // ── 分支 ─────────────────────────────────────────────────────
    //
    // 消息树的价值全在这几个操作上：没有它们，分叉存了也没人能用。
    // 它们都会改动树，所以一律要求当前没有轮次在跑。

    /// 列出所有分支端点。
    pub fn branches(&self, session_id: &str) -> Result<Vec<BranchInfo>, HubError> {
        let sessions = self.agent.sessions();
        let meta = sessions
            .get_session(session_id)?
            .ok_or_else(|| HubError::NotFound(session_id.to_string()))?;

        let mut branches = Vec::new();
        for leaf_id in sessions.list_leaves(session_id)? {
            let record = sessions.get_message(session_id, leaf_id)?;
            branches.push(BranchInfo {
                leaf_id,
                is_active: meta.active_leaf_id == Some(leaf_id),
                role: record.payload.role.as_str().to_string(),
                preview: preview_of(&record),
                created_at: record.created_at.to_rfc3339(),
            });
        }
        Ok(branches)
    }

    /// 整棵消息树。
    ///
    /// 界面用它画分叉图；点开某个节点就能看到那一步的思考、工具参数与耗时——
    /// 上一代要为此单独做一个调用追踪页，我们把每次 user / assistant / tool 都
    /// 存成了独立节点，追踪信息本来就在树里，不必另起一套。
    pub fn tree(&self, session_id: &str) -> Result<SessionTree, HubError> {
        let sessions = self.agent.sessions();
        let meta = sessions
            .get_session(session_id)?
            .ok_or_else(|| HubError::NotFound(session_id.to_string()))?;
        Ok(SessionTree {
            active_leaf_id: meta.active_leaf_id,
            leaves: sessions.list_leaves(session_id)?,
            nodes: sessions.list_messages(session_id)?,
        })
    }

    /// 切换到另一个分支。
    pub fn switch_branch(&self, session_id: &str, leaf_id: i64) -> Result<(), HubError> {
        self.ensure_idle(session_id)?;
        self.agent.sessions().switch_leaf(session_id, leaf_id)?;
        Ok(())
    }

    /// 删除一个叶节点。
    pub fn delete_message(&self, session_id: &str, message_id: i64) -> Result<(), HubError> {
        self.ensure_idle(session_id)?;
        self.agent.sessions().delete_leaf(session_id, message_id)?;
        Ok(())
    }

    /// 重新生成上一个回合。
    ///
    /// 回退到当前路径里**最后一条用户消息**再跑，而不是只重跑最后一次 LLM 调用
    /// ——用户点「重新生成」想要的是「换个答法回答我刚才的问题」，中间那几次工具
    /// 调用也该一起重来。旧分支原样留着，随时能切回去。
    pub fn regenerate(self: &Arc<Self>, session_id: &str) -> Result<(), HubError> {
        self.ensure_idle(session_id)?;
        let sessions = self.agent.sessions();
        let path = sessions.path_to_active_leaf(session_id)?;
        let last_user = path
            .iter()
            .rev()
            .find(|record| record.payload.role == MessageRole::User)
            .ok_or_else(|| HubError::Invalid("这个会话还没有用户消息，无从重新生成".into()))?;

        sessions.fork_at(session_id, Some(last_user.id))?;
        // 分叉换掉了整条可见路径，增量说不清，重推一份快照
        self.resync(session_id);
        self.start_turn(session_id)
    }

    /// 改掉某条用户消息并重新发送。
    ///
    /// 分叉到那条消息的父节点再追加新内容，于是旧问法与旧回答成为一条并列分支，
    /// 不会被抹掉。
    pub fn edit_and_resend(
        self: &Arc<Self>,
        session_id: &str,
        message_id: i64,
        text: &str,
    ) -> Result<(), HubError> {
        if text.trim().is_empty() {
            return Err(HubError::Invalid("消息不能为空".into()));
        }
        self.ensure_idle(session_id)?;

        let sessions = self.agent.sessions();
        let record = sessions.get_message(session_id, message_id)?;
        if record.payload.role != MessageRole::User {
            return Err(HubError::Invalid(format!(
                "消息 #{message_id} 不是用户消息，只能改自己发的"
            )));
        }

        sessions.fork_at(session_id, record.parent_id)?;
        self.resync(session_id);
        self.push_user_message(session_id, text)?;
        self.start_turn(session_id)
    }

    /// 追加一条用户消息，并告诉订阅者。
    ///
    /// 不广播的话，**发消息的人自己看不到自己发的消息**——用户消息由这里写进树，
    /// 而 agent 只为它自己做的事发事件。同一个会话在手机上开着、从电脑发一句，
    /// 手机那头就只会看到回复凭空冒出来。
    pub fn push_user_message(
        &self,
        session_id: &str,
        text: impl Into<String>,
    ) -> Result<MessageRecord, HubError> {
        let record =
            self.agent
                .sessions()
                .append(session_id, MessagePayload::user_text(text), false)?;
        self.publish(
            session_id,
            &AgentEvent::MessageCommitted {
                record: Box::new(record.clone()),
            },
        );
        Ok(record)
    }

    /// 把当前快照重新推给订阅者。
    ///
    /// 换分支时用。增量事件描述的是「树上多了/改了/少了一个节点」，而分叉换的是
    /// **整条可见路径**——那用增量表达不了，重发一份快照最直接，客户端整体替换
    /// 即可（和断线重连走同一条路）。
    pub fn resync(&self, session_id: &str) {
        let Ok(snapshot) = self.snapshot(session_id) else {
            return;
        };
        let Some(channel) = self.channel_if_present(session_id) else {
            return;
        };
        let seq = self.seq.fetch_add(1, Ordering::Relaxed);
        let payload = serde_json::to_value(&snapshot).unwrap_or_else(|_| serde_json::json!({}));
        let _ = channel.lock().unwrap().tx.send(Envelope {
            scope: Scope::Session(session_id.to_string()).as_wire(),
            kind: "snapshot".into(),
            seq,
            payload,
        });
    }

    /// 改树之前确认没有轮次在跑。
    fn ensure_idle(&self, session_id: &str) -> Result<(), HubError> {
        if self.is_running(session_id) {
            return Err(HubError::Busy(session_id.to_string()));
        }
        Ok(())
    }

    /// 停止当前轮或正在执行的挂起工具；没有可取消的任务时返回 false。
    pub fn stop(self: &Arc<Self>, session_id: &str) -> bool {
        let Some(channel) = self.channel_if_present(session_id) else {
            return false;
        };
        let cancelled = {
            let guard = channel.lock().unwrap();
            let mut did = false;
            if let Some(cancel) = &guard.cancel {
                cancel.cancel();
                did = true;
            }
            if let Some(cancel) = &guard.operation {
                cancel.cancel();
                did = true;
            }
            did
        };
        if cancelled {
            let hub = Arc::clone(self);
            let id = session_id.to_string();
            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_millis(2500)).await;
                if hub.is_running(&id) {
                    hub.force_abort_turn(&id);
                }
            });
            return true;
        }
        false
    }

    /// Stop 已发出但轮次仍占着坑（任务 panic、HTTP 连接迟迟不回等）时的兜底。
    fn force_abort_turn(self: &Arc<Self>, session_id: &str) {
        let Some(channel) = self.channel_if_present(session_id) else {
            return;
        };
        let (buffer, handle) = {
            let mut guard = channel.lock().unwrap();
            if guard.cancel.is_none() && guard.operation.is_none() {
                return;
            }
            let buffer = guard.buffer.clone();
            let handle = guard.turn_task.take();
            (buffer, handle)
        };
        if let Some(handle) = handle {
            handle.abort();
        }

        if let Some(buf) = buffer {
            if let Some(msg_id) = buf.message_id {
                let produced = !buf.content.trim().is_empty()
                    || !buf.reasoning.trim().is_empty()
                    || !buf.calls.is_empty();
                if produced {
                    if let Ok(record) = self.agent.sessions().get_message(session_id, msg_id) {
                        let mut payload = record.payload.clone();
                        payload.status = MessageStatus::Interrupted;
                        if let Ok(updated) =
                            self.agent.sessions().update_payload(session_id, msg_id, &payload)
                        {
                            self.publish(
                                session_id,
                                &AgentEvent::MessageUpdated {
                                    record: Box::new(updated),
                                },
                            );
                        }
                    }
                } else {
                    let _ = self.agent.sessions().delete_leaf(session_id, msg_id);
                    self.publish(session_id, &AgentEvent::MessageDeleted { id: msg_id });
                }
            }
        }

        self.publish(
            session_id,
            &AgentEvent::TurnEnd {
                reason: TurnEndReason::Cancelled,
            },
        );
        self.finish(session_id);
    }

    /// 更新缓冲并广播。
    ///
    /// 两件事必须在同一把锁里做：否则一个刚好在中间订阅进来的客户端，会既在快照
    /// 里看到这段增量、又收到它的事件，界面上就重了一段。
    fn publish(&self, session_id: &str, event: &AgentEvent) {
        let title = self.session_title(session_id);
        if let Some((kind, payload)) = event::notify_global(session_id, &title, event) {
            self.broadcast_global(kind, payload);
        }

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
            guard.operation = None;
            guard.buffer = None;
            guard.turn_task = None;
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
                operation: None,
                buffer: None,
                turn_task: None,
            }))
        }))
    }

    /// 只取已有通道，不创建。
    fn channel_if_present(&self, session_id: &str) -> Option<Arc<Mutex<SessionChannel>>> {
        self.channels.lock().unwrap().get(session_id).cloned()
    }

    fn session_title(&self, session_id: &str) -> String {
        self.agent
            .sessions()
            .get_session(session_id)
            .ok()
            .flatten()
            .map(|meta| meta.title)
            .filter(|title| !title.trim().is_empty())
            .unwrap_or_else(|| "未命名会话".into())
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
            buffer.provider_searches.clear();
        }
        AgentEvent::Delta(text) => buffer.content.push_str(text),
        AgentEvent::Reasoning(text) => buffer.reasoning.push_str(text),
        AgentEvent::MessageCommitted { record } => {
            if buffer.message_id.is_none() {
                buffer.message_id = Some(record.id);
            }
        }
        // 这两个不进缓冲：缓冲存的是「本轮还没落库的部分」，而它们说的正是
        // 库里发生了什么，订阅者拿事件直接改自己的消息列表即可
        AgentEvent::MessageUpdated { .. } | AgentEvent::MessageDeleted { .. } => {}
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
        AgentEvent::ProviderSearch {
            call_id,
            phase,
            query,
        } => {
            if let Some(slot) = buffer
                .provider_searches
                .iter_mut()
                .find(|s| s.call_id == *call_id)
            {
                slot.phase = phase.as_str().into();
                slot.query = query.clone();
            } else {
                buffer.provider_searches.push(ProviderSearchState {
                    call_id: call_id.clone(),
                    phase: phase.as_str().into(),
                    query: query.clone(),
                });
            }
        }
        AgentEvent::ToolBatchStarted { .. }
        | AgentEvent::AwaitHuman { .. }
        | AgentEvent::TurnEnd { .. } => {}
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

    /// 缓冲只关心 id，其余字段随便填。
    fn fake_record(id: i64) -> lya_session::MessageRecord {
        lya_session::MessageRecord {
            id,
            session_id: "s".into(),
            parent_id: None,
            sort_key: id,
            payload: lya_session::MessagePayload::assistant_text(
                "",
                lya_session::MessageStatus::Streaming,
            ),
            created_at: chrono::Utc::now(),
        }
    }

    #[test]
    fn deltas_accumulate() {
        let buffer = buffer_after(&[
            AgentEvent::RoundStarted { round: 1 },
            AgentEvent::MessageCommitted {
                record: Box::new(fake_record(7)),
            },
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
