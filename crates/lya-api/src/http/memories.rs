//! 记忆管理端点。
//!
//! **删除只在这里**：模型有读、写、检索，没有删。破坏性操作留给用户，这一点
//! 从设计之初就定下了。

use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use lya_llm::LlmClient;
use lya_memory::{Memory, MemoryHit, MemoryPatch, NewMemory};
use serde::Deserialize;

use lya_hub::{HubError, SessionHub};
use super::sessions::ApiError;

type Hub = State<Arc<SessionHub<LlmClient>>>;

/// 全部记忆，按更新时间倒序。
pub async fn list(State(hub): Hub) -> Result<Json<Vec<Memory>>, ApiError> {
    Ok(Json(hub.agent().memory().list()?))
}

/// 检索入参。
#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    /// 关键词。
    pub q: String,
    /// 最多返回几条。
    #[serde(default = "default_limit")]
    pub limit: usize,
}

fn default_limit() -> usize {
    20
}

/// 检索记忆。
pub async fn search(
    State(hub): Hub,
    Query(query): Query<SearchQuery>,
) -> Result<Json<Vec<MemoryHit>>, ApiError> {
    Ok(Json(
        hub.agent().memory().search(&query.q, query.limit.min(100))?,
    ))
}

/// 读取一条（含正文）。
pub async fn read(State(hub): Hub, Path(id): Path<i64>) -> Result<Json<Memory>, ApiError> {
    Ok(Json(hub.agent().memory().get(id)?))
}

/// 新建记忆的入参。
#[derive(Debug, Deserialize)]
pub struct CreateBody {
    /// 标题，唯一。
    pub title: String,
    /// 正文。
    pub body: String,
    /// 摘要。
    #[serde(default)]
    pub summary: String,
    /// 标签。
    #[serde(default)]
    pub tags: Vec<String>,
}

/// 新建一条。
pub async fn create(
    State(hub): Hub,
    Json(body): Json<CreateBody>,
) -> Result<(StatusCode, Json<Memory>), ApiError> {
    let memory = hub.agent().memory().create(NewMemory {
        title: body.title,
        body: body.body,
        summary: body.summary,
        tags: body.tags,
        source_session_id: None,
    })?;
    Ok((StatusCode::CREATED, Json(memory)))
}

/// 局部更新；没给的字段保持原值。
#[derive(Debug, Default, Deserialize)]
pub struct UpdateBody {
    /// 新标题。
    pub title: Option<String>,
    /// 新摘要。
    pub summary: Option<String>,
    /// 新正文。
    pub body: Option<String>,
    /// 新标签，整体替换。
    pub tags: Option<Vec<String>>,
}

/// 更新一条。
pub async fn update(
    State(hub): Hub,
    Path(id): Path<i64>,
    Json(body): Json<UpdateBody>,
) -> Result<Json<Memory>, ApiError> {
    let memory = hub.agent().memory().update(
        id,
        MemoryPatch {
            title: body.title,
            summary: body.summary,
            body: body.body,
            tags: body.tags,
        },
    )?;
    Ok(Json(memory))
}

/// 删除一条。
pub async fn delete(State(hub): Hub, Path(id): Path<i64>) -> Result<StatusCode, ApiError> {
    hub.agent().memory().delete(id)?;
    Ok(StatusCode::NO_CONTENT)
}

impl From<lya_memory::MemoryError> for ApiError {
    fn from(err: lya_memory::MemoryError) -> Self {
        use lya_memory::MemoryError;
        match err {
            MemoryError::NotFound(id) => HubError::NotFound(format!("memory #{id}")).into(),
            MemoryError::DuplicateTitle(_) | MemoryError::Invalid(_) => {
                HubError::Invalid(err.to_string()).into()
            }
            other => HubError::Invalid(other.to_string()).into(),
        }
    }
}
