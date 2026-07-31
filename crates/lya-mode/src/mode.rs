//! 工作模式定义、权限映射、提示词与工具筛选。

use std::fmt;
use std::str::FromStr;

use lya_tool::{Permission, ToolBundle, ToolRegistry};
use serde::{Deserialize, Serialize};

use crate::ModeParseError;

/// lya 的工作模式。
///
/// 模式本身只表达行为边界；实际可见工具还要与 session 的启用工具列表取交集。
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    /// 问答模式：只允许 `-R-` 工具。
    #[default]
    Ask,
    /// 编辑模式：允许 `-R-W-` 工具，不允许任何带 `X` 的工具。
    Edit,
    /// Agent 模式：允许 `-R-W-X-` 工具。
    Agent,
}

impl Mode {
    /// 模式的稳定字符串标识，用于 session 存储、API 与前端。
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ask => "ask",
            Self::Edit => "edit",
            Self::Agent => "agent",
        }
    }

    /// 当前模式允许的最高工具权限。
    pub const fn permission(self) -> Permission {
        match self {
            Self::Ask => Permission::READ_ONLY,
            Self::Edit => Permission::READ_WRITE,
            Self::Agent => Permission::READ_WRITE_EXEC,
        }
    }

    /// 当前模式对应的 system prompt 段。
    ///
    /// 工具清单已由运行时按照权限过滤；本段用于告诉模型当前行为目标，
    /// 以及遇到越权请求时应该如何回复。
    pub const fn prompt_section(self) -> &'static str {
        match self {
            Self::Ask => ASK_PROMPT,
            Self::Edit => EDIT_PROMPT,
            Self::Agent => AGENT_PROMPT,
        }
    }

    /// 按当前模式筛选工具。
    ///
    /// - `names = None`：从注册中心全部工具中按权限筛选
    /// - `names = Some(...)`：先限制为 session 启用列表，再按权限筛选
    pub fn tools(self, registry: &ToolRegistry, names: Option<&[&str]>) -> ToolBundle {
        registry.bundle(names, self.permission())
    }

    /// 一次性解析当前模式所需的提示词与工具材料。
    ///
    /// 返回值不直接依赖 `lya-prompt`：上层应把 `mode_prompt` 放入
    /// `PromptInput::mode_section`，把 `tools.prompt` 放入
    /// `PromptInput::tool_section`，把 `tools.schemas` 交给 `lya-llm`。
    pub fn resolve(self, registry: &ToolRegistry, names: Option<&[&str]>) -> ModeBundle {
        ModeBundle {
            mode: self,
            permission: self.permission(),
            mode_prompt: self.prompt_section().to_string(),
            tools: self.tools(registry, names),
        }
    }
}

impl fmt::Display for Mode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Mode {
    type Err = ModeParseError;

    /// 解析 `ask` / `edit` / `agent`（忽略首尾空格与大小写）。
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "ask" => Ok(Self::Ask),
            "edit" => Ok(Self::Edit),
            "agent" => Ok(Self::Agent),
            _ => Err(ModeParseError {
                value: value.to_string(),
            }),
        }
    }
}

/// 一次模式解析产生的完整运行时材料。
#[derive(Debug, Clone, PartialEq)]
pub struct ModeBundle {
    /// 当前模式。
    pub mode: Mode,
    /// 当前模式允许的最高工具权限。
    pub permission: Permission,
    /// 交给 `lya-prompt` 的模式提示词段。
    pub mode_prompt: String,
    /// 经「session 启用列表 ∩ 模式权限」过滤后的工具材料。
    pub tools: ToolBundle,
}

/// Ask 模式提示词。
const ASK_PROMPT: &str = "\
=== [运行时] 当前工作模式：ask ===
你处于问答模式，只能查询、阅读和解释信息。
- 仅可使用只读（-R-）工具。
- 禁止创建、修改、删除文件或状态，也禁止执行命令。
- 如果用户要求写入或执行，说明当前模式的限制，并建议切换到 edit 或 agent 模式。
工具列表已经按当前模式过滤；不要调用未提供的工具。";

/// Edit 模式提示词。
const EDIT_PROMPT: &str = "\
=== [运行时] 当前工作模式：edit ===
你处于编辑模式，可以读取和修改文件或状态。
- 可使用读取与写入（-R-W-）工具。
- 禁止使用任何带执行权限（X）的工具，也不要通过其它工具绕过该限制。
- 完成修改后应检查结果；若任务必须执行命令，说明限制并建议切换到 agent 模式。
工具列表已经按当前模式过滤；不要调用未提供的工具。";

/// Agent 模式提示词。
const AGENT_PROMPT: &str = "\
=== [运行时] 当前工作模式：agent ===
你处于 agent 模式，可以使用读取、写入和执行（-R-W-X-）工具完成任务。
- 根据用户目标主动完成必要步骤，并在操作后检查结果。
- 对高风险或不可逆操作，先说明影响范围并取得必要确认。
工具列表已经按当前模式过滤；不要调用未提供的工具。";

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use lya_tool::traits::ToolCallFuture;
    use lya_tool::{Tool, ToolCtx, ToolMeta, ToolResult};
    use serde_json::{Value, json};

    use super::*;

    struct TestTool {
        meta: ToolMeta,
        schema: Value,
    }

    impl Tool for TestTool {
        fn meta(&self) -> &ToolMeta {
            &self.meta
        }

        fn parameters(&self) -> &Value {
            &self.schema
        }

        fn prompt_hint(&self) -> &str {
            "测试工具提示"
        }

        fn call(&self, _ctx: ToolCtx, _args: Value) -> ToolCallFuture<'_> {
            Box::pin(async { ToolResult::ok("ok") })
        }
    }

    fn tool(name: &str, permission: Permission) -> Arc<dyn Tool> {
        Arc::new(TestTool {
            meta: ToolMeta::new(name, name, "测试", permission),
            schema: json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }),
        })
    }

    fn registry() -> ToolRegistry {
        let mut registry = ToolRegistry::new();
        registry.register(tool("reader", Permission::READ)).unwrap();
        registry
            .register(tool("writer", Permission::READ_WRITE))
            .unwrap();
        registry
            .register(tool("runner", Permission::READ_WRITE_EXEC))
            .unwrap();
        registry
    }

    fn schema_names(bundle: &ToolBundle) -> Vec<&str> {
        bundle
            .schemas
            .iter()
            .filter_map(|schema| schema["function"]["name"].as_str())
            .collect()
    }

    #[test]
    fn permissions_are_exact() {
        assert_eq!(Mode::Ask.permission(), Permission::READ);
        assert_eq!(Mode::Edit.permission(), Permission::READ_WRITE);
        assert_eq!(Mode::Agent.permission(), Permission::READ_WRITE_EXEC);
    }

    #[test]
    fn filters_tools_by_mode() {
        let registry = registry();
        assert_eq!(
            schema_names(&Mode::Ask.resolve(&registry, None).tools),
            vec!["reader"]
        );
        assert_eq!(
            schema_names(&Mode::Edit.resolve(&registry, None).tools),
            vec!["reader", "writer"]
        );
        assert_eq!(
            schema_names(&Mode::Agent.resolve(&registry, None).tools),
            vec!["reader", "runner", "writer"]
        );
    }

    #[test]
    fn intersects_session_names_and_permission() {
        let registry = registry();
        let enabled = ["reader", "runner"];
        let edit = Mode::Edit.resolve(&registry, Some(&enabled));
        assert_eq!(schema_names(&edit.tools), vec!["reader"]);
    }

    #[test]
    fn parses_and_serializes_modes() {
        assert_eq!(" ASK ".parse::<Mode>().unwrap(), Mode::Ask);
        assert_eq!("edit".parse::<Mode>().unwrap(), Mode::Edit);
        assert_eq!(serde_json::to_string(&Mode::Agent).unwrap(), "\"agent\"");
        assert!("other".parse::<Mode>().is_err());
    }

    #[test]
    fn prompt_matches_mode() {
        assert!(Mode::Ask.prompt_section().contains("只读"));
        assert!(
            Mode::Edit
                .prompt_section()
                .contains("禁止使用任何带执行权限")
        );
        assert!(Mode::Agent.prompt_section().contains("-R-W-X-"));
    }
}
