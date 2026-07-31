//! 模型清单（`models.toml`）。
//!
//! 这是**资源目录**，不是配置层级：它列出「有哪些模型可用」，而「当前用哪个」
//! 是 `runtime.toml` 的 `default_model` 或会话自己的选择。

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::error::ConfigError;

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
///
/// 固定字段会校验，**其余字段原样透传进请求体**——模型特有的参数
/// （`reasoning_effort`、`thinking` 之类）直接写在同一张表里就行，加新参数
/// 不需要改代码。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelEntry {
    /// 内部标识，被 `default_model` 与会话引用。
    pub id: String,
    /// 展示名。
    pub name: String,
    /// API 基地址。
    pub base_url: String,
    /// API 密钥。
    pub api_key: String,
    /// 透传字段，直接作为请求体的一部分（应包含 `model`）。
    #[serde(flatten)]
    pub params: Map<String, Value>,
}

impl ModelEntry {
    /// 结构性校验：固定字段都不能为空。
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
        Ok(())
    }

    /// `api_key` 是否还是模板里的占位符。
    ///
    /// 模板生成的是 `<在这里填入…>` 这种尖括号包裹的提示文本，用户没改就
    /// 直接发请求只会拿到一个莫名其妙的 401，不如启动时就说清楚。
    pub fn api_key_is_placeholder(&self) -> bool {
        let key = self.api_key.trim();
        key.starts_with('<') && key.ends_with('>')
    }
}
