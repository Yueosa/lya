//! 组装一次 system prompt 所需的外部注入与覆盖项。

/// [`crate::PromptBuilder::build`] 的输入。
///
/// 本 crate **只内置**系统认知 / 自我认知 / 人设；其余段落由调用方填入。
/// 留空的可选字段不会出现在最终 prompt 里。
#[derive(Debug, Clone, Default)]
pub struct PromptInput {
    /// 会话级人设；`Some` 时覆盖 builder 上的全局默认人设。
    ///
    /// `None`：用 builder 的全局人设（若也空则用 [`crate::DEFAULT_PERSONA`]）。
    /// 若希望本轮**完全不注入人设**，设 `persona: Some("")` 并把 builder
    /// 全局人设也清掉，或使用 [`crate::PromptBuilder::build`] 前
    /// [`crate::PromptBuilder::clear_persona`]。
    pub persona: Option<String>,

    /// 元认知 / Action 说明段（未来由 `lya-action` 生成）。
    ///
    /// 建议已含标题（如 `=== [元认知] Action ===`）；本 crate 原样拼接。
    pub action_section: Option<String>,

    /// 工具说明段（通常来自 `lya-tool` 的 `ToolBundle.prompt`）。
    pub tool_section: Option<String>,

    /// 工作模式说明段（未来由 `agent_mode` 生成；含 ask/edit/agent 等差异）。
    pub mode_section: Option<String>,

    /// 长期记忆的常驻索引（通常来自 `lya-memory::MemoryStore::index_section`）。
    ///
    /// 放的是标题 / 标签 / 摘要，不含正文；模型按编号取正文。
    pub memory_section: Option<String>,

    /// 紧跟工具段的能力补充说明（如 Responses 原生联网）。
    ///
    /// 放在工具段后面而不是最末尾：它讲的是「这类活儿该怎么干」，离工具列表
    /// 越远越容易被忽略。
    pub extra_section: Option<String>,

    /// 本会话模型能否读懂图片内容（`models.toml` 的 `vision` capability）。
    ///
    /// 影响聊天媒体段的措辞。默认 `false`——看不见是更安全的假设。
    pub vision: bool,
}

impl PromptInput {
    /// 空输入（仅内置三段 + 默认人设）。
    pub fn new() -> Self {
        Self::default()
    }

    /// 设置工具段。
    pub fn with_tools(mut self, section: impl Into<String>) -> Self {
        self.tool_section = Some(section.into());
        self
    }

    /// 设置 action 段。
    pub fn with_actions(mut self, section: impl Into<String>) -> Self {
        self.action_section = Some(section.into());
        self
    }

    /// 设置模式段。
    pub fn with_mode(mut self, section: impl Into<String>) -> Self {
        self.mode_section = Some(section.into());
        self
    }

    /// 设置记忆索引段。
    pub fn with_memory(mut self, section: impl Into<String>) -> Self {
        self.memory_section = Some(section.into());
        self
    }

    /// 设置紧跟工具段的能力补充说明（如 Responses 原生联网提示）。
    pub fn with_extra(mut self, section: impl Into<String>) -> Self {
        self.extra_section = Some(section.into());
        self
    }

    /// 声明本会话模型能读懂图片内容。
    pub fn with_vision(mut self, vision: bool) -> Self {
        self.vision = vision;
        self
    }

    /// 设置会话人设覆盖。
    pub fn with_persona(mut self, persona: impl Into<String>) -> Self {
        self.persona = Some(persona.into());
        self
    }
}
