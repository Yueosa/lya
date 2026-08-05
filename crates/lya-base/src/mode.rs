//! 工作模式：`ask` / `edit` / `agent`。
//!
//! 只表达**行为边界**：模式 → 权限上限，以及告诉模型「你现在能做什么」的那段提示词。
//! 「按这个上限筛出哪些工具」是 `lya-tool` 的事，需要注册中心，不在这一层。
//!
//! 住在这里而不是一个 `lya-mode`：会话表要存它、配置文件要写它、HTTP 接口要收它，
//! 三处都在工具层之下。放在需要 `lya-tool` 的 crate 里，就等于让 `lya-config` 依赖
//! HTTP 客户端。

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::error::ModeParseError;
use crate::permission::Permission;

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

/// Ask 模式提示词。
const ASK_PROMPT: &str = "\
=== [模式] 当前工作模式：ask ===
你处于问答模式，只能查询、阅读和解释信息。
- 仅可使用只读（-R-）工具。
- 禁止创建、修改、删除文件或状态，也禁止执行命令。
- 如果用户要求写入或执行，说明当前模式的限制，并建议切换到 edit 或 agent 模式。
工具列表已经按当前模式过滤；不要调用未提供的工具。";

/// Edit 模式提示词。
const EDIT_PROMPT: &str = "\
=== [模式] 当前工作模式：edit ===
你处于编辑模式，可以读取和修改文件或状态。
- 可使用读取与写入（-R-W-）工具。
- 禁止使用任何带执行权限（X）的工具，也不要通过其它工具绕过该限制。
- 完成修改后应检查结果；若任务必须执行命令，说明限制并建议切换到 agent 模式。
工具列表已经按当前模式过滤；不要调用未提供的工具。";

/// Agent 模式提示词。
const AGENT_PROMPT: &str = "\
=== [模式] 当前工作模式：agent ===
你处于 agent 模式，可以使用读取、写入和执行（-R-W-X-）工具完成任务。
- 根据用户目标主动完成必要步骤，并在操作后检查结果。
- 对高风险或不可逆操作，先说明影响范围并取得必要确认。
工具列表已经按当前模式过滤；不要调用未提供的工具。";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permissions_are_exact() {
        assert_eq!(Mode::Ask.permission(), Permission::READ);
        assert_eq!(Mode::Edit.permission(), Permission::READ_WRITE);
        assert_eq!(Mode::Agent.permission(), Permission::READ_WRITE_EXEC);
    }

    #[test]
    fn parses_and_serializes_modes() {
        assert_eq!(" ASK ".parse::<Mode>().unwrap(), Mode::Ask);
        assert_eq!("edit".parse::<Mode>().unwrap(), Mode::Edit);
        assert_eq!(serde_json::to_string(&Mode::Agent).unwrap(), "\"agent\"");
        assert!("other".parse::<Mode>().is_err());
    }

    #[test]
    fn as_str_and_serde_agree() {
        // 库里存的是 as_str，配置是 serde 读的，两者错开会「存得进读不出」
        for mode in [Mode::Ask, Mode::Edit, Mode::Agent] {
            let json = serde_json::to_string(&mode).unwrap();
            assert_eq!(json, format!("\"{}\"", mode.as_str()));
            assert_eq!(mode.as_str().parse::<Mode>().unwrap(), mode);
        }
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
