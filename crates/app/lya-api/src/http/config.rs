//! 配置读写与模型探测。
//!
//! `core.toml` 只读展示——端口、日志、库路径这些改了要重启进程才生效，界面上能改
//! 却不生效比不给改更让人困惑。其余三个文件可写，写回时由 `lya-config` 用
//! `toml_edit` 保住注释与字段顺序。
//!
//! 每次写入都广播一条 `global` 事件：多端场景下，手机改了默认模型，网页端的设置页
//! 得跟着变。这也是 LyaSSE 里 `global` 作用域的第一个真实产出方。

use std::collections::BTreeMap;
use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use lya_config::{Config, CoreConfig, ModelEntry, RuntimeConfig};
use lya_llm::LlmClient;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use lya_hub::{HubError, SessionHub};
use super::sessions::ApiError;

type Hub = State<Arc<SessionHub<LlmClient>>>;

/// 配置总览。
#[derive(Debug, Serialize)]
pub struct ConfigView {
    /// 进程级配置。
    pub core: CoreConfig,
    /// 运行时默认值。
    pub runtime: RuntimeConfig,
    /// 模型清单；密钥已打码。
    pub models: Vec<MaskedModel>,
    /// 全局人设。
    pub persona: Option<String>,
    /// 告诉界面 core 那一段不可改。
    pub core_readonly: bool,
}

/// 某个 API 栈在界面上的摘要。
#[derive(Debug, Serialize)]
pub struct MaskedModeView {
    /// 此栈下的能力标签。
    pub capabilities: Vec<String>,
}

/// 打码后的模型条目。
///
/// 界面需要知道「配没配密钥」，但没必要把密钥本身发进浏览器。
#[derive(Debug, Serialize)]
pub struct MaskedModel {
    /// 内部 id。
    pub id: String,
    /// 展示名。
    pub name: String,
    /// API 基地址。
    pub base_url: String,
    /// 打码后的密钥。
    pub api_key_masked: String,
    /// 密钥是否还是模板占位符。
    pub api_key_placeholder: bool,
    /// 上下文窗口（token）；lya 元数据，不透传 API。
    pub context_window: Option<u64>,
    /// 按 API 栈划分的能力；前端据此过滤可选模型。
    pub modes: BTreeMap<String, MaskedModeView>,
}

/// 前端启动时要拿的一次性信息。
#[derive(Debug, Serialize)]
pub struct Bootstrap {
    /// 访问 `/api/local-image` 所需的令牌。
    ///
    /// 只能从这里拿：跨域 `fetch` 一定带 `Origin`、会被守卫挡掉，跨域
    /// `<script>` / `<img>` 又读不到 JSON，所以恶意页面拿不到它。
    pub image_token: String,
    /// 家目录，前端据此判断一个路径是否属于本机可显示的图片。
    pub home: Option<String>,
    /// `runtime.toml` 的默认模型 id；会话 `model_id` 为空时后端用这个。
    pub default_model_id: Option<String>,
    /// 默认模型的展示名。
    pub default_model_name: Option<String>,
}

/// 前端启动握手。
pub async fn bootstrap(State(hub): Hub) -> Json<Bootstrap> {
    let default = load().ok().and_then(|config| {
        config.default_model().map(|entry| (entry.id.clone(), entry.name.clone()))
    });
    Json(Bootstrap {
        image_token: hub.image_token().to_string(),
        home: std::env::var_os("HOME").map(|home| home.to_string_lossy().into_owned()),
        default_model_id: default.as_ref().map(|(id, _)| id.clone()),
        default_model_name: default.map(|(_, name)| name),
    })
}

/// 读取全部配置。
pub async fn read(State(_hub): Hub) -> Result<Json<ConfigView>, ApiError> {
    let config = load()?;
    Ok(Json(ConfigView {
        core: config.core,
        runtime: config.runtime,
        models: config.models.models.iter().map(mask).collect(),
        persona: config.persona,
        core_readonly: true,
    }))
}

/// 模型清单（供界面选择）。
pub async fn models(State(_hub): Hub) -> Result<Json<Vec<MaskedModel>>, ApiError> {
    Ok(Json(load()?.models.models.iter().map(mask).collect()))
}

/// 要改的表名与键值。
#[derive(Debug, Deserialize)]
pub struct RuntimeBody {
    /// 形如 `{"agent": {"max_tool_rounds": 8}}`，只覆盖提到的键。
    #[serde(flatten)]
    pub tables: serde_json::Map<String, Value>,
}

/// 改 `runtime.toml`。
pub async fn write_runtime(
    State(hub): Hub,
    Json(body): Json<RuntimeBody>,
) -> Result<Json<RuntimeConfig>, ApiError> {
    let dir = lya_config::data_root().map_err(invalid)?;
    lya_config::write_runtime(&dir, &body.tables).map_err(invalid)?;

    // 立刻回读：既验证写出去的东西还解析得通，也拿到生效后的值
    let config = load()?;
    apply(&hub)?;
    hub.broadcast_global("config_changed", json!({ "file": "runtime" }));
    Ok(Json(config.runtime))
}

/// 人设正文。
#[derive(Debug, Deserialize)]
pub struct PersonaBody {
    /// 空字符串表示回退到内置默认人设。
    pub text: String,
}

/// 改 `persona.toml`。
pub async fn write_persona(
    State(hub): Hub,
    Json(body): Json<PersonaBody>,
) -> Result<StatusCode, ApiError> {
    let dir = lya_config::data_root().map_err(invalid)?;
    lya_config::write_persona(&dir, &body.text).map_err(invalid)?;
    apply(&hub)?;
    hub.broadcast_global("config_changed", json!({ "file": "persona" }));
    Ok(StatusCode::NO_CONTENT)
}

/// 把刚写进文件的配置推给运行中的组件。
///
/// 少了这一步，界面读磁盘（立刻是新的）、模型读进程内存（还是启动那一刻的），
/// 于是「改完人设显示已生效，模型却还用旧的」——两个真相来源只更新了一个。
fn apply(hub: &SessionHub<LlmClient>) -> Result<(), ApiError> {
    hub.reload_config()
        .map_err(|err| ApiError::from(HubError::Invalid(format!("配置已写入，但重新加载失败：{err}"))))
}

/// 取某个配置文件的原文，供「高级编辑」直接看 TOML。
///
/// `models` 会脱敏 `api_key`，避免完整密钥进浏览器。
pub async fn raw(Path(file): Path<String>) -> Result<String, ApiError> {
    let name = match file.as_str() {
        "core" => lya_config::CORE_FILE,
        "runtime" => lya_config::RUNTIME_FILE,
        "models" => lya_config::MODELS_FILE,
        "persona" => lya_config::PERSONA_FILE,
        other => return Err(HubError::Invalid(format!("没有名为 {other} 的配置文件")).into()),
    };
    let path = lya_config::data_root().map_err(invalid)?.join(name);
    let text = std::fs::read_to_string(&path).map_err(|err| {
        ApiError::from(HubError::Invalid(format!(
            "{} 读取失败：{err}",
            path.display()
        )))
    })?;
    if file == "models" {
        return Ok(lya_config::redact_models_toml(&text).map_err(invalid)?);
    }
    Ok(text)
}

/// 探测入参。
///
/// 两种用法：给 `model_id` 测一个**已配置**的模型（用服务器上存的真密钥），
/// 或者给 `base_url` + `api_key` 测一对还没写进配置的新凭据。
#[derive(Debug, Deserialize)]
pub struct ProbeBody {
    /// 已配置模型的 id。
    ///
    /// 给了它就不用再传密钥——界面手里只有脱敏后的那串，而真密钥不该为了测一下
    /// 就发到浏览器里再原样发回来。
    #[serde(default)]
    pub model_id: Option<String>,
    /// API 基地址；给了 `model_id` 就不用填。
    #[serde(default)]
    pub base_url: String,
    /// API 密钥；给了 `model_id` 就不用填。
    #[serde(default)]
    pub api_key: String,
}

/// 探测结果。
#[derive(Debug, Serialize)]
pub struct ProbeResult {
    /// 是否连通。
    pub ok: bool,
    /// 该供应商声明支持的模型 id。
    pub models: Vec<String>,
    /// 失败原因。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// 用 base_url + key 打一次 `GET /models`。
///
/// 选 `/models` 而不是发一次试探性对话：它不花 token，而且几乎所有 OpenAI 兼容
/// 服务都实现了。这把「手填模型 id 猜对不对」变成「点一下看列表」。
///
/// 连不通不算服务器错误——那是探测的正常结果之一，所以照常返回 200，用 `ok`
/// 字段表达成败，界面才好显示原因。
pub async fn probe(State(hub): Hub, Json(body): Json<ProbeBody>) -> Json<ProbeResult> {
    // 给了 model_id 就从配置里取真密钥，界面因此不必持有它
    let (base_url, api_key) = match &body.model_id {
        Some(id) => {
            let Ok(config) = load() else {
                return Json(ProbeResult::failed("读不到配置".into()));
            };
            match config.models.models.iter().find(|entry| &entry.id == id) {
                Some(entry) => (entry.base_url.clone(), entry.api_key.clone()),
                None => return Json(ProbeResult::failed(format!("没有名为 {id} 的模型"))),
            }
        }
        None => (body.base_url.clone(), body.api_key.clone()),
    };

    let url = format!("{}/models", base_url.trim_end_matches('/'));
    let http = hub.http();
    let request = http
        .get(&url)
        .header("Authorization", format!("Bearer {api_key}"));

    let response = match http.send(request).await {
        Ok(response) => response,
        Err(err) => return Json(ProbeResult::failed(format!("连不上：{err}"))),
    };
    if !response.is_success() {
        let status = response.status();
        return Json(ProbeResult::failed(format!("对端返回 HTTP {status}")));
    }
    let payload: Value = match response.json().await {
        Ok(payload) => payload,
        Err(err) => return Json(ProbeResult::failed(format!("响应不是合法 JSON：{err}"))),
    };

    let mut models: Vec<String> = payload["data"]
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item["id"].as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    models.sort();
    Json(ProbeResult {
        ok: true,
        models,
        error: None,
    })
}

impl ProbeResult {
    fn failed(reason: String) -> Self {
        Self {
            ok: false,
            models: Vec::new(),
            error: Some(reason),
        }
    }
}

fn load() -> Result<Config, ApiError> {
    Config::load().map_err(invalid).map_err(Into::into)
}

fn invalid(err: lya_config::ConfigError) -> HubError {
    HubError::Invalid(err.to_string())
}

/// 打码：留前三位和后四位，够辨认是哪一个就行。
fn mask(entry: &ModelEntry) -> MaskedModel {
    let key = entry.api_key.trim();
    let masked = if entry.api_key_is_placeholder() {
        "（未填写）".to_string()
    } else if key.chars().count() <= 8 {
        "…".to_string()
    } else {
        let head: String = key.chars().take(3).collect();
        let tail: String = key
            .chars()
            .rev()
            .take(4)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        format!("{head}…{tail}")
    };
    let modes = entry
        .modes
        .iter()
        .map(|(name, cfg)| {
            (
                name.clone(),
                MaskedModeView {
                    capabilities: cfg.capabilities.clone(),
                },
            )
        })
        .collect();
    MaskedModel {
        id: entry.id.clone(),
        name: entry.name.clone(),
        base_url: entry.base_url.clone(),
        api_key_masked: masked,
        api_key_placeholder: entry.api_key_is_placeholder(),
        context_window: entry.context_window,
        modes,
    }
}
