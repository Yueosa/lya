//! 白盒端点：让用户看得见模型手里到底有什么。
//!
//! 工具和动作的名字、描述、参数 schema、权限与流转全部照实暴露。用户不该靠猜
//! 来判断助手能做什么、不能做什么。

use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, Query, State};
use lya_llm::LlmClient;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use lya_hub::{HubError, SessionHub};
use super::sessions::ApiError;

type Hub = State<Arc<SessionHub<LlmClient>>>;

/// 一个工具的对外描述。
#[derive(Debug, Serialize)]
pub struct ToolInfo {
    /// 内部名，也是模型看到的函数名。
    pub name: String,
    /// 展示名。
    pub raw_name: String,
    /// 短描述。
    pub description: String,
    /// 权限位，形如 `-R-W-`。
    pub permission: String,
    /// 最低需要哪种工作模式才看得见。
    pub min_mode: &'static str,
    /// 参数 JSON Schema。
    pub parameters: Value,
    /// 详细用法说明（就是喂给模型的那份）。
    pub prompt_hint: String,
    /// 在所查会话里是否启用；不带 `session` 参数时为 `None`。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
}

/// 一个动作的对外描述。
#[derive(Debug, Serialize)]
pub struct ActionInfo {
    /// 内部名。
    pub name: String,
    /// 展示名。
    pub raw_name: String,
    /// 短描述。
    pub description: String,
    /// 执行后的流转：`continue` 或 `await_human`。
    pub flow: &'static str,
    /// 参数 JSON Schema。
    pub parameters: Value,
    /// 详细用法说明。
    pub prompt_hint: String,
    /// 在哪些模式下可见。
    pub visible_in: Vec<&'static str>,
}

/// 可选的会话上下文：给了就顺带告知该会话的启用情况。
#[derive(Debug, Default, Deserialize)]
pub struct ScopeQuery {
    /// 会话 id。
    pub session: Option<String>,
}

/// 列出全部工具。
pub async fn tools(
    State(hub): Hub,
    Query(query): Query<ScopeQuery>,
) -> Result<Json<Vec<ToolInfo>>, ApiError> {
    let agent = hub.agent();

    // 带了会话就顺带算出它实际生效的名单，界面才好画开关
    let enabled = match &query.session {
        Some(id) => {
            let meta = agent
                .sessions()
                .get_session(id)?
                .ok_or_else(|| HubError::NotFound(id.clone()))?;
            Some(agent.effective_tools(&meta))
        }
        None => None,
    };

    let infos = agent
        .tools()
        .list()
        .map(|tool| {
            let meta = tool.meta();
            ToolInfo {
                name: meta.name.clone(),
                raw_name: meta.raw_name.clone(),
                description: meta.desc.clone(),
                permission: meta.prmt.to_string(),
                min_mode: min_mode_for(meta.prmt),
                parameters: tool.parameters().clone(),
                prompt_hint: tool.prompt_hint().to_string(),
                enabled: enabled.as_ref().map(|names| match names {
                    // 没有名单就是全部启用
                    None => true,
                    Some(list) => list.contains(&meta.name),
                }),
            }
        })
        .collect();
    Ok(Json(infos))
}

/// 列出全部动作。
pub async fn actions(State(hub): Hub) -> Json<Vec<ActionInfo>> {
    let infos = hub
        .agent()
        .actions()
        .list()
        .map(|action| {
            let meta = action.meta();
            ActionInfo {
                name: meta.name.clone(),
                raw_name: meta.raw_name.clone(),
                description: meta.desc.clone(),
                flow: match meta.flow {
                    lya_action::ActionFlow::Continue => "continue",
                    lya_action::ActionFlow::AwaitHuman => "await_human",
                },
                parameters: action.parameters().clone(),
                prompt_hint: action.prompt_hint().to_string(),
                visible_in: [lya_mode::Mode::Ask, lya_mode::Mode::Edit, lya_mode::Mode::Agent]
                    .into_iter()
                    .filter(|mode| action.visible_in(*mode))
                    .map(|mode| mode.as_str())
                    .collect(),
            }
        })
        .collect();
    Json(infos)
}

/// 改会话的工具启用名单。
#[derive(Debug, Deserialize)]
pub struct ToggleBody {
    /// 是否启用。
    pub enabled: bool,
}

/// 单独开关某个工具。
///
/// 比整份 `PATCH` 名单方便：界面上就是一个个开关。会话此前没自定义过名单时，
/// 会先按当前生效的集合展开成显式列表，再增删。
pub async fn toggle_tool(
    State(hub): Hub,
    Path((session_id, tool_name)): Path<(String, String)>,
    Json(body): Json<ToggleBody>,
) -> Result<Json<Vec<String>>, ApiError> {
    let agent = hub.agent();
    if agent.tools().get(&tool_name).is_none() {
        return Err(HubError::Invalid(format!("没有名为 `{tool_name}` 的工具")).into());
    }
    let meta = agent
        .sessions()
        .get_session(&session_id)?
        .ok_or_else(|| HubError::NotFound(session_id.clone()))?;

    let mut names = agent
        .effective_tools(&meta)
        .unwrap_or_else(|| agent.tools().names());
    names.retain(|name| name != &tool_name);
    if body.enabled {
        names.push(tool_name);
    }
    names.sort();

    agent.sessions().set_enabled_tools(&session_id, Some(&names))?;
    Ok(Json(names))
}

/// 该权限最低需要哪个模式。
fn min_mode_for(permission: lya_tool::Permission) -> &'static str {
    use lya_tool::Permission;
    if permission.is_subset_of(Permission::READ_ONLY) {
        "ask"
    } else if permission.is_subset_of(Permission::READ_WRITE) {
        "edit"
    } else {
        "agent"
    }
}
