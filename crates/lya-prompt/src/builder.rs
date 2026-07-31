//! [`PromptBuilder`]：按固定顺序拼接 system prompt。

use crate::identity::{
    format_persona_section, DEFAULT_PERSONA, SELF_AWARENESS, SYSTEM_AWARENESS, TIME_ANCHOR,
};
use crate::input::PromptInput;

/// 提示词组装器。
///
/// 持有**全局默认人设**；每次 [`PromptBuilder::build`] 用 [`PromptInput`]
/// 覆盖/追加外部段落。
#[derive(Debug, Clone)]
pub struct PromptBuilder {
    /// 全局人设正文（不含标题）。空字符串表示「默认用 [`DEFAULT_PERSONA`]」。
    ///
    /// 若调用 [`PromptBuilder::clear_persona`]，则全局与默认都不再注入人设，
    /// 除非 [`PromptInput::persona`] 显式提供非空内容。
    global_persona: Option<String>,
}

impl Default for PromptBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl PromptBuilder {
    /// 使用内置 [`DEFAULT_PERSONA`] 作为全局人设回退。
    pub fn new() -> Self {
        Self {
            global_persona: None, // None = 回退 DEFAULT_PERSONA
        }
    }

    /// 设置全局人设正文（覆盖默认）。
    pub fn with_persona(mut self, persona: impl Into<String>) -> Self {
        self.global_persona = Some(persona.into());
        self
    }

    /// 清除全局人设，且不再回退到 [`DEFAULT_PERSONA`]。
    ///
    /// 之后若 `input.persona` 也为空/`None`，最终 prompt **无人设段**。
    pub fn clear_persona(mut self) -> Self {
        self.global_persona = Some(String::new());
        self
    }

    /// 当前将用于回退的人设正文（未考虑 `input.persona` 覆盖）。
    pub fn global_persona_body(&self) -> &str {
        match &self.global_persona {
            None => DEFAULT_PERSONA,
            Some(s) => s.as_str(),
        }
    }

    /// 组装完整 system prompt。
    ///
    /// 顺序：系统认知 → 自我认知 → 时间锚点 → action → tools → mode → memory
    /// → extra → 人设。
    ///
    /// 记忆排在能力三段（action / tools / mode）之后：先讲「你能做什么」，
    /// 再讲「你已经知道什么」。
    ///
    /// 全程**不含任何随时间变化的内容**——整段必须是逐字节确定的，否则前缀
    /// 缓存每轮都会失效。当前时间通过消息前缀传达，见 [`TIME_ANCHOR`]。
    pub fn build(&self, input: &PromptInput) -> String {
        let mut parts: Vec<String> = Vec::new();

        parts.push(SYSTEM_AWARENESS.trim().to_string());
        parts.push(SELF_AWARENESS.trim().to_string());
        parts.push(TIME_ANCHOR.trim().to_string());

        push_optional(&mut parts, input.action_section.as_deref());
        push_optional(&mut parts, input.tool_section.as_deref());
        push_optional(&mut parts, input.mode_section.as_deref());
        push_optional(&mut parts, input.memory_section.as_deref());
        push_optional(&mut parts, input.extra_section.as_deref());

        let persona_body = resolve_persona_body(self, input);
        let persona_section = format_persona_section(persona_body);
        if !persona_section.is_empty() {
            parts.push(persona_section);
        }

        parts.join("\n\n")
    }
}

/// 解析本轮人设正文。
///
/// - `input.persona = Some(s)` → 用 `s`（可为空 = 本轮强制无人设）
/// - `input.persona = None` → 用 builder 全局；全局 `None` 则 [`DEFAULT_PERSONA`]；
///   全局 `Some("")`（clear_persona）则无人设
fn resolve_persona_body<'a>(builder: &'a PromptBuilder, input: &'a PromptInput) -> &'a str {
    match &input.persona {
        Some(s) => s.as_str(),
        None => builder.global_persona_body(),
    }
}

/// 非空才追加一段。
fn push_optional(parts: &mut Vec<String>, section: Option<&str>) {
    if let Some(s) = section {
        let t = s.trim();
        if !t.is_empty() {
            parts.push(t.to_string());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn order_and_defaults() {
        let text = PromptBuilder::new().build(&PromptInput::new());
        let sys = text.find("=== [系统]").expect("system");
        let self_pos = text.find("=== [自我认知]").expect("self");
        let persona = text.find("=== [人设]").expect("persona");
        assert!(sys < self_pos && self_pos < persona);
        assert!(text.contains("lya"));
        assert!(text.contains("小恋"));
        assert!(text.contains(DEFAULT_PERSONA.trim().lines().next().unwrap()));
    }

    #[test]
    fn injects_external_sections() {
        let text = PromptBuilder::new().build(
            &PromptInput::new()
                .with_actions("=== [元认知] ===\nform / memory")
                .with_tools("## Tools\n### file_read")
                .with_mode("=== [模式] ask ===\n只读")
                .with_memory("=== [记忆] Memory ===\n#1 环境操作偏好"),
        );
        let action = text.find("=== [元认知]").unwrap();
        let tools = text.find("## Tools").unwrap();
        let mode = text.find("=== [模式]").unwrap();
        let memory = text.find("=== [记忆]").unwrap();
        let persona = text.find("=== [人设]").unwrap();
        assert!(action < tools && tools < mode && mode < memory && memory < persona);
    }

    #[test]
    fn session_persona_overrides() {
        let text = PromptBuilder::new()
            .with_persona("全局冷淡")
            .build(&PromptInput::new().with_persona("会话活泼"));
        assert!(text.contains("会话活泼"));
        assert!(!text.contains("全局冷淡"));
    }

    #[test]
    fn clear_persona_omits_section() {
        let text = PromptBuilder::new().clear_persona().build(&PromptInput::new());
        assert!(!text.contains("=== [人设]"));
    }
}
