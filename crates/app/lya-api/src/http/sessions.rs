//! 会话相关的端点。

use std::convert::Infallible;
use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use futures_util::stream::Stream;
use lya_action::FormAnswer;
use lya_config::{ApiMode, Config, validate_session_binding};
use lya_session::{CreateSession, SessionMeta, SessionStatus};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::sync::broadcast::error::RecvError;

use lya_hub::event::Envelope;
use lya_hub::{BranchInfo, HubError, SessionHub, SessionTree, Snapshot};

type Hub = State<Arc<SessionHub>>;

/// 列出活跃会话。
pub async fn list(State(hub): Hub) -> Result<Json<Vec<SessionMeta>>, ApiError> {
    Ok(Json(hub.agent().sessions().list_sessions()?))
}

/// 新建会话的入参。
#[derive(Debug, Default, Deserialize)]
pub struct CreateBody {
    /// 标题，可空。
    #[serde(default)]
    pub title: String,
    /// 工作模式；不给则用配置默认。
    #[serde(default)]
    pub work_mode: Option<lya_base::Mode>,
    /// 模型 id；不给则用配置默认。
    #[serde(default)]
    pub model_id: Option<String>,
    /// API 栈；不给则用 `runtime.agent.default_api_mode`。
    #[serde(default)]
    pub api_mode: Option<ApiMode>,
}

/// 新建会话。
pub async fn create(
    State(hub): Hub,
    Json(body): Json<CreateBody>,
) -> Result<(StatusCode, Json<SessionMeta>), ApiError> {
    let config = load_config()?;
    let default_id = config
        .default_model()
        .map(|entry| entry.id.as_str())
        .unwrap_or("");
    let api_mode = body
        .api_mode
        .unwrap_or(config.runtime.agent.default_api_mode);
    validate_session_binding(
        &config.models,
        body.model_id.as_deref(),
        default_id,
        api_mode,
    )
    .map_err(invalid_config)?;

    let meta = hub.agent().sessions().create_session(CreateSession {
        title: body.title,
        work_mode: body.work_mode.unwrap_or_default(),
        model_id: body.model_id,
        api_mode: Some(api_mode.as_str().into()),
        ..Default::default()
    })?;
    Ok((StatusCode::CREATED, Json(meta)))
}

/// 读取会话快照。
pub async fn snapshot(State(hub): Hub, Path(id): Path<String>) -> Result<Json<Snapshot>, ApiError> {
    Ok(Json(hub.snapshot(&id)?))
}

/// 会话可改字段；只改给出的那些。
#[derive(Debug, Default, Deserialize)]
pub struct PatchBody {
    /// 新标题。
    pub title: Option<String>,
    /// 新工作模式。
    pub work_mode: Option<lya_base::Mode>,
    /// 新模型 id；显式给 `null` 表示回退到默认模型。
    #[serde(default, deserialize_with = "double_option")]
    pub model_id: Option<Option<String>>,
    /// 启用的工具；显式给 `null` 表示启用全部。
    #[serde(default, deserialize_with = "double_option")]
    pub enabled_tools: Option<Option<Vec<String>>>,
    /// 归档或取回。归档后会话只能回看，不能再发消息。
    pub status: Option<SessionStatus>,
    /// 会话专属人设；显式给 `null` 表示回退到全局默认。
    #[serde(default, deserialize_with = "double_option")]
    pub persona: Option<Option<String>>,
    /// API 栈；仅空会话可改，有消息后锁定。
    #[serde(default)]
    pub api_mode: Option<ApiMode>,
}

/// 区分「没给这个字段」和「给了 null」。
fn double_option<'de, D, T>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer).map(Some)
}

/// 修改会话设置。
pub async fn patch(
    State(hub): Hub,
    Path(id): Path<String>,
    Json(body): Json<PatchBody>,
) -> Result<Json<SessionMeta>, ApiError> {
    let agent = hub.agent();
    let sessions = agent.sessions();

    // 栈和模型一起校验：换栈往往要顺带换模型，分开校验会卡在「旧模型 + 新栈」
    // 这个根本不会落库的中间态上。
    if body.api_mode.is_some() || body.model_id.is_some() {
        if body.api_mode.is_some() && !sessions.session_is_empty(&id)? {
            return Err(ApiError::bad_request(
                "已有消息的会话不能改 API 栈；要换栈请新建会话",
            ));
        }
        let session = sessions
            .get_session(&id)?
            .ok_or_else(|| HubError::NotFound(id.clone()))?;
        let api_mode = body
            .api_mode
            .unwrap_or_else(|| ApiMode::parse(&session.api_mode).unwrap_or(ApiMode::Completions));
        let model_id = match &body.model_id {
            Some(explicit) => explicit.clone(),
            None => session.model_id.clone(),
        };

        let config = load_config()?;
        let default_id = config
            .default_model()
            .map(|entry| entry.id.as_str())
            .unwrap_or("");
        validate_session_binding(&config.models, model_id.as_deref(), default_id, api_mode)
            .map_err(invalid_config)?;

        if let Some(mode) = body.api_mode {
            sessions.set_api_mode(&id, mode.as_str())?;
        }
        if body.model_id.is_some() {
            sessions.set_model(&id, model_id.as_deref())?;
        }
    }

    if let Some(title) = body.title {
        sessions.set_title(&id, title)?;
    }
    if let Some(mode) = body.work_mode {
        // 走 agent 而不是仓储：它会在树上留一条模式变更说明
        agent.switch_mode(&id, mode)?;
    }
    if let Some(tools) = body.enabled_tools {
        sessions.set_enabled_tools(&id, tools.as_deref())?;
    }
    if let Some(persona) = body.persona {
        sessions.set_persona(&id, persona.as_deref())?;
    }
    if let Some(status) = body.status {
        // 归档中途不该有轮次在跑，否则那一轮写回结果时会撞上只读
        if status == SessionStatus::Archived && hub.is_running(&id) {
            return Err(ApiError::bad_request("这个会话正在生成中，先停下再归档"));
        }
        match status {
            SessionStatus::Archived => sessions.archive_session(&id)?,
            SessionStatus::Active => sessions.unarchive_session(&id)?,
        }
        // 会话列表变了，别的页面也要跟着更新
        hub.broadcast_global("sessions_changed", serde_json::json!({ "id": id }));
    }

    sessions
        .get_session(&id)?
        .map(Json)
        .ok_or_else(|| HubError::NotFound(id).into())
}

/// 发消息。
#[derive(Debug, Deserialize)]
pub struct SendBody {
    /// 消息正文。
    pub text: String,
}

/// 追加一条用户消息并开跑。
///
/// 返回 202 就结束——正文从订阅流里出来。这样同一个会话在几个端上看到的是同一份
/// 流，而不是「谁发的谁才看得到响应」。
pub async fn send(
    State(hub): Hub,
    Path(id): Path<String>,
    Json(body): Json<SendBody>,
) -> Result<StatusCode, ApiError> {
    if body.text.trim().is_empty() {
        return Err(ApiError::bad_request("消息不能为空"));
    }
    // 走 hub 而不是直接写 store：它会把这条消息广播出去，否则订阅者
    // （包括发消息的人自己）看不到它
    hub.send_user_message_and_start(&id, &body.text)?;
    Ok(StatusCode::ACCEPTED)
}

/// 真删一个会话。
///
/// 和归档是两回事：归档只是收起来、仍可回看，这个是从库里去掉，不可恢复。
/// 界面上必须先问一句再调。
pub async fn remove(State(hub): Hub, Path(id): Path<String>) -> Result<StatusCode, ApiError> {
    // 正在跑的时候删掉，那一轮还会往一张不存在的表里写
    if hub.is_running(&id) {
        return Err(ApiError::bad_request("这个会话正在生成中，先停下再删除"));
    }
    hub.agent().sessions().delete_session(&id)?;
    // 库里没了，盘上那份也不该留：会话不在了，它的媒体再没有界面能看到。
    // 删不掉也不该让删除会话失败——库里已经没了，重试也不会成功
    if let Err(err) = lya_media::remove_session_media(&id) {
        eprintln!("会话 {id} 的媒体目录没删干净：{err}");
    }
    hub.broadcast_global("sessions_changed", serde_json::json!({ "id": id }));
    Ok(StatusCode::NO_CONTENT)
}

/// 已归档的会话。
pub async fn archived(State(hub): Hub) -> Result<Json<Vec<SessionMeta>>, ApiError> {
    Ok(Json(hub.agent().sessions().list_archived()?))
}

/// 整棵消息树，供分叉图与逐节点回看。
pub async fn tree(State(hub): Hub, Path(id): Path<String>) -> Result<Json<SessionTree>, ApiError> {
    Ok(Json(hub.tree(&id)?))
}

/// 列出分支端点。
pub async fn branches(
    State(hub): Hub,
    Path(id): Path<String>,
) -> Result<Json<Vec<BranchInfo>>, ApiError> {
    Ok(Json(hub.branches(&id)?))
}

/// 切换分支。
#[derive(Debug, Deserialize)]
pub struct SwitchBody {
    /// 要切到哪个叶节点。
    pub leaf_id: i64,
}

/// 切到另一条分支。
pub async fn switch_branch(
    State(hub): Hub,
    Path(id): Path<String>,
    Json(body): Json<SwitchBody>,
) -> Result<Json<Snapshot>, ApiError> {
    hub.switch_branch(&id, body.leaf_id)?;
    Ok(Json(hub.snapshot(&id)?))
}

/// 重新生成上一个回合。
pub async fn regenerate(State(hub): Hub, Path(id): Path<String>) -> Result<StatusCode, ApiError> {
    hub.regenerate(&id)?;
    Ok(StatusCode::ACCEPTED)
}

/// 改掉一条用户消息并重发。
#[derive(Debug, Deserialize)]
pub struct EditBody {
    /// 新的消息正文。
    pub text: String,
}

/// 编辑重发。
pub async fn edit_message(
    State(hub): Hub,
    Path((id, message_id)): Path<(String, i64)>,
    Json(body): Json<EditBody>,
) -> Result<StatusCode, ApiError> {
    hub.edit_and_resend(&id, message_id, &body.text)?;
    Ok(StatusCode::ACCEPTED)
}

/// 删除一个叶节点。
pub async fn delete_message(
    State(hub): Hub,
    Path((id, message_id)): Path<(String, i64)>,
) -> Result<StatusCode, ApiError> {
    hub.delete_message(&id, message_id)?;
    Ok(StatusCode::NO_CONTENT)
}

/// 停止当前轮。
pub async fn stop(State(hub): Hub, Path(id): Path<String>) -> StatusCode {
    if hub.stop(&id) {
        StatusCode::ACCEPTED
    } else {
        StatusCode::NO_CONTENT
    }
}

/// HITL 答复。
///
/// 三种打断合用一个端点：它们共享「结清当前挂起、让本轮能接着跑」这个语义，
/// 拆成三个只会让前端多记三个地址。
#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum HitlBody {
    /// 提交表单作答。
    Form {
        /// 作答内容。
        answer: FormAnswer,
    },
    /// 放行或拒绝一次工具执行。
    Confirm {
        /// 是否放行。
        approved: bool,
        /// 附带的备注，会混进结果给模型看。
        #[serde(default)]
        note: Option<String>,
    },
    /// 同意或拒绝切换工作模式。
    ModeChange {
        /// 是否同意。
        approved: bool,
    },
}

/// 答复当前挂起，并自动接着跑下一轮。
pub async fn hitl(
    State(hub): Hub,
    Path(id): Path<String>,
    Json(body): Json<HitlBody>,
) -> Result<StatusCode, ApiError> {
    let agent = hub.agent();
    match body {
        HitlBody::Form { answer } => agent.submit_form(&id, &answer)?,
        HitlBody::Confirm { approved, note } => {
            let cancel = hub.reserve_operation(&id)?;
            let result = agent
                .resolve_tool_confirm(&id, approved, note.as_deref(), cancel)
                .await;
            hub.release_operation(&id);
            result?
        }
        HitlBody::ModeChange { approved } => agent.resolve_mode_change(&id, approved)?,
    }

    // 结清一次挂起会同时改两条消息：追加一条工具结果，再把 HITL 节点翻成
    // resolved。两条都不在事件流里，订阅者因此不知道它已经结清——界面上那个
    // 等你答复的托盘会一直挂着。两条一起变，增量说不清，重推一份快照最直接。
    hub.resync(&id);

    if agent.sessions().pending_hitl(&id)?.is_none() {
        let cancel = hub.reserve_operation(&id)?;
        let flush = agent.flush_deferred_tool_executions(&id, cancel).await;
        hub.release_operation(&id);
        flush?;
        hub.start_turn(&id)?;
    }
    Ok(StatusCode::ACCEPTED)
}

/// 订阅会话事件流。
pub async fn subscribe(
    State(hub): Hub,
    Path(id): Path<String>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, ApiError> {
    let (snapshot, mut rx) = hub.subscribe(&id)?;
    let hub_for_resync = Arc::clone(&hub);
    let session_id = id.clone();

    let stream = async_stream_helper(async_stream::stream! {
        yield sse_event("snapshot", &Envelope {
            scope: format!("session:{session_id}"),
            kind: "snapshot".into(),
            seq: 0,
            payload: serde_json::to_value(&snapshot).unwrap_or_else(|_| json!({})),
        });

        loop {
            match rx.recv().await {
                Ok(envelope) => yield sse_event(&envelope.kind.clone(), &envelope),
                // 订阅者跟不上时不要断开，补一份快照就能对齐——
                // 快照是累积的，客户端整体替换即可
                Err(RecvError::Lagged(_)) => {
                    if let Ok(fresh) = hub_for_resync.snapshot(&session_id) {
                        yield sse_event("snapshot", &Envelope {
                            scope: format!("session:{session_id}"),
                            kind: "snapshot".into(),
                            seq: 0,
                            payload: serde_json::to_value(&fresh).unwrap_or_else(|_| json!({})),
                        });
                    }
                }
                Err(RecvError::Closed) => break,
            }
        }
    });

    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}

/// 订阅全局事件流。
///
/// 与会话流分开是眼下最简单的做法；因为信封自带 `scope`，将来若要合并成一条
/// 连接同时承载两者，客户端的分发逻辑不用改。
pub async fn subscribe_global(
    State(hub): Hub,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let mut rx = hub.subscribe_global();
    let stream = async_stream_helper(async_stream::stream! {
        loop {
            match rx.recv().await {
                Ok(envelope) => yield sse_event(&envelope.kind.clone(), &envelope),
                // 全局事件都是「有变化，去重新拉一下」的通知，漏几条不影响，
                // 客户端下次收到照常处理
                Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => break,
            }
        }
    });
    Sse::new(stream).keep_alive(KeepAlive::default())
}

/// 让类型推断看清返回的是什么流。
fn async_stream_helper<S>(stream: S) -> S
where
    S: Stream<Item = Result<Event, Infallible>>,
{
    stream
}

/// 拼一条 SSE 事件。
fn sse_event(kind: &str, envelope: &Envelope) -> Result<Event, Infallible> {
    let data = serde_json::to_string(envelope).unwrap_or_else(|_| "{}".into());
    Ok(Event::default().event(kind).data(data))
}

/// HTTP 层错误。
#[derive(Debug)]
pub struct ApiError {
    /// HTTP 状态码。
    status: StatusCode,
    message: String,
}

impl ApiError {
    fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: message.into(),
        }
    }
}

impl From<HubError> for ApiError {
    fn from(err: HubError) -> Self {
        let status = match &err {
            HubError::NotFound(_) => StatusCode::NOT_FOUND,
            // 已经有一轮在跑，前端应当提示「正在回复中」而不是重试
            HubError::Busy(_) => StatusCode::CONFLICT,
            HubError::Invalid(_) => StatusCode::BAD_REQUEST,
            HubError::Agent(lya_agent::AgentError::Invalid(_)) => StatusCode::BAD_REQUEST,
            // 会话层的「不是叶子」「消息不存在」也是请求问题，不是服务器故障
            HubError::Session(
                lya_session::SessionError::NotLeaf(_)
                | lya_session::SessionError::MessageNotFound(_)
                | lya_session::SessionError::Invalid(_),
            ) => StatusCode::BAD_REQUEST,
            // 归档与未决 HITL 都是「现在这个会话不接受写入」，属于状态冲突而不是
            // 请求写错了——前端据此提示「先取回归档」或「先答复上面那个」
            HubError::Session(
                lya_session::SessionError::Archived(_) | lya_session::SessionError::PendingHitl(_),
            ) => StatusCode::CONFLICT,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };
        Self {
            status,
            message: err.to_string(),
        }
    }
}

impl From<lya_session::SessionError> for ApiError {
    fn from(err: lya_session::SessionError) -> Self {
        HubError::Session(err).into()
    }
}

impl From<lya_agent::AgentError> for ApiError {
    fn from(err: lya_agent::AgentError) -> Self {
        HubError::Agent(err).into()
    }
}

/// 错误统一序列化成 `{"error": "…"}`。
#[derive(Serialize)]
struct ErrorBody<'a> {
    error: &'a str,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(ErrorBody {
                error: &self.message,
            }),
        )
            .into_response()
    }
}

fn load_config() -> Result<Config, ApiError> {
    Config::load().map_err(invalid_config)
}

fn invalid_config(err: lya_config::ConfigError) -> ApiError {
    ApiError {
        status: StatusCode::BAD_REQUEST,
        message: err.to_string(),
    }
}
