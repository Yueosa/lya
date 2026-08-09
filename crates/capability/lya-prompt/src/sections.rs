//! 按段落拆开的 system prompt，供上下文占用统计。

/// system prompt 各段（与 [`crate::PromptBuilder::build`] 顺序一致）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SystemSections {
    /// 系统认知 + 自我认知 + 时间锚点 + 聊天媒体。
    pub core: String,
    /// 动作说明（元认知）。
    pub actions: String,
    /// 工具用法说明（非 JSON schema）。
    pub tools: String,
    /// 能力补充（如 Responses 原生联网）。
    pub extra: String,
    /// 工作模式。
    pub mode: String,
    /// 长期记忆索引。
    pub memory: String,
    /// 人设。
    pub persona: String,
}

impl SystemSections {
    /// 拼成完整 system prompt。
    pub fn join(&self) -> String {
        let mut parts = Vec::new();
        push_part(&mut parts, &self.core);
        push_part(&mut parts, &self.actions);
        push_part(&mut parts, &self.tools);
        push_part(&mut parts, &self.extra);
        push_part(&mut parts, &self.mode);
        push_part(&mut parts, &self.memory);
        push_part(&mut parts, &self.persona);
        parts.join("\n\n")
    }
}

pub(crate) fn trim_section(section: Option<&str>) -> String {
    section
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("")
        .to_string()
}

fn push_part(parts: &mut Vec<String>, section: &str) {
    let trimmed = section.trim();
    if !trimmed.is_empty() {
        parts.push(trimmed.to_string());
    }
}

#[cfg(test)]
mod tests {
    use crate::{PromptBuilder, PromptInput};

    #[test]
    fn join_matches_build() {
        let builder = PromptBuilder::new();
        let input = PromptInput::new()
            .with_actions("=== [动作] A ===")
            .with_tools("=== [工具] T ===")
            .with_persona("会话人设");
        assert_eq!(builder.build(&input), builder.build_sections(&input).join());
    }
}
