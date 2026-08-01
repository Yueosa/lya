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
use lya_agent::CancelToken;
use lya_session::{CreateSession, MessagePayload, SessionMeta};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::sync::broadcast::error::RecvError;

use crate::event::Envelope;
use crate::hub::{BranchInfo, HubError, SessionHub, SessionTree, Snapshot};

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
    pub work_mode: Option<lya_mode::Mode>,
    /// 模型 id；不给则用配置默认。
    #[serde(default)]
    pub model_id: Option<String>,
}

/// 新建会话。
pub async fn create(
    State(hub): Hub,
    Json(body): Json<CreateBody>,
) -> Result<(StatusCode, Json<SessionMeta>), ApiError> {
    let meta = hub.agent().sessions().create_session(CreateSession {
        title: body.title,
        work_mode: body.work_mode.unwrap_or_default(),
        model_id: body.model_id,
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
    pub work_mode: Option<lya_mode::Mode>,
    /// 新模型 id；显式给 `null` 表示回退到默认模型。
    #[serde(default, deserialize_with = "double_option")]
    pub model_id: Option<Option<String>>,
    /// 启用的工具；显式给 `null` 表示启用全部。
    #[serde(default, deserialize_with = "double_option")]
    pub enabled_tools: Option<Option<Vec<String>>>,
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

    if let Some(title) = body.title {
        sessions.set_title(&id, title)?;
    }
    if let Some(mode) = body.work_mode {
        // 走 agent 而不是仓储：它会在树上留一条模式变更说明
        agent.switch_mode(&id, mode)?;
    }
    if let Some(model_id) = body.model_id {
        sessions.set_model(&id, model_id.as_deref())?;
    }
    if let Some(tools) = body.enabled_tools {
        sessions.set_enabled_tools(&id, tools.as_deref())?;
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
    hub.agent()
        .sessions()
        .append(&id, MessagePayload::user_text(body.text), false)?;
    hub.start_turn(&id)?;
    Ok(StatusCode::ACCEPTED)
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
            agent
                .resolve_tool_confirm(&id, approved, note.as_deref(), CancelToken::new())
                .await?
        }
        HitlBody::ModeChange { approved } => agent.resolve_mode_change(&id, approved)?,
    }
    // 答复完就接着跑，用户不用再点一次「继续」
    hub.start_turn(&id)?;
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
            // 会话层的「不是叶子」「消息不存在」也是请求问题，不是服务器故障
            HubError::Session(
                lya_session::SessionError::NotLeaf(_)
                | lya_session::SessionError::MessageNotFound(_)
                | lya_session::SessionError::Invalid(_),
            ) => StatusCode::BAD_REQUEST,
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
