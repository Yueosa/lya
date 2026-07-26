//! 模型端点描述。
//!
//! 由配置层（如读 `model.toml`）填好后注入本 crate；`lya-llm` 不读文件。

use serde_json::{Map, Value};

/// 一次 LLM 调用所指向的端点。
///
/// - [`LlmEndpoint::base_url`]：形如 `https://api.deepseek.com/v1`
/// - [`LlmEndpoint::api_key`]：Bearer token
/// - [`LlmEndpoint::params`]：合并进请求 JSON 的额外字段
///   （至少应含 `model`；也可含 `temperature`、思考相关字段等）
#[derive(Debug, Clone)]
pub struct LlmEndpoint {
    /// 逻辑 id（来自配置，便于日志；请求本身不强制使用）。
    pub id: String,
    /// API 根路径（不含 `/chat/completions`）。
    pub base_url: String,
    /// Bearer API Key。
    pub api_key: String,
    /// 合并进 chat/completions 请求体的额外字段。
    pub params: Map<String, Value>,
}

impl LlmEndpoint {
    /// 用 base_url + api_key 构造；`id` 默认为 `"default"`，`params` 为空。
    pub fn new(base_url: impl Into<String>, api_key: impl Into<String>) -> Self {
        Self {
            id: "default".into(),
            base_url: base_url.into(),
            api_key: api_key.into(),
            params: Map::new(),
        }
    }

    /// 设置逻辑 id。
    pub fn with_id(mut self, id: impl Into<String>) -> Self {
        self.id = id.into();
        self
    }

    /// 写入 / 覆盖一个 param（如 `model`）。
    pub fn with_param(mut self, key: impl Into<String>, value: Value) -> Self {
        self.params.insert(key.into(), value);
        self
    }

    /// 批量合并 params。
    pub fn with_params(mut self, params: Map<String, Value>) -> Self {
        self.params.extend(params);
        self
    }

    /// 拼出 `…/chat/completions` URL。
    ///
    /// 会去掉 `base_url` 尾部 `/`，再追加路径。
    pub fn chat_completions_url(&self) -> String {
        let base = self.base_url.trim_end_matches('/');
        format!("{base}/chat/completions")
    }
}
