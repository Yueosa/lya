//! `prompt.toml`：全局提示词各段。

use serde::{Deserialize, Serialize};

/// 一段可配置提示词正文。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PromptSection {
    /// 段落正文（含或不含 `=== […] ===` 标题均可；空则回退内置默认）。
    pub text: String,
}

/// `prompt.toml` 顶层结构。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PromptFile {
    /// 环境：lya / 什亭之匣、老师是谁。
    pub environment: PromptSection,
    /// 运行：工具、动作、HITL、失败策略。
    pub operations: PromptSection,
    /// 表达修正：去大模型八股（不禁 OS 口癖）。
    pub voice: PromptSection,
    /// 身份默认：新会话创建时抄进 `sessions.identity`。
    pub identity: PromptSection,
    /// 口吻默认：新会话创建时抄进 `sessions.style`。
    pub style: PromptSection,
}

impl PromptFile {
    /// 取某段正文；空白视为未配置。
    pub fn section_text(&self, key: PromptSectionKey) -> Option<&str> {
        let text = match key {
            PromptSectionKey::Environment => &self.environment.text,
            PromptSectionKey::Operations => &self.operations.text,
            PromptSectionKey::Voice => &self.voice.text,
            PromptSectionKey::Identity => &self.identity.text,
            PromptSectionKey::Style => &self.style.text,
        };
        let trimmed = text.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    }

    /// 按段写回（供 API 单段更新）。
    pub fn set_section(&mut self, key: PromptSectionKey, text: String) {
        match key {
            PromptSectionKey::Environment => self.environment.text = text,
            PromptSectionKey::Operations => self.operations.text = text,
            PromptSectionKey::Voice => self.voice.text = text,
            PromptSectionKey::Identity => self.identity.text = text,
            PromptSectionKey::Style => self.style.text = text,
        }
    }
}

/// 可编辑的提示词段键。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PromptSectionKey {
    /// 环境段。
    Environment,
    /// 运行段。
    Operations,
    /// 表达修正段。
    Voice,
    /// 身份默认段。
    Identity,
    /// 口吻默认段。
    Style,
}

impl PromptSectionKey {
    /// TOML 表名。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Environment => "environment",
            Self::Operations => "operations",
            Self::Voice => "voice",
            Self::Identity => "identity",
            Self::Style => "style",
        }
    }

    /// 解析 API / 路径参数。
    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "environment" => Some(Self::Environment),
            "operations" => Some(Self::Operations),
            "voice" => Some(Self::Voice),
            "identity" => Some(Self::Identity),
            "style" => Some(Self::Style),
            _ => None,
        }
    }
}
