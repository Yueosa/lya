//! 模型端点描述。
//!
//! 由配置层（如读 `model.toml`）填好后注入本 crate；`lya-llm` 不读文件。

use std::collections::BTreeMap;

use serde_json::{Map, Value};

use crate::error::LlmError;
use lya_base::ApiMode;

/// 一次 LLM 调用所指向的端点。
///
/// - [`LlmEndpoint::base_url`]：形如 `https://api.deepseek.com/v1`
/// - [`LlmEndpoint::api_key`]：Bearer token
/// - [`LlmEndpoint::params`]：按 [`ApiMode`] 合并进请求 JSON 的额外字段
///   （至少应含 `model`；也可含 `temperature`、思考相关字段等）
#[derive(Debug, Clone)]
pub struct LlmEndpoint {
    /// 逻辑 id（来自配置，便于日志；请求本身不强制使用）。
    pub id: String,
    /// API 根路径（不含 `/chat/completions` 或 `/responses`）。
    pub base_url: String,
    /// Bearer API Key。
    pub api_key: String,
    /// 输入上下文上限（token）；lya 元数据，来自 `models.toml`，不发给 API。
    pub context_window: Option<u64>,
    mode_params: BTreeMap<String, Map<String, Value>>,
    mode_capabilities: BTreeMap<String, Vec<String>>,
}

impl LlmEndpoint {
    /// 用 base_url + api_key 构造；`id` 默认为 `"default"`，无 params。
    pub fn new(base_url: impl Into<String>, api_key: impl Into<String>) -> Self {
        Self {
            id: "default".into(),
            base_url: base_url.into(),
            api_key: api_key.into(),
            context_window: None,
            mode_params: BTreeMap::new(),
            mode_capabilities: BTreeMap::new(),
        }
    }

    /// 设置输入上下文上限（token）。
    pub fn with_context_window(mut self, limit: Option<u64>) -> Self {
        self.context_window = limit;
        self
    }

    /// 有效上下文上限；未配置时用 1M。
    pub fn effective_context_window(&self) -> u64 {
        self.context_window.unwrap_or(1_048_576)
    }

    /// 设置逻辑 id。
    pub fn with_id(mut self, id: impl Into<String>) -> Self {
        self.id = id.into();
        self
    }

    /// 写入 / 覆盖 completions 栈的一个 param（如 `model`）。
    pub fn with_param(mut self, key: impl Into<String>, value: Value) -> Self {
        self.mode_params
            .entry(ApiMode::Completions.as_str().into())
            .or_default()
            .insert(key.into(), value);
        self
    }

    /// 写入 / 覆盖某 API 栈的一个 param。
    pub fn with_mode_param(mut self, mode: ApiMode, key: impl Into<String>, value: Value) -> Self {
        self.mode_params
            .entry(mode.as_str().into())
            .or_default()
            .insert(key.into(), value);
        self
    }

    /// 批量合并 completions params。
    pub fn with_params(self, params: Map<String, Value>) -> Self {
        self.with_mode_params(ApiMode::Completions, params)
    }

    /// 批量合并某 API 栈的 params。
    pub fn with_mode_params(mut self, mode: ApiMode, params: Map<String, Value>) -> Self {
        self.mode_params.insert(mode.as_str().into(), params);
        self
    }

    /// 声明某 API 栈下的能力列表（来自 `models.toml` capabilities）。
    pub fn with_mode_capabilities(mut self, mode: ApiMode, capabilities: Vec<String>) -> Self {
        self.mode_capabilities
            .insert(mode.as_str().into(), capabilities);
        self
    }

    /// 某 API 栈是否声明了给定能力。
    pub fn supports(&self, mode: ApiMode, capability: &str) -> bool {
        self.mode_capabilities
            .get(mode.as_str())
            .is_some_and(|caps| caps.iter().any(|c| c == capability))
    }

    /// 取出某 API 栈的透传 params。
    pub fn params(&self, mode: ApiMode) -> Result<&Map<String, Value>, LlmError> {
        self.mode_params.get(mode.as_str()).ok_or_else(|| {
            LlmError::Other(format!(
                "模型 {} 未配置 modes.{}（请在 models.toml 为该栈添加 params，或新建 Completions 会话）",
                self.id,
                mode.as_str()
            ))
        })
    }

    /// 拼出 `…/chat/completions` URL。
    ///
    /// 会去掉 `base_url` 尾部 `/`，再追加路径。
    pub fn chat_completions_url(&self) -> String {
        let base = self.base_url.trim_end_matches('/');
        format!("{base}/chat/completions")
    }

    /// 拼出 `…/responses` URL。
    pub fn responses_url(&self) -> String {
        let base = self.base_url.trim_end_matches('/');
        format!("{base}/responses")
    }
}
