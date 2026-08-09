//! 上下文占用估算（只读，与 [`super::agent::Agent::run_turn`] 装配对齐）。

use lya_llm::{ApiMode, CAPABILITY_VISION, CAPABILITY_WEB_SEARCH};
use lya_prompt::RESPONSES_NATIVE_SEARCH;
use lya_prompt::{PromptBuilder, PromptInput};
use lya_session::SessionMeta;
use lya_token::{ContextUsageReport, UsageCategory, build_report, count_text, serialize_tool_schemas};

use crate::agent::Agent;
use crate::backend::ChatBackend;
use crate::context_breakdown::breakdown_path;
use crate::error::AgentError;

const DEFAULT_LIMIT: u64 = 1_048_576;

impl<B: ChatBackend> Agent<B> {
    /// 估算当前活跃分支将要发给模型的上下文占用。
    pub fn estimate_context_usage(&self, session_id: &str) -> Result<ContextUsageReport, AgentError> {
        let settings = self.settings();
        let meta = self
            .sessions()
            .get_session(session_id)?
            .ok_or_else(|| AgentError::Invalid(format!("会话不存在：{session_id}")))?;

        let assembly = self.assemble_turn_context(&meta, &settings.prompt)?;
        let limit = assembly.limit.unwrap_or(DEFAULT_LIMIT);
        let sections = &assembly.sections;
        let conv = &assembly.conversation;

        let system_prompt = [
            sections.core.as_str(),
            sections.actions.as_str(),
            sections.tools.as_str(),
            sections.extra.as_str(),
            sections.mode.as_str(),
            sections.memory.as_str(),
            serialize_tool_schemas(&assembly.schemas).as_str(),
        ]
        .join("\n\n");

        let tool_wire = join_lines(&[conv.tool_calls.as_str(), conv.tool_results.as_str()]);

        let mut categories = Vec::new();
        push_category(&mut categories, "system", "系统提示词", &system_prompt);
        push_category(&mut categories, "persona", "人设", &sections.persona);
        push_category(&mut categories, "system_messages", "系统消息", &conv.system);
        push_category(&mut categories, "assistant", "模型输出", &conv.assistant);
        push_category(&mut categories, "tool_calls", "工具调用", &tool_wire);
        push_category(&mut categories, "user", "用户输入", &conv.user);
        push_category(&mut categories, "provider", "Provider 原生", &conv.provider_items);

        Ok(build_report(limit, categories))
    }

    fn assemble_turn_context(
        &self,
        meta: &SessionMeta,
        prompt: &PromptBuilder,
    ) -> Result<TurnAssembly, AgentError> {
        let settings = self.settings();
        let enabled = meta
            .enabled_tools
            .clone()
            .or_else(|| settings.default_enabled_tools.clone());
        let enabled: Option<Vec<&str>> = enabled
            .as_ref()
            .map(|names| names.iter().map(String::as_str).collect());
        let api_mode = ApiMode::parse(&meta.api_mode).unwrap_or(ApiMode::Completions);
        let endpoint = self
            .endpoint_for(meta.model_id.as_deref(), &settings.default_model)
            .map_err(AgentError::Invalid)?;
        let native_web = api_mode == ApiMode::Responses
            && endpoint.supports(ApiMode::Responses, CAPABILITY_WEB_SEARCH);
        let tool_exclude: &[&str] = if native_web { &["web_search"] } else { &[] };
        let tool_bundle = self.tools().bundle(
            enabled.as_deref(),
            meta.work_mode.permission(),
            tool_exclude,
        );
        let action_bundle = self.actions().bundle(meta.work_mode);
        let memory_section = self.memory().index_section()?;
        let vision = endpoint.supports(api_mode, CAPABILITY_VISION);

        let mut input = PromptInput::new()
            .with_actions(action_bundle.prompt.clone())
            .with_tools(tool_bundle.prompt.clone())
            .with_mode(meta.work_mode.prompt_section().to_string())
            .with_memory(memory_section)
            .with_vision(vision);
        if native_web {
            input = input.with_extra(RESPONSES_NATIVE_SEARCH);
        }
        input.persona = meta.persona.clone();

        let sections = prompt.build_sections(&input);
        let path = self.sessions().path_to_active_leaf(&meta.id)?;
        let conversation = breakdown_path(&path);

        let mut schemas = tool_bundle.schemas.clone();
        schemas.extend(action_bundle.schemas.clone());

        Ok(TurnAssembly {
            sections,
            schemas,
            conversation,
            limit: Some(endpoint.effective_context_window()),
        })
    }
}

struct TurnAssembly {
    sections: lya_prompt::SystemSections,
    schemas: Vec<serde_json::Value>,
    conversation: crate::context_breakdown::ConversationBreakdown,
    limit: Option<u64>,
}

fn join_lines(parts: &[&str]) -> String {
    parts
        .iter()
        .map(|part| part.trim())
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn push_category(out: &mut Vec<UsageCategory>, id: &str, label: &str, text: &str) {
    let text = text.trim();
    if text.is_empty() {
        return;
    }
    let tokens = count_text(text);
    if tokens == 0 {
        return;
    }
    out.push(UsageCategory {
        id: id.into(),
        label: label.into(),
        tokens,
        in_context: true,
    });
}
