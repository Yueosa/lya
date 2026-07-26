//! 工具元数据与调用结果。

use crate::permission::Permission;

/// 工具静态元信息。
///
/// 对应你约定的 meta 四元组：`name` / `raw_name` / `desc` / `prmt`。
/// 用法长文案不在这里，见 [`crate::Tool::prompt_hint`]。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolMeta {
    /// 内部名字：注册键、LLM `function.name`、存储与 API 都用它。
    pub name: String,
    /// 展示名：管理页 / 日志可读名称。
    pub raw_name: String,
    /// 短描述：映射到 OpenAI `function.description`。
    pub desc: String,
    /// 权限级别（R/W/X 组合）。
    pub prmt: Permission,
}

impl ToolMeta {
    /// 构造 meta。
    pub fn new(
        name: impl Into<String>,
        raw_name: impl Into<String>,
        desc: impl Into<String>,
        prmt: Permission,
    ) -> Self {
        Self {
            name: name.into(),
            raw_name: raw_name.into(),
            desc: desc.into(),
            prmt,
        }
    }
}

/// 单次工具调用的结果。
///
/// 无论成功失败，都建议把可读内容放进 [`ToolResult::content`]，
/// 便于原样写回 `role=tool` 消息。预检失败、钩子拒绝等也走这里，
/// 而不是一律 panic。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolResult {
    /// 是否视为成功（false 时上层可标 is_error）。
    pub success: bool,
    /// 回传给模型的文本（可以是纯文本或 JSON 字符串）。
    pub content: String,
}

impl ToolResult {
    /// 成功结果。
    pub fn ok(content: impl Into<String>) -> Self {
        Self {
            success: true,
            content: content.into(),
        }
    }

    /// 失败结果（仍带可读说明）。
    pub fn err(content: impl Into<String>) -> Self {
        Self {
            success: false,
            content: content.into(),
        }
    }

    /// 是否失败。
    pub fn is_error(&self) -> bool {
        !self.success
    }
}
