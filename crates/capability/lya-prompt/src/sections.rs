//! 按段落拆开的 system prompt，供上下文占用统计。

/// system prompt 各段（与 [`crate::PromptBuilder::build`] 顺序一致）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SystemSections {
    /// 环境：lya / 什亭之匣。
    pub environment: String,
    /// 身份：角色是谁（会话级）。
    pub identity: String,
    /// 运行：工具、动作、边界。
    pub operations: String,
    /// 表达修正：去大模型八股。
    pub voice: String,
    /// 时间锚点 + 聊天媒体。
    pub core: String,
    /// 动作说明。
    pub actions: String,
    /// 工具说明。
    pub tools: String,
    /// 能力补充。
    pub extra: String,
    /// 工作模式。
    pub mode: String,
    /// 长期记忆索引。
    pub memory: String,
    /// 口吻与 few-shot（会话级）。
    pub style: String,
}

impl SystemSections {
    /// 拼成完整 system prompt。
    pub fn join(&self) -> String {
        let mut parts = Vec::new();
        push_part(&mut parts, &self.environment);
        push_part(&mut parts, &self.identity);
        push_part(&mut parts, &self.operations);
        push_part(&mut parts, &self.voice);
        push_part(&mut parts, &self.core);
        push_part(&mut parts, &self.actions);
        push_part(&mut parts, &self.tools);
        push_part(&mut parts, &self.extra);
        push_part(&mut parts, &self.mode);
        push_part(&mut parts, &self.memory);
        push_part(&mut parts, &self.style);
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
            .with_identity("会话身份")
            .with_style("会话口吻");
        assert_eq!(builder.build(&input), builder.build_sections(&input).join());
    }
}
