//! 动作注册中心。
//!
//! 与 [`lya_tool::ToolRegistry`] 并列：启动时注册全部动作，运行时按**模式**
//! 导出 [`ActionBundle`]。注意筛选条件只有模式，没有 RWX、也没有会话启用
//! 名单——动作是 lya 自己的元认知能力，不由用户逐个开关。

use std::collections::BTreeMap;
use std::sync::Arc;

use lya_mode::Mode;
use lya_tool::openai_function_schema;
use serde_json::Value;

use crate::error::ActionError;
use crate::meta::ActionOutcome;
use crate::traits::{Action, ActionCtx};

/// 提示词段落里对动作机制的总说明。
const ACTION_SECTION_HEADER: &str = "\
=== [动作] 元认知动作 ===

下面这些函数是你的**元认知动作**，调用方式与工具完全相同（function calling），\
但它们操作的是你自己的状态（记忆、与用户的交互、工作模式），而不是外部环境。

每个动作标注了执行后的流转方式：

- **[执行后: 继续]**：结果会回灌给你，你接着决策。
- **[执行后: 等待用户]**：会挂起当前处理，等用户答复后再带着答复继续。不要在同一条\
消息里既发起等待又安排后续动作。

参数填错时你会收到错误说明，按提示改正后重试即可。";

/// 一次筛选导出的结果：提示词段 + `tools` 数组元素。
#[derive(Debug, Clone, PartialEq)]
pub struct ActionBundle {
    /// 拼进 system prompt 的动作说明段；无可见动作时为空串。
    pub prompt: String,
    /// OpenAI 兼容的 `tools` 数组元素列表。
    ///
    /// 与工具的 schemas 合并后一起发出去，所以两边名字不能撞——合并由
    /// agent 负责，撞名应当在那里报错。
    pub schemas: Vec<Value>,
}

impl ActionBundle {
    /// 空结果。
    pub fn empty() -> Self {
        Self {
            prompt: String::new(),
            schemas: Vec::new(),
        }
    }

    /// 是否没有任何动作。
    pub fn is_empty(&self) -> bool {
        self.schemas.is_empty()
    }
}

/// 进程内动作目录：`name → Action`。
#[derive(Default)]
pub struct ActionRegistry {
    /// 按 name 排序存储，保证导出顺序稳定。
    actions: BTreeMap<String, Arc<dyn Action>>,
}

impl ActionRegistry {
    /// 空注册表。
    pub fn new() -> Self {
        Self::default()
    }

    /// 注册一个动作；`meta.name` 重复则报错。
    pub fn register(&mut self, action: Arc<dyn Action>) -> Result<(), ActionError> {
        let name = action.meta().name.clone();
        if self.actions.contains_key(&name) {
            return Err(ActionError::DuplicateName(name));
        }
        self.actions.insert(name, action);
        Ok(())
    }

    /// 已注册数量。
    pub fn len(&self) -> usize {
        self.actions.len()
    }

    /// 是否为空。
    pub fn is_empty(&self) -> bool {
        self.actions.is_empty()
    }

    /// 按内部名查找。
    pub fn get(&self, name: &str) -> Option<&Arc<dyn Action>> {
        self.actions.get(name)
    }

    /// 全部动作（按 name 排序）。
    pub fn list(&self) -> impl Iterator<Item = &Arc<dyn Action>> {
        self.actions.values()
    }

    /// 全部内部名（排序后）。
    pub fn names(&self) -> Vec<String> {
        self.actions.keys().cloned().collect()
    }

    /// 该模式下可见的动作名。
    pub fn visible_names(&self, mode: Mode) -> Vec<String> {
        self.actions
            .values()
            .filter(|action| action.visible_in(mode))
            .map(|action| action.meta().name.clone())
            .collect()
    }

    /// 按模式导出 [`ActionBundle`]。
    pub fn bundle(&self, mode: Mode) -> ActionBundle {
        let selected: Vec<&Arc<dyn Action>> = self
            .actions
            .values()
            .filter(|action| action.visible_in(mode))
            .collect();
        if selected.is_empty() {
            return ActionBundle::empty();
        }

        let mut prompt = String::from(ACTION_SECTION_HEADER);
        prompt.push_str("\n\n");
        let mut schemas = Vec::with_capacity(selected.len());

        for action in selected {
            let meta = action.meta();
            // 与工具段对齐：标出这是「执行后怎么走」，不要和工具的权限标注撞形状
            prompt.push_str(&format!(
                "### {} ({}) [执行后: {}]\n{}\n\n{}\n\n",
                meta.name,
                meta.raw_name,
                meta.flow.label(),
                meta.desc,
                action.prompt_hint().trim()
            ));
            // description 前缀标 [action]：模型在扁平的 tools[] 里也能一眼
            // 区分「改自己状态」和「碰外部环境」两类函数
            schemas.push(openai_function_schema(
                &meta.name,
                &format!("[action] {}", meta.desc),
                action.parameters(),
            ));
        }

        ActionBundle { prompt, schemas }
    }

    /// 只导出提示词段。
    pub fn prompt_section(&self, mode: Mode) -> String {
        self.bundle(mode).prompt
    }

    /// 只导出 OpenAI schemas。
    pub fn openai_actions(&self, mode: Mode) -> Vec<Value> {
        self.bundle(mode).schemas
    }

    /// 执行动作。
    ///
    /// 不校验该动作在当前模式下是否可见——筛选已经在导出 schema 时做过，
    /// 模型不该看到不可见的动作。若要防模型硬编造，由 agent 层再挡一次。
    pub async fn invoke(
        &self,
        name: &str,
        ctx: ActionCtx<'_>,
        args: Value,
    ) -> Result<ActionOutcome, ActionError> {
        let action = self
            .actions
            .get(name)
            .ok_or_else(|| ActionError::NotFound(name.to_string()))?;
        Ok(action.call(ctx, args).await)
    }
}
