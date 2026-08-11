//! [`PromptBuilder`]：按固定顺序拼接 system prompt。

use crate::identity::{
    format_environment, format_identity, format_operations, format_style, format_voice,
    DEFAULT_ENVIRONMENT, DEFAULT_IDENTITY, DEFAULT_OPERATIONS, DEFAULT_STYLE, DEFAULT_VOICE,
    TIME_ANCHOR,
};
use crate::input::PromptInput;
use crate::media::chat_media_section;
use crate::sections::{trim_section, SystemSections};

/// 提示词组装器：持有全局默认各段（来自 `prompt.toml`）。
#[derive(Debug, Clone)]
pub struct PromptBuilder {
    environment: Option<String>,
    operations: Option<String>,
    voice: Option<String>,
    identity_default: Option<String>,
    style_default: Option<String>,
}

impl Default for PromptBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl PromptBuilder {
    /// 使用内置默认各段。
    pub fn new() -> Self {
        Self {
            environment: None,
            operations: None,
            voice: None,
            identity_default: None,
            style_default: None,
        }
    }

    /// 从已解析的 `prompt.toml` 正文注入全局默认（空白段仍回退内置常量）。
    pub fn with_prompt_file(
        mut self,
        environment: Option<String>,
        operations: Option<String>,
        voice: Option<String>,
        identity: Option<String>,
        style: Option<String>,
    ) -> Self {
        self.environment = environment;
        self.operations = operations;
        self.voice = voice;
        self.identity_default = identity;
        self.style_default = style;
        self
    }

    /// 新会话默认身份正文。
    pub fn global_identity_body(&self) -> &str {
        resolve_body(self.identity_default.as_deref(), DEFAULT_IDENTITY)
    }

    /// 新会话默认口吻正文。
    pub fn global_style_body(&self) -> &str {
        resolve_body(self.style_default.as_deref(), DEFAULT_STYLE)
    }

    /// 组装完整 system prompt。
    pub fn build(&self, input: &PromptInput) -> String {
        self.build_sections(input).join()
    }

    /// 按段组装 system prompt。
    pub fn build_sections(&self, input: &PromptInput) -> SystemSections {
        let environment = format_environment(resolve_body(
            self.environment.as_deref(),
            DEFAULT_ENVIRONMENT,
        ));
        let identity = format_identity(resolve_session_body(
            input.identity.as_deref(),
            self.identity_default.as_deref(),
            DEFAULT_IDENTITY,
        ));
        let operations = format_operations(resolve_body(
            self.operations.as_deref(),
            DEFAULT_OPERATIONS,
        ));
        let voice = format_voice(resolve_body(self.voice.as_deref(), DEFAULT_VOICE));
        let core = [
            TIME_ANCHOR.trim(),
            chat_media_section(input.vision).trim(),
        ]
        .join("\n\n");

        let style = format_style(resolve_session_body(
            input.style.as_deref(),
            self.style_default.as_deref(),
            DEFAULT_STYLE,
        ));

        SystemSections {
            environment,
            identity,
            operations,
            voice,
            core,
            actions: trim_section(input.action_section.as_deref()),
            tools: trim_section(input.tool_section.as_deref()),
            extra: trim_section(input.extra_section.as_deref()),
            mode: trim_section(input.mode_section.as_deref()),
            style,
        }
    }
}

fn resolve_body<'a>(configured: Option<&'a str>, fallback: &'a str) -> &'a str {
    configured
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(fallback)
}

/// 会话 `Some("")` 表示强制空段；`None` 用全局默认。
fn resolve_session_body<'a>(
    session: Option<&'a str>,
    global: Option<&'a str>,
    fallback: &'a str,
) -> &'a str {
    match session {
        Some(s) => s,
        None => resolve_body(global, fallback),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn order_environment_before_identity() {
        let text = PromptBuilder::new().build(
            &PromptInput::new().with_style("=== [口吻] 测试 ==="),
        );
        let env = text.find("=== [环境]").expect("environment");
        let id = text.find("=== [身份]").expect("identity");
        let ops = text.find("=== [运行]").expect("operations");
        let voice = text.find("=== [表达修正]").expect("voice");
        let style = text.find("=== [口吻]").expect("style");
        assert!(env < id && id < ops && ops < voice && voice < style);
    }

    #[test]
    fn injects_external_sections() {
        let text = PromptBuilder::new().build(
            &PromptInput::new()
                .with_actions("=== [动作] 元认知动作 ===\nform / memory")
                .with_tools("=== [工具] 可用工具 ===\n### file_read")
                .with_extra("=== [联网] 原生搜索 ===\n内置搜索")
                .with_mode("=== [模式] ask ===\n只读")
                .with_style("=== [口吻] 测试 ==="),
        );
        let action = text.find("=== [动作]").unwrap();
        let tools = text.find("=== [工具]").unwrap();
        let network = text.find("=== [联网]").unwrap();
        let mode = text.find("=== [模式]").unwrap();
        let style = text.find("=== [口吻]").unwrap();
        assert!(action < tools && tools < network);
        assert!(network < mode && mode < style);
        assert!(!text.contains("=== [记忆]"), "记忆索引不进 system");
    }

    #[test]
    fn session_identity_overrides() {
        let text = PromptBuilder::new()
            .with_prompt_file(None, None, None, Some("全局身份".into()), None)
            .build(
                &PromptInput::new()
                    .with_identity("会话身份")
                    .with_style("会话口吻"),
            );
        assert!(text.contains("会话身份"));
        assert!(!text.contains("全局身份"));
        assert!(text.contains("会话口吻"));
    }

    #[test]
    fn empty_session_identity_omits_section() {
        let text = PromptBuilder::new().build(
            &PromptInput::new()
                .with_identity("")
                .with_style("仍有口吻"),
        );
        assert!(!text.contains("=== [身份]"));
        assert!(text.contains("仍有口吻"));
    }
}
