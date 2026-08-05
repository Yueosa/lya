//! 模型清单（`models.toml`）。
//!
//! 每条模型按 **API 栈**（`completions` / `responses`）分别声明能力与透传参数。
//! 旧版顶层 `capabilities` 与扁平透传字段已删除——格式不对就加载失败。

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use lya_base::{ApiMode, CAPABILITY_TEXT};

use crate::error::ConfigError;

/// 某个 API 栈下的配置。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModeConfig {
    /// 此栈下模型具备的能力。
    pub capabilities: Vec<String>,
    /// 透传进该 API 请求体的字段（应含 `model`）。
    #[serde(default)]
    pub params: Map<String, Value>,
}

/// `models.toml` 的内容。
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ModelCatalog {
    /// 全部可用模型。
    pub models: Vec<ModelEntry>,
}

impl ModelCatalog {
    /// 按 id 查找。
    pub fn get(&self, id: &str) -> Option<&ModelEntry> {
        self.models.iter().find(|entry| entry.id == id)
    }

    /// 是否为空。
    pub fn is_empty(&self) -> bool {
        self.models.is_empty()
    }

    /// 全部 id。
    pub fn ids(&self) -> Vec<&str> {
        self.models.iter().map(|entry| entry.id.as_str()).collect()
    }

    /// 在指定栈下具备某项能力的第一个模型。
    pub fn first_with(&self, mode: ApiMode, capability: &str) -> Option<&ModelEntry> {
        self.models
            .iter()
            .find(|entry| entry.can(mode, capability))
    }

    /// 列出在指定栈下可用的模型。
    pub fn for_api_mode(&self, mode: ApiMode) -> Vec<&ModelEntry> {
        self.models
            .iter()
            .filter(|entry| entry.supports(mode))
            .collect()
    }

    /// 结构性校验：字段非空、id 不重复。
    pub(crate) fn validate(&self) -> Result<(), ConfigError> {
        let mut seen: Vec<&str> = Vec::with_capacity(self.models.len());
        for entry in &self.models {
            entry.validate()?;
            if seen.contains(&entry.id.as_str()) {
                return Err(ConfigError::Invalid(format!("模型 id 重复：{}", entry.id)));
            }
            seen.push(&entry.id);
        }
        Ok(())
    }
}

/// 一个模型条目。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelEntry {
    /// 内部标识，被 `default_model` 与会话引用。
    pub id: String,
    /// 展示名。
    pub name: String,
    /// API 基地址。
    pub base_url: String,
    /// API 密钥。
    pub api_key: String,
    /// 模型上下文窗口（token 量级）；lya 元数据，**不**透传 API。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_window: Option<u64>,
    /// 按 API 栈划分的配置；至少一项。
    pub modes: BTreeMap<String, ModeConfig>,
}

impl ModelEntry {
    fn validate(&self) -> Result<(), ConfigError> {
        for (field, value) in [
            ("id", &self.id),
            ("name", &self.name),
            ("base_url", &self.base_url),
            ("api_key", &self.api_key),
        ] {
            if value.trim().is_empty() {
                return Err(ConfigError::Invalid(format!(
                    "模型 {:?} 的 `{field}` 不能为空",
                    self.id
                )));
            }
        }
        if self.modes.is_empty() {
            return Err(ConfigError::Invalid(format!(
                "模型 {} 至少声明一个 [models.modes.*] 段",
                self.id
            )));
        }
        for (key, mode) in &self.modes {
            let Some(api_mode) = ApiMode::parse(key) else {
                return Err(ConfigError::Invalid(format!(
                    "模型 {} 的 modes 键 `{key}` 无效，只允许 completions / responses",
                    self.id
                )));
            };
            mode.validate(&self.id, api_mode)?;
        }
        Ok(())
    }

    /// 是否声明了某个 API 栈。
    pub fn supports(&self, mode: ApiMode) -> bool {
        self.modes.contains_key(mode.as_str())
    }

    /// 某栈下的配置。
    pub fn mode(&self, mode: ApiMode) -> Option<&ModeConfig> {
        self.modes.get(mode.as_str())
    }

    /// 某栈下是否具备某项能力。
    pub fn can(&self, mode: ApiMode, capability: &str) -> bool {
        self.mode(mode)
            .is_some_and(|cfg| cfg.capabilities.iter().any(|item| item == capability))
    }

    /// 某栈下的透传 params。
    pub fn params_for(&self, mode: ApiMode) -> Map<String, Value> {
        self.mode(mode)
            .map(|cfg| cfg.params.clone())
            .unwrap_or_default()
    }

    /// 某栈下的能力清单。
    pub fn capabilities_for(&self, mode: ApiMode) -> Vec<String> {
        self.mode(mode)
            .map(|cfg| cfg.capabilities.clone())
            .unwrap_or_default()
    }

    /// `api_key` 是否还是模板里的占位符。
    pub fn api_key_is_placeholder(&self) -> bool {
        let key = self.api_key.trim();
        key.starts_with('<') && key.ends_with('>')
    }
}

impl ModeConfig {
    fn validate(&self, model_id: &str, mode: ApiMode) -> Result<(), ConfigError> {
        if self.capabilities.is_empty() {
            return Err(ConfigError::Invalid(format!(
                "模型 {model_id} 的 modes.{} 缺少 capabilities",
                mode.as_str()
            )));
        }
        if !self.capabilities.iter().any(|c| c == CAPABILITY_TEXT) {
            return Err(ConfigError::Invalid(format!(
                "模型 {model_id} 的 modes.{} 须包含 text 能力",
                mode.as_str()
            )));
        }
        Ok(())
    }
}

/// 校验会话所选模型与 API 栈是否匹配。
pub fn validate_session_binding(
    catalog: &ModelCatalog,
    model_id: Option<&str>,
    default_model_id: &str,
    api_mode: ApiMode,
) -> Result<String, ConfigError> {
    let resolved = model_id
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .unwrap_or(default_model_id);
    let entry = catalog.get(resolved).ok_or_else(|| {
        ConfigError::Invalid(format!("未知模型 id：{resolved}"))
    })?;
    if !entry.supports(api_mode) {
        let hint = match api_mode {
            ApiMode::Responses => {
                "该模型未在 models.toml 配置 modes.responses（例如 Pro 仅 Completions）"
            }
            ApiMode::Completions => "该模型未在 models.toml 配置 modes.completions",
        };
        return Err(ConfigError::Invalid(format!(
            "模型「{}」不支持 {} 栈：{hint}。请换支持该栈的模型，或新建使用其他 API 栈的会话",
            entry.name,
            api_mode.as_str(),
        )));
    }
    Ok(resolved.to_string())
}
