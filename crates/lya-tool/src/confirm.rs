//! 执行前的用户确认请求。
//!
//! 这些类型刻意定义在本 crate 而不是复用 `lya-session` 的 HITL 块：这里的是**运行时
//! 请求**，那边的是**落库形状**，后者要跟着 schema 走而不该跟着工具签名走。由
//! `lya-agent` 负责把这里的「请求」映射成会话里的持久节点。
//!
//! 设计上把**判断**和**执行**分成两步：[`crate::Tool::confirm_request`] 是对参数
//! 的纯函数，只回答「这次调用要不要先问用户」；真正的副作用仍在
//! [`crate::Tool::call`] 里，且只在用户放行后才发生。

/// 一次调用需要用户确认的完整说明。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfirmRequest {
    /// 一句话概括这次调用整体要做什么。
    pub summary: String,
    /// 逐段拆解。
    ///
    /// 对 shell 来说就是把 `&&`、`|` 串起来的命令拆开——一长串命令原样丢给用户
    /// 等于让人闭眼签字。
    pub steps: Vec<ConfirmStep>,
    /// 为什么需要确认。
    pub reasons: Vec<String>,
}

impl ConfirmRequest {
    /// 是否存在任何一段带风险。
    pub fn has_risk(&self) -> bool {
        self.steps.iter().any(|step| step.risk.is_some())
    }
}

/// 拆解出的一步。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfirmStep {
    /// 原始片段，原样展示，不做任何美化。
    pub raw: String,
    /// 这一段在做什么，人话。
    pub explain: String,
    /// 这一段的风险；`None` 表示看起来没问题。
    pub risk: Option<String>,
    /// 与上一段的关系，如「成功后」「接上一步输出」；首段为空。
    pub connector: String,
}
