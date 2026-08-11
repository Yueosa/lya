//! 组装一次 system prompt 所需的外部注入与覆盖项。

/// [`crate::PromptBuilder::build`] 的输入。
#[derive(Debug, Clone, Default)]
pub struct PromptInput {
    /// 会话级身份；`Some("")` 强制不注入身份段。
    pub identity: Option<String>,
    /// 会话级口吻；`Some("")` 强制不注入口吻段。
    pub style: Option<String>,

    /// 元认知 / Action 说明段。
    pub action_section: Option<String>,
    /// 工具说明段。
    pub tool_section: Option<String>,
    /// 工作模式说明段。
    pub mode_section: Option<String>,
    /// 能力补充说明段。
    pub extra_section: Option<String>,
    /// 本会话模型能否读懂图片。
    pub vision: bool,
}

impl PromptInput {
    /// 空输入（用 builder 全局默认 + 内置 core）。
    pub fn new() -> Self {
        Self::default()
    }

    /// 注入工具说明段。
    pub fn with_tools(mut self, section: impl Into<String>) -> Self {
        self.tool_section = Some(section.into());
        self
    }

    /// 注入动作说明段。
    pub fn with_actions(mut self, section: impl Into<String>) -> Self {
        self.action_section = Some(section.into());
        self
    }

    /// 注入工作模式说明段。
    pub fn with_mode(mut self, section: impl Into<String>) -> Self {
        self.mode_section = Some(section.into());
        self
    }

    /// 注入能力补充说明段。
    pub fn with_extra(mut self, section: impl Into<String>) -> Self {
        self.extra_section = Some(section.into());
        self
    }

    /// 标记本会话模型是否支持视觉。
    pub fn with_vision(mut self, vision: bool) -> Self {
        self.vision = vision;
        self
    }

    /// 覆盖会话级身份正文。
    pub fn with_identity(mut self, identity: impl Into<String>) -> Self {
        self.identity = Some(identity.into());
        self
    }

    /// 覆盖会话级口吻正文。
    pub fn with_style(mut self, style: impl Into<String>) -> Self {
        self.style = Some(style.into());
        self
    }
}
