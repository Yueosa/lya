//! [`Agent`]：一轮对话的驱动器。

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use futures_core::Stream;
use futures_util::StreamExt;
use lya_action::{ActionCtx, ActionOutcome, ActionRegistry, FormAnswer, render_form_answer};
use lya_llm::{
    ApiMode, CAPABILITY_VISION, CAPABILITY_WEB_SEARCH, ChatEventStream, ChatStreamRequest,
    CompletionAssembler, LlmEndpoint, LlmError, StreamEvent, WebSearchStatus,
};
use lya_prompt::RESPONSES_NATIVE_SEARCH;
use lya_memory::MemoryStore;
use lya_base::{Live, Mode};
use lya_prompt::{PromptBuilder, PromptInput};
use lya_session::{
    ConfirmStepBlock, HitlBlock, MessageKind, MessagePayload, MessageRole, MessageStatus,
    OpenAiFunction, OpenAiMessage, OpenAiToolCall, SessionMeta, SessionStore,
};
use lya_tool::{ConfirmRequest, ToolCtx, ToolRegistry};
use serde_json::{Value, json};

use crate::backend::ChatBackend;
use crate::context::build_messages;
use crate::context_responses::build_responses_input;
use crate::error::AgentError;
use crate::event::{AgentEvent, BatchCallInfo, CallKind, CancelToken, ProviderSearchPhase, TurnEndReason};

/// 构造 [`Agent`] 所需的全部部件。
pub struct AgentParts<B: ChatBackend> {
    /// LLM 后端。
    pub backend: B,
    /// 可用的模型端点，按 `LlmEndpoint::id` 索引。
    ///
    /// 会话可以用 `model_id` 指定其中一个；没指定就用 `default_model`。
    pub endpoints: Vec<LlmEndpoint>,
    /// 默认模型 id，必须在 `endpoints` 里。
    pub default_model: String,
    /// 会话与消息树。
    pub sessions: Arc<SessionStore>,
    /// 长期记忆。
    pub memory: Arc<MemoryStore>,
    /// 工具目录。
    pub tools: Arc<ToolRegistry>,
    /// 动作目录。
    pub actions: Arc<ActionRegistry>,
    /// 提示词组装器（持有全局人设）。
    pub prompt: PromptBuilder,
    /// 单轮内 LLM 与工具最多来回几次。
    pub max_tool_rounds: u32,
    /// 同一条 assistant 消息里 tool_calls 数量上限。
    pub max_parallel_tools: u32,
    /// 连续多少次工具调用全失败就中止本轮；`0` 表示不启用。
    pub max_consecutive_tool_failures: u32,
    /// 会话没自定义工具列表时用的默认值。
    ///
    /// `None` 表示默认启用全部。
    pub default_enabled_tools: Option<Vec<String>>,
}

/// 默认模型必须真的指得到一个端点。
fn check_default_model(
    endpoints: &BTreeMap<String, LlmEndpoint>,
    default_model: &str,
) -> Result<(), AgentError> {
    if endpoints.contains_key(default_model) {
        return Ok(());
    }
    Err(AgentError::Invalid(format!(
        "默认模型 {:?} 不在端点列表里；现有：{:?}",
        default_model,
        endpoints.keys().collect::<Vec<_>>()
    )))
}

/// agent 里那些**来自配置、因此会在运行时被改**的值。
///
/// 单独成一族是为了能整体换：它们原先是 [`Agent`] 上的独立字段，装配时拷一份进来
/// 就再没人动过，于是用户在界面上改完 `runtime.toml` / `prompt.toml`，要重启才生效
/// （而界面读的是磁盘，显示的已经是新值，两边对不上更难查）。
///
/// 换的时候是**整族一起换**，[`Agent::run_turn`] 在轮次开头取一次快照用到底。
/// 所以一轮之内这些值绝不会变；改配置影响的是**下一轮**。
#[derive(Debug, Clone)]
pub struct TurnSettings {
    /// 默认模型 id，必须在端点列表里。
    pub default_model: String,
    /// 提示词组装器（持有全局人设）。
    pub prompt: PromptBuilder,
    /// 单轮内 LLM 与工具最多来回几次。
    pub max_tool_rounds: u32,
    /// 同一条 assistant 消息里 tool_calls 数量上限。
    pub max_parallel_tools: u32,
    /// 连续多少次工具调用全失败就中止本轮；`0` 表示不启用。
    pub max_consecutive_tool_failures: u32,
    /// 会话没自定义工具列表时用的默认值；`None` 表示启用全部。
    pub default_enabled_tools: Option<Vec<String>>,
}

/// 一轮对话的驱动器。
///
/// **自身无状态**：每次 [`Agent::run_turn`] 都从消息树读当前状态。HITL 挂起
/// 不在内存里留任何东西——表单发出去本轮就正常结束了，用户什么时候答复都行，
/// 进程重启也能接上。
///
/// 唯一的例外是 [`TurnSettings`]，它是配置的镜像而不是对话状态；见
/// [`Agent::apply_settings`]。
pub struct Agent<B: ChatBackend> {
    backend: B,
    endpoints: BTreeMap<String, LlmEndpoint>,
    sessions: Arc<SessionStore>,
    memory: Arc<MemoryStore>,
    tools: Arc<ToolRegistry>,
    actions: Arc<ActionRegistry>,
    settings: Live<TurnSettings>,
}

impl<B: ChatBackend> Agent<B> {
    /// 组装 agent。
    ///
    /// 工具与动作会合并进同一个 `tools[]`，所以这里检查一次重名——名字集合
    /// 是固定的（`visible_in` 只取子集），不必每轮都查。
    pub fn new(parts: AgentParts<B>) -> Result<Self, AgentError> {
        for name in parts.actions.names() {
            if parts.tools.get(&name).is_some() {
                return Err(AgentError::NameCollision(name));
            }
        }
        let endpoints: BTreeMap<String, LlmEndpoint> = parts
            .endpoints
            .into_iter()
            .map(|endpoint| (endpoint.id.clone(), endpoint))
            .collect();
        let settings = TurnSettings {
            default_model: parts.default_model,
            prompt: parts.prompt,
            max_tool_rounds: parts.max_tool_rounds,
            max_parallel_tools: parts.max_parallel_tools,
            max_consecutive_tool_failures: parts.max_consecutive_tool_failures,
            default_enabled_tools: parts.default_enabled_tools,
        };
        check_default_model(&endpoints, &settings.default_model)?;
        Ok(Self {
            backend: parts.backend,
            endpoints,
            sessions: parts.sessions,
            memory: parts.memory,
            tools: parts.tools,
            actions: parts.actions,
            settings: Live::new(settings),
        })
    }

    /// 工具目录，供界面展示「模型手里有什么」。
    pub fn tools(&self) -> &ToolRegistry {
        &self.tools
    }

    /// 动作目录。
    pub fn actions(&self) -> &ActionRegistry {
        &self.actions
    }

    /// 当前设置的快照。
    pub fn settings(&self) -> Arc<TurnSettings> {
        self.settings.get()
    }

    /// 换一份设置；配置文件改动后由装配处调用。
    ///
    /// `default_model` 不在端点列表里就整族拒绝、维持原样。端点来自 `models.toml`
    /// 而它没有写接口，所以进程活着的时候端点是固定的，能变的只有指哪一个——
    /// 让它指空会把 [`Agent::run_turn`] 变成必然失败，宁可在这里挡下。
    pub fn apply_settings(&self, next: TurnSettings) -> Result<(), AgentError> {
        check_default_model(&self.endpoints, &next.default_model)?;
        self.settings.set(next);
        Ok(())
    }

    /// 会话实际生效的工具名单。
    ///
    /// 会话没自定义（`None`）时**跟随全局默认**，而不是一律「全部启用」——
    /// 否则用户在配置里关掉某个工具，已有会话完全不受影响，很反直觉。
    /// 想让某个会话就是要全部，把它显式列全即可。
    pub fn effective_tools(&self, session: &SessionMeta) -> Option<Vec<String>> {
        session
            .enabled_tools
            .clone()
            .or_else(|| self.settings.get().default_enabled_tools.clone())
    }

    /// 取出会话该用的端点。
    ///
    /// 会话指名了一个已经不存在的模型时**报错而不是悄悄退回默认**——静默换成
    /// 另一个模型（可能更贵、能力也不同）比直接说清楚更让人困惑。
    pub(crate) fn endpoint_for(
        &self,
        model_id: Option<&str>,
        default_model: &str,
    ) -> Result<&LlmEndpoint, String> {
        let id = model_id.unwrap_or(default_model);
        self.endpoints.get(id).ok_or_else(|| {
            format!(
                "会话指定的模型 {id:?} 不在配置里；请重新选一个。现有：{:?}",
                self.endpoints.keys().collect::<Vec<_>>()
            )
        })
    }

    /// 会话仓储。
    pub fn sessions(&self) -> &SessionStore {
        &self.sessions
    }

    /// 记忆仓储。
    pub fn memory(&self) -> &MemoryStore {
        &self.memory
    }

    /// 跑一轮对话。
    ///
    /// **不接收用户输入**——用户消息由调用方先 append 进树。这样「发消息」
    /// 「重新生成」「编辑重发」「HITL 答复后继续」都是同一套：改树，再跑一轮。
    ///
    /// 返回的流最后一定恰好有一条 [`AgentEvent::TurnEnd`]。
    pub fn run_turn(
        &self,
        session_id: impl Into<String>,
        cancel: CancelToken,
    ) -> impl Stream<Item = AgentEvent> + '_ {
        let session_id = session_id.into();

        async_stream::stream! {
            /// 出错就收尾走人，省掉满地的 match。
            macro_rules! bail {
                ($e:expr) => {
                    match $e {
                        Ok(value) => value,
                        Err(err) => {
                            yield AgentEvent::TurnEnd {
                                reason: TurnEndReason::Failed(err.to_string()),
                            };
                            return;
                        }
                    }
                };
            }

            if let Some(pending) = bail!(self.sessions.pending_hitl(&session_id)) {
                yield AgentEvent::TurnEnd {
                    reason: TurnEndReason::Failed(format!(
                        "会话有未处理的确认（消息 #{pending}），请先答复"
                    )),
                };
                return;
            }

            // 整轮就认这一份：中途改配置不会让同一轮的前半段和后半段按两套上限跑，
            // 那种不一致比「改了要等下一轮」难查得多。
            let settings = self.settings.get();

            let mut round = 0u32;
            // 连续失败计数跨轮累计，任一次调用成功就清零。检查放在轮次开头：
            // 上一轮刚失败完就在这里收，不用再白跑一次 LLM。
            let mut consecutive_failures = 0u32;
            let mut last_failed_tool = String::new();
            loop {
                if cancel.is_cancelled() {
                    yield AgentEvent::TurnEnd { reason: TurnEndReason::Cancelled };
                    return;
                }
                if round >= settings.max_tool_rounds {
                    yield AgentEvent::TurnEnd { reason: TurnEndReason::MaxRounds };
                    return;
                }
                if settings.max_consecutive_tool_failures > 0
                    && consecutive_failures >= settings.max_consecutive_tool_failures
                {
                    yield AgentEvent::TurnEnd {
                        reason: TurnEndReason::ToolFailureLoop {
                            count: consecutive_failures,
                            last_tool: last_failed_tool.clone(),
                        },
                    };
                    return;
                }
                round += 1;
                yield AgentEvent::RoundStarted { round };

                // ── 每轮重新装配 ────────────────────────────────
                // 系统提示词每轮重建：模型可能刚写了一条记忆，重建才能让它
                // 立刻出现在常驻索引里。
                let meta = match bail!(self.sessions.get_session(&session_id)) {
                    Some(meta) => meta,
                    None => {
                        yield AgentEvent::TurnEnd {
                            reason: TurnEndReason::Failed(format!("会话不存在：{session_id}")),
                        };
                        return;
                    }
                };

                // 会话没自定义就跟随全局默认。这里读本轮快照而不是 effective_tools()，
                // 免得中途改配置让同一轮的两次装配拿到不同的工具集
                let enabled = meta
                    .enabled_tools
                    .clone()
                    .or_else(|| settings.default_enabled_tools.clone());
                let enabled: Option<Vec<&str>> = enabled
                    .as_ref()
                    .map(|names| names.iter().map(String::as_str).collect());
                let api_mode =
                    ApiMode::parse(&meta.api_mode).unwrap_or(ApiMode::Completions);
                let endpoint = match self
                    .endpoint_for(meta.model_id.as_deref(), &settings.default_model)
                {
                    Ok(endpoint) => endpoint,
                    Err(msg) => {
                        yield AgentEvent::TurnEnd {
                            reason: TurnEndReason::Failed(msg),
                        };
                        return;
                    }
                };
                let native_web = api_mode == ApiMode::Responses
                    && endpoint.supports(ApiMode::Responses, CAPABILITY_WEB_SEARCH);
                let tool_exclude: &[&str] = if native_web { &["web_search"] } else { &[] };
                // 「模式 → 权限上限 → 筛工具」原先包在 lya-mode 的 ModeBundle 里，
                // 而那一层为此把整个工具依赖垫到了 Mode 底下。展开就是这两行
                let tool_bundle = self.tools.bundle(
                    enabled.as_deref(),
                    meta.work_mode.permission(),
                    tool_exclude,
                );
                let action_bundle = self.actions.bundle(meta.work_mode);
                let memory_section = bail!(self.memory.index_section());

                // 模型自己判断不了「本会话支不支持看图」，由这里查 capabilities
                // 后在提示词里下断言
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
                input.identity = meta.identity.clone();
                input.style = meta.style.clone();
                let system = settings.prompt.build(&input);

                let path = bail!(self.sessions.path_to_active_leaf(&session_id));
                let request = match api_mode {
                    ApiMode::Completions => {
                        ChatStreamRequest::Completions(build_messages(&system, &path))
                    }
                    ApiMode::Responses => {
                        let (instructions, input) = build_responses_input(&system, &path);
                        ChatStreamRequest::Responses {
                            instructions,
                            input,
                            native_web_search: native_web,
                        }
                    }
                };

                let mut schemas = tool_bundle.schemas.clone();
                schemas.extend(action_bundle.schemas.clone());

                // ── 流式生成 ────────────────────────────────────
                // 先落一条占位消息，界面才有 id 可以挂增量
                let draft = bail!(self.sessions.append(
                    &session_id,
                    MessagePayload::assistant_text("", MessageStatus::Streaming),
                    false,
                ));
                yield AgentEvent::MessageCommitted { record: Box::new(draft.clone()) };

                let mut stream = match connect_chat_stream(
                    &self.backend,
                    &cancel,
                    api_mode,
                    endpoint,
                    request,
                    &schemas,
                )
                .await
                {
                    ChatConnect::Cancelled => {
                        let _ = self.sessions.delete_leaf(&session_id, draft.id);
                        yield AgentEvent::MessageDeleted { id: draft.id };
                        yield AgentEvent::TurnEnd { reason: TurnEndReason::Cancelled };
                        return;
                    }
                    ChatConnect::Failed(err) => {
                        let _ = self.sessions.delete_leaf(&session_id, draft.id);
                        yield AgentEvent::MessageDeleted { id: draft.id };
                        yield AgentEvent::TurnEnd {
                            reason: TurnEndReason::Failed(err.to_string()),
                        };
                        return;
                    }
                    ChatConnect::Ready(stream) => stream,
                };

                let mut assembler = CompletionAssembler::default();
                let mut cancelled = false;
                let mut failure: Option<String> = None;
                let mut produced = false;
                let mut responses_items: Vec<Value> = Vec::new();

                while let Some(item) = stream.next().await {
                    if cancel.is_cancelled() {
                        cancelled = true;
                        break;
                    }
                    match item {
                        Ok(event) => {
                            match &event {
                                StreamEvent::TextDelta(text) => {
                                    produced = true;
                                    yield AgentEvent::Delta(text.clone());
                                }
                                StreamEvent::ReasoningDelta(text) => {
                                    produced = true;
                                    yield AgentEvent::Reasoning(text.clone());
                                }
                                StreamEvent::ToolCallDelta(_) => produced = true,
                                StreamEvent::WebSearchStatus(status) => {
                                    produced = true;
                                    if let Some(event) = provider_search_event(status) {
                                        yield event;
                                    }
                                }
                                StreamEvent::WebSearchCallItem(item) => {
                                    produced = true;
                                    responses_items.push(item.clone());
                                }
                                StreamEvent::Finished { .. } => {}
                            }
                            assembler.apply(&event);
                        }
                        Err(err) => {
                            failure = Some(err.to_string());
                            break;
                        }
                    }
                }
                drop(stream);

                let completion = assembler.into_completion();

                if cancelled || failure.is_some() {
                    // 有内容就留下来标成中断，什么都没有就清掉
                    if produced {
                        let mut payload = assistant_payload(&completion, &responses_items);
                        payload.status = MessageStatus::Interrupted;
                        if let Ok(record) = self.sessions.update_payload(&session_id, draft.id, &payload) {
                            yield AgentEvent::MessageUpdated { record: Box::new(record) };
                        }
                    } else {
                        let _ = self.sessions.delete_leaf(&session_id, draft.id);
                        yield AgentEvent::MessageDeleted { id: draft.id };
                    }
                    yield AgentEvent::TurnEnd {
                        reason: match failure {
                            Some(msg) => TurnEndReason::Failed(msg),
                            None => TurnEndReason::Cancelled,
                        },
                    };
                    return;
                }

                if completion.content.trim().is_empty()
                    && completion.tool_calls.is_empty()
                    && responses_items.is_empty()
                {
                    let _ = self.sessions.delete_leaf(&session_id, draft.id);
                    yield AgentEvent::MessageDeleted { id: draft.id };
                    yield AgentEvent::TurnEnd { reason: TurnEndReason::EmptyResponse };
                    return;
                }

                let calls = &completion.tool_calls;
                let mut payload = assistant_payload(&completion, &responses_items);

                // 不带 tool_calls 就是本轮说完了
                if calls.is_empty() {
                    let finalized = bail!(self.sessions.update_payload(&session_id, draft.id, &payload));
                    yield AgentEvent::MessageUpdated { record: Box::new(finalized) };
                    yield AgentEvent::TurnEnd { reason: TurnEndReason::Completed };
                    return;
                }

                // ── 分发调用组 ────────────────────────────────────
                let mut plans = Vec::with_capacity(calls.len());
                for call in calls {
                    plans.push(
                        self.classify_call(
                            &session_id,
                            meta.work_mode,
                            &tool_bundle,
                            &call.name,
                            &call.arguments,
                        )
                        .await,
                    );
                }

                let batch_id = uuid::Uuid::new_v4().to_string();
                let call_ids: Vec<String> = calls.iter().map(|c| c.id.clone()).collect();
                let needs_review: Vec<String> = calls
                    .iter()
                    .zip(&plans)
                    .filter(|(_, plan)| matches!(plan, Planned::Hitl(_)))
                    .map(|(call, _)| call.id.clone())
                    .collect();
                let review_total = needs_review.len() as u32;
                payload.lya.meta = Some(json!({
                    "tool_batch": {
                        "id": batch_id,
                        "call_ids": call_ids,
                        "needs_review": needs_review,
                    }
                }));

                let finalized = bail!(self.sessions.update_payload(&session_id, draft.id, &payload));
                yield AgentEvent::MessageUpdated { record: Box::new(finalized.clone()) };

                yield AgentEvent::ToolBatchStarted {
                    batch_id: batch_id.clone(),
                    message_id: draft.id,
                    calls: calls
                        .iter()
                        .zip(&plans)
                        .map(|(call, plan)| BatchCallInfo {
                            call_id: call.id.clone(),
                            name: call.name.clone(),
                            needs_review: matches!(plan, Planned::Hitl(_)),
                        })
                        .collect(),
                };

                if calls.len() as u32 > settings.max_parallel_tools {
                    for call in calls {
                        let kind = call_kind(&call.name, &self.actions);
                        yield AgentEvent::CallStarted {
                            call_id: call.id.clone(),
                            name: call.name.clone(),
                            kind,
                        };
                        let msg = format!(
                            "本批 {} 个工具调用超过上限 max_parallel_tools={}，整组未执行。请拆成更小的批次。",
                            calls.len(),
                            settings.max_parallel_tools
                        );
                        let record = bail!(self.sessions.append(
                            &session_id,
                            MessagePayload::tool_result(&call.id, msg),
                            true,
                        ));
                        yield AgentEvent::MessageCommitted { record: Box::new(record) };
                        consecutive_failures += 1;
                        last_failed_tool = call.name.clone();
                        yield AgentEvent::CallFinished {
                            call_id: call.id.clone(),
                            name: call.name.clone(),
                            success: false,
                        };
                    }
                    continue;
                }

                // auto 项并行执行（纯工具；动作在 classify 阶段已执行完毕）
                let auto_items: Vec<(String, String, Value)> = calls
                    .iter()
                    .zip(&plans)
                    .filter_map(|(call, plan)| {
                        if let Planned::Auto { name, args } = plan {
                            Some((call.id.clone(), name.clone(), args.clone()))
                        } else {
                            None
                        }
                    })
                    .collect();
                let auto_results: HashMap<String, (String, bool)> = futures_util::future::join_all(
                    auto_items.iter().map(|(call_id, name, args)| async {
                        let result = self
                            .execute_auto(
                                meta.work_mode,
                                &tool_bundle,
                                name,
                                args.clone(),
                                ToolCtx::new(cancel.clone()),
                            )
                            .await;
                        (call_id.clone(), result)
                    }),
                )
                .await
                .into_iter()
                .collect();

                let mut review_index = 0u32;
                let mut awaiting = false;
                for (call, plan) in calls.iter().zip(&plans) {
                    let kind = call_kind(&call.name, &self.actions);
                    yield AgentEvent::CallStarted {
                        call_id: call.id.clone(),
                        name: call.name.clone(),
                        kind,
                    };

                    match plan {
                        Planned::Done { content, success } => {
                            let record = bail!(self.sessions.append(
                                &session_id,
                                MessagePayload::tool_result(&call.id, content.clone()),
                                true,
                            ));
                            yield AgentEvent::MessageCommitted { record: Box::new(record) };
                            if *success {
                                consecutive_failures = 0;
                            } else {
                                consecutive_failures += 1;
                                last_failed_tool = call.name.clone();
                            }
                            yield AgentEvent::CallFinished {
                                call_id: call.id.clone(),
                                name: call.name.clone(),
                                success: *success,
                            };
                        }
                        Planned::Auto { .. } => {
                            let (content, success) = auto_results
                                .get(&call.id)
                                .cloned()
                                .unwrap_or_else(|| ("内部错误：auto 结果缺失".into(), false));
                            let record = bail!(self.sessions.append(
                                &session_id,
                                MessagePayload::tool_result(&call.id, content),
                                true,
                            ));
                            yield AgentEvent::MessageCommitted { record: Box::new(record) };
                            if success {
                                consecutive_failures = 0;
                            } else {
                                consecutive_failures += 1;
                                last_failed_tool = call.name.clone();
                            }
                            yield AgentEvent::CallFinished {
                                call_id: call.id.clone(),
                                name: call.name.clone(),
                                success,
                            };
                        }
                        Planned::Hitl(block) => {
                            review_index += 1;
                            let payload = hitl_payload(
                                &call.id,
                                block.clone(),
                                Some(&batch_id),
                                review_index,
                                review_total,
                            );
                            let record = bail!(self.sessions.append(&session_id, payload, true));
                            yield AgentEvent::MessageCommitted { record: Box::new(record.clone()) };
                            yield AgentEvent::AwaitHuman {
                                message_id: record.id,
                                batch_id: Some(batch_id.clone()),
                                review_index: Some(review_index),
                                review_total: Some(review_total),
                            };
                            consecutive_failures = 0;
                            yield AgentEvent::CallFinished {
                                call_id: call.id.clone(),
                                name: call.name.clone(),
                                success: true,
                            };
                            awaiting = true;
                        }
                        Planned::Err(message) => {
                            let record = bail!(self.sessions.append(
                                &session_id,
                                MessagePayload::tool_result(&call.id, message.clone()),
                                true,
                            ));
                            yield AgentEvent::MessageCommitted { record: Box::new(record) };
                            consecutive_failures += 1;
                            last_failed_tool = call.name.clone();
                            yield AgentEvent::CallFinished {
                                call_id: call.id.clone(),
                                name: call.name.clone(),
                                success: false,
                            };
                        }
                    }
                }

                if awaiting {
                    yield AgentEvent::TurnEnd { reason: TurnEndReason::AwaitingHuman };
                    return;
                }
            }
        }
    }

    /// 分类一次调用：动作当场执行；工具只做静态分类。
    async fn classify_call(
        &self,
        session_id: &str,
        mode: Mode,
        allowed: &lya_tool::ToolBundle,
        name: &str,
        arguments: &str,
    ) -> Planned {
        let args: Value = if arguments.trim().is_empty() {
            json!({})
        } else {
            match serde_json::from_str(arguments) {
                Ok(value) => value,
                Err(err) => return Planned::Err(format!("参数不是合法 JSON：{err}")),
            }
        };

        if let Some(action) = self.actions.get(name) {
            if !action.visible_in(mode) {
                return Planned::Err(format!("动作 `{name}` 在当前 {mode} 模式下不可用。"));
            }
            return match action
                .call(ActionCtx::new(session_id, mode), args)
                .await
            {
                ActionOutcome::Continue(result) => Planned::Done {
                    content: result.content,
                    success: result.success,
                },
                ActionOutcome::AwaitHuman(block) => Planned::Hitl(*block),
            };
        }

        self.plan_tool(mode, allowed, name, args)
    }

    /// 纯工具路径的静态分类（不执行）。
    fn plan_tool(
        &self,
        mode: Mode,
        allowed: &lya_tool::ToolBundle,
        name: &str,
        args: Value,
    ) -> Planned {
        let Some(tool) = self.tools.get(name) else {
            return Planned::Err(format!("没有名为 `{name}` 的函数，请检查可用列表。"));
        };

        if let Some(reason) = deny_tool(tool.as_ref(), allowed, mode, name) {
            return Planned::Err(reason);
        }

        if let Some(request) = tool.confirm_request(&args) {
            return Planned::Hitl(confirm_block(name, &args, request));
        }

        Planned::Auto {
            name: name.to_string(),
            args,
        }
    }

    /// 执行已分类为 auto 的调用（动作或工具）。
    async fn execute_auto(
        &self,
        mode: Mode,
        allowed: &lya_tool::ToolBundle,
        name: &str,
        args: Value,
        ctx: ToolCtx,
    ) -> (String, bool) {
        if let Some(action) = self.actions.get(name) {
            if !action.visible_in(mode) {
                return (
                    format!("动作 `{name}` 在当前 {mode} 模式下不可用。"),
                    false,
                );
            }
            let outcome = action
                .call(ActionCtx::new("", mode), args)
                .await;
            return match outcome {
                ActionOutcome::Continue(result) => (result.content, result.success),
                ActionOutcome::AwaitHuman(_) => (
                    "内部错误：auto 路径不应再挂起 HITL".into(),
                    false,
                ),
            };
        }

        let Some(tool) = self.tools.get(name) else {
            return (
                format!("没有名为 `{name}` 的函数，请检查可用列表。"),
                false,
            );
        };
        if let Some(reason) = deny_tool(tool.as_ref(), allowed, mode, name) {
            return (reason, false);
        }
        let result = tool.call(ctx, args).await;
        (result.content, result.success)
    }

    /// 本批 HITL 全部结清后，按原始 call 顺序串行执行已批准且 deferred 的工具确认。
    pub async fn flush_deferred_tool_executions(
        &self,
        session_id: &str,
        cancel: CancelToken,
    ) -> Result<(), AgentError> {
        if self.sessions.pending_hitl(session_id)?.is_some() {
            return Ok(());
        }

        let mut deferred = self.collect_deferred_confirms(session_id)?;
        if deferred.is_empty() {
            return Ok(());
        }

        deferred.sort_by_key(|item| item.batch_index);

        for item in deferred {
            let content = self
                .execute_confirmed(
                    session_id,
                    &item.tool_name,
                    item.arguments.clone(),
                    item.note.as_deref(),
                    cancel.clone(),
                )
                .await?;
            self.sessions.append(
                session_id,
                MessagePayload::tool_result(&item.call_id, content),
                true,
            )?;
            self.mark_deferred_executed(session_id, item.hitl_id)?;
        }
        Ok(())
    }

    fn collect_deferred_confirms(
        &self,
        session_id: &str,
    ) -> Result<Vec<DeferredConfirm>, AgentError> {
        let path = self.sessions.path_to_active_leaf(session_id)?;
        let mut out = Vec::new();
        for msg in &path {
            if msg.payload.role != MessageRole::Hitl
                || msg.payload.status != MessageStatus::Resolved
            {
                continue;
            }
            let HitlBlock::ToolConfirm {
                tool_name,
                arguments,
                ..
            } = msg
                .payload
                .lya
                .hitl
                .as_ref()
                .ok_or_else(|| AgentError::Invalid(format!("消息 #{} 无 HITL", msg.id)))?
            else {
                continue;
            };
            let Some(meta) = msg.payload.lya.meta.as_ref() else {
                continue;
            };
            let Some(answer) = meta.get("answer") else {
                continue;
            };
            if answer.get("approved") != Some(&json!(true))
                || answer.get("deferred") != Some(&json!(true))
                || answer.get("executed") == Some(&json!(true))
            {
                continue;
            }
            let call_id = meta
                .get("tool_call_id")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    AgentError::Invalid(format!("消息 #{} 没有 tool_call_id", msg.id))
                })?
                .to_string();
            let note = answer
                .get("note")
                .and_then(Value::as_str)
                .map(str::to_string);
            let batch_index = meta
                .get("batch_index")
                .and_then(Value::as_u64)
                .unwrap_or(0) as u32;
            out.push(DeferredConfirm {
                hitl_id: msg.id,
                call_id,
                tool_name: tool_name.clone(),
                arguments: arguments.clone(),
                note,
                batch_index,
            });
        }
        Ok(out)
    }

    fn mark_deferred_executed(&self, session_id: &str, hitl_id: i64) -> Result<(), AgentError> {
        let mut record = self.sessions.get_message(session_id, hitl_id)?;
        let meta = record
            .payload
            .lya
            .meta
            .get_or_insert_with(|| json!({}));
        if let Some(obj) = meta.as_object_mut() {
            if let Some(answer) = obj.get_mut("answer").and_then(|v| v.as_object_mut()) {
                answer.insert("executed".into(), json!(true));
                answer.remove("deferred");
            }
        }
        self.sessions
            .update_payload(session_id, hitl_id, &record.payload)?;
        Ok(())
    }

    /// 答复一次工具确认。
    ///
    /// 批准时不立刻执行：记入 deferred 队列，等同批 HITL 全部审完后再并行跑。
    /// 拒绝则立刻写 tool 结果。
    pub async fn resolve_tool_confirm(
        &self,
        session_id: &str,
        approved: bool,
        note: Option<&str>,
        _cancel: CancelToken,
    ) -> Result<(), AgentError> {
        let (hitl_id, call_id, block) = self.pending_block(session_id)?;
        let HitlBlock::ToolConfirm { tool_name, .. } = &block else {
            return Err(AgentError::Invalid("当前待处理的不是工具确认".into()));
        };

        if approved {
            self.sessions.resolve_hitl(
                session_id,
                hitl_id,
                Some(json!({ "approved": true, "note": note, "deferred": true })),
            )?;
            return Ok(());
        }

        let mut text = format!("[用户拒绝] 用户没有放行 `{tool_name}`，该操作未执行。");
        if let Some(note) = note.filter(|n| !n.trim().is_empty()) {
            text.push_str(&format!("\n[用户备注: {}]", note.trim()));
        }
        self.sessions.append(
            session_id,
            MessagePayload::tool_result(&call_id, text),
            true,
        )?;
        self.sessions.resolve_hitl(
            session_id,
            hitl_id,
            Some(json!({ "approved": false, "note": note })),
        )?;
        Ok(())
    }

    /// 放行后执行，并把权限重新查一遍。
    ///
    /// 挂起期间用户可能改过模式或工具启用列表，不能拿当初的判断直接执行。
    async fn execute_confirmed(
        &self,
        session_id: &str,
        tool_name: &str,
        arguments: Value,
        note: Option<&str>,
        cancel: CancelToken,
    ) -> Result<String, AgentError> {
        let meta = self
            .sessions
            .get_session(session_id)?
            .ok_or_else(|| AgentError::Invalid(format!("会话不存在：{session_id}")))?;
        let enabled = self.effective_tools(&meta);
        let enabled: Option<Vec<&str>> = enabled
            .as_ref()
            .map(|names| names.iter().map(String::as_str).collect());
        let allowed = self
            .tools
            .bundle(enabled.as_deref(), meta.work_mode.permission(), &[]);

        let Some(tool) = self.tools.get(tool_name) else {
            return Ok(format!("工具 `{tool_name}` 已经不存在，操作未执行。"));
        };
        if let Some(reason) = deny_tool(tool.as_ref(), &allowed, meta.work_mode, tool_name) {
            return Ok(format!("放行后重新检查权限：{reason}"));
        }

        let result = tool.call(ToolCtx::new(cancel), arguments).await;
        Ok(match note.filter(|n| !n.trim().is_empty()) {
            Some(note) => format!("[用户备注: {}]\n{}", note.trim(), result.content),
            None => result.content,
        })
    }
}

/// 调用组里一条 call 的分类结果。
enum Planned {
    /// 动作已在 classify 阶段执行完毕。
    Done {
        content: String,
        success: bool,
    },
    /// 工具可立刻并行执行。
    Auto {
        name: String,
        args: Value,
    },
    /// 需用户介入，尚未执行。
    Hitl(HitlBlock),
    /// 参数或权限错误。
    Err(String),
}

struct DeferredConfirm {
    hitl_id: i64,
    call_id: String,
    tool_name: String,
    arguments: Value,
    note: Option<String>,
    batch_index: u32,
}

fn call_kind(name: &str, actions: &ActionRegistry) -> CallKind {
    if actions.get(name).is_some() {
        CallKind::Action
    } else {
        CallKind::Tool
    }
}

/// 执行前的权限复查；放行返回 `None`，拦下返回给模型看的原因。
fn deny_tool(
    tool: &dyn lya_tool::Tool,
    allowed: &lya_tool::ToolBundle,
    mode: Mode,
    name: &str,
) -> Option<String> {
    if allowed.allows(name) {
        return None;
    }
    let prmt = tool.meta().prmt;
    Some(if prmt.is_subset_of(mode.permission()) {
        format!("工具 `{name}` 没有在本会话启用，请让用户在工具管理里打开。")
    } else {
        format!(
            "工具 `{name}` 在当前 {mode} 模式下不可用（需要 {prmt}，本模式只授予 {}）。\
             请改用当前可用的工具，或用 request_mode_change 请用户切换模式。",
            mode.permission()
        )
    })
}

/// 把工具的确认请求映射成会话里的 HITL 块。
fn confirm_block(name: &str, args: &Value, request: ConfirmRequest) -> HitlBlock {
    HitlBlock::ToolConfirm {
        // 真正的 tool_call_id 由 hitl_payload 补进 meta，这里先占位
        tool_call_id: String::new(),
        tool_name: name.to_string(),
        arguments: args.clone(),
        summary: request.summary,
        steps: request
            .steps
            .into_iter()
            .map(|step| ConfirmStepBlock {
                raw: step.raw,
                explain: step.explain,
                risk: step.risk,
                connector: step.connector,
            })
            .collect(),
        reasons: request.reasons,
    }
}

impl<B: ChatBackend> Agent<B> {
    /// 用户手动切换工作模式，并在树上留一条系统消息说明这次变更。
    ///
    /// 模型自己发起的切换走 `request_mode_change` + [`Agent::resolve_mode_change`]，
    /// 那条路的 tool 结果已经交代了来龙去脉，不必再加标记。用户从界面切则毫无
    /// 痕迹——模型只会发现自己突然能干别的事了，所以这里补一条。
    ///
    /// 做成持久节点而不是一次性消息：树是唯一真相，追加也不影响前缀缓存。
    pub fn switch_mode(&self, session_id: &str, mode: Mode) -> Result<(), AgentError> {
        let meta = self
            .sessions
            .get_session(session_id)?
            .ok_or_else(|| AgentError::Invalid(format!("会话不存在：{session_id}")))?;
        if meta.work_mode == mode {
            return Ok(());
        }

        self.sessions.set_work_mode(session_id, mode)?;
        self.sessions.append(
            session_id,
            MessagePayload::system_text(format!(
                "[模式变更] 用户已将工作模式从 {} 切换为 {}",
                meta.work_mode, mode
            )),
            true,
        )?;
        Ok(())
    }

    /// 提交表单答复：把作答写成 tool 结果、结清 HITL。
    ///
    /// 之后再调一次 [`Agent::run_turn`] 就能接着跑。
    pub fn submit_form(&self, session_id: &str, answer: &FormAnswer) -> Result<(), AgentError> {
        let (hitl_id, call_id, block) = self.pending_block(session_id)?;
        let HitlBlock::Form {
            form_id,
            title,
            questions,
        } = block
        else {
            return Err(AgentError::Invalid("当前待处理的不是表单".into()));
        };
        if form_id != answer.form_id {
            return Err(AgentError::Invalid(format!(
                "表单 id 对不上：待处理的是 {form_id}，提交的是 {}",
                answer.form_id
            )));
        }

        let text = render_form_answer(&title, &questions, answer);
        self.sessions
            .append(session_id, MessagePayload::tool_result(call_id, text), true)?;
        // 原始作答一并留档：界面回看时才能把当时勾选的选项原样回显，
        // 而不是从渲染后的中文里反解
        self.sessions
            .resolve_hitl(session_id, hitl_id, serde_json::to_value(answer).ok())?;
        Ok(())
    }

    /// 答复模式切换请求；`approved` 为真时顺带把会话模式改掉。
    pub fn resolve_mode_change(&self, session_id: &str, approved: bool) -> Result<(), AgentError> {
        let (hitl_id, call_id, block) = self.pending_block(session_id)?;
        let HitlBlock::ModeChange { to_mode, .. } = block else {
            return Err(AgentError::Invalid("当前待处理的不是模式切换请求".into()));
        };

        let text = if approved {
            let mode: Mode = to_mode
                .parse()
                .map_err(|err: lya_base::ModeParseError| AgentError::Invalid(err.to_string()))?;
            self.sessions.set_work_mode(session_id, mode)?;
            format!("用户已同意，会话切换到 {to_mode} 模式。")
        } else {
            format!("用户拒绝切换到 {to_mode} 模式，请在现有权限内继续。")
        };

        self.sessions
            .append(session_id, MessagePayload::tool_result(call_id, text), true)?;
        self.sessions
            .resolve_hitl(session_id, hitl_id, Some(json!({ "approved": approved })))?;
        Ok(())
    }

    /// 取出当前待处理的 HITL：节点 id、对应的 tool_call_id、块内容。
    fn pending_block(&self, session_id: &str) -> Result<(i64, String, HitlBlock), AgentError> {
        let hitl_id = self
            .sessions
            .pending_hitl(session_id)?
            .ok_or_else(|| AgentError::Invalid("当前没有待处理的确认".into()))?;
        let record = self.sessions.get_message(session_id, hitl_id)?;
        let block = record
            .payload
            .lya
            .hitl
            .clone()
            .ok_or_else(|| AgentError::Invalid(format!("消息 #{hitl_id} 没有 HITL 内容")))?;
        let call_id = record
            .payload
            .lya
            .meta
            .as_ref()
            .and_then(|meta| meta.get("tool_call_id"))
            .and_then(Value::as_str)
            .ok_or_else(|| AgentError::Invalid(format!("消息 #{hitl_id} 没有记录 tool_call_id")))?
            .to_string();
        Ok((hitl_id, call_id, block))
    }
}

fn provider_search_event(status: &WebSearchStatus) -> Option<AgentEvent> {
    Some(match status {
        WebSearchStatus::InProgress { call_id } => AgentEvent::ProviderSearch {
            call_id: call_id.clone(),
            phase: ProviderSearchPhase::InProgress,
            query: None,
        },
        WebSearchStatus::Searching { call_id } => AgentEvent::ProviderSearch {
            call_id: call_id.clone(),
            phase: ProviderSearchPhase::Searching,
            query: None,
        },
        WebSearchStatus::Completed { call_id, query } => AgentEvent::ProviderSearch {
            call_id: call_id.clone(),
            phase: ProviderSearchPhase::Completed,
            query: query.clone(),
        },
        WebSearchStatus::Failed { call_id, message: _ } => AgentEvent::ProviderSearch {
            call_id: call_id.clone(),
            phase: ProviderSearchPhase::Failed,
            query: None,
        },
    })
}

/// 把一次生成结果落成助手消息。
fn assistant_payload(
    completion: &lya_llm::ChatCompletion,
    responses_items: &[Value],
) -> MessagePayload {
    let tool_calls: Vec<OpenAiToolCall> = completion
        .tool_calls
        .iter()
        .map(|call| OpenAiToolCall {
            id: call.id.clone(),
            kind: "function".into(),
            function: OpenAiFunction {
                name: call.name.clone(),
                arguments: call.arguments.clone(),
            },
        })
        .collect();

    let mut payload = MessagePayload {
        v: MessagePayload::VERSION,
        role: MessageRole::Assistant,
        kind: if tool_calls.is_empty() {
            MessageKind::Chat
        } else {
            MessageKind::ToolCall
        },
        status: MessageStatus::Complete,
        openai: Some(OpenAiMessage {
            role: "assistant".into(),
            content: completion.content.clone(),
            tool_calls: (!tool_calls.is_empty()).then_some(tool_calls),
            tool_call_id: None,
        }),
        lya: Default::default(),
    };
    if !completion.content.is_empty() {
        payload.lya.blocks = vec![json!({ "type": "text", "text": completion.content })];
    }
    if !completion.reasoning.is_empty() {
        payload.lya.reasoning = Some(completion.reasoning.clone());
    }
    if !responses_items.is_empty() {
        payload.lya.responses_items = responses_items.to_vec();
    }
    payload
}

/// 把 HITL 块落成待处理节点，并记下它对应哪次调用。
///
/// `tool_call_id` 必须存下来：用户答复时要用它写 tool 结果，否则那次调用
/// 永远配不上结果。
fn hitl_payload(
    call_id: &str,
    block: HitlBlock,
    batch_id: Option<&str>,
    batch_index: u32,
    batch_total: u32,
) -> MessagePayload {
    let kind = match &block {
        HitlBlock::Form { .. } => MessageKind::Form,
        HitlBlock::ToolConfirm { .. } => MessageKind::ToolConfirm,
        HitlBlock::ModeChange { .. } => MessageKind::ModeChange,
    };
    // 工具确认块自带一份 tool_call_id 供界面直接用，这里补齐
    let block = match block {
        HitlBlock::ToolConfirm {
            tool_name,
            arguments,
            summary,
            steps,
            reasons,
            ..
        } => HitlBlock::ToolConfirm {
            tool_call_id: call_id.to_string(),
            tool_name,
            arguments,
            summary,
            steps,
            reasons,
        },
        other => other,
    };
    let mut meta = json!({ "tool_call_id": call_id });
    if let Some(batch_id) = batch_id {
        if let Some(obj) = meta.as_object_mut() {
            obj.insert("batch_id".into(), json!(batch_id));
            obj.insert("batch_index".into(), json!(batch_index));
            obj.insert("batch_total".into(), json!(batch_total));
        }
    }
    let mut payload = MessagePayload::hitl_pending(kind, block);
    payload.lya.meta = Some(meta);
    payload
}

enum ChatConnect {
    Cancelled,
    Failed(LlmError),
    Ready(ChatEventStream),
}

async fn connect_chat_stream<B: ChatBackend>(
    backend: &B,
    cancel: &CancelToken,
    mode: ApiMode,
    endpoint: &LlmEndpoint,
    request: ChatStreamRequest,
    schemas: &[Value],
) -> ChatConnect {
    tokio::select! {
        _ = wait_until_cancelled(cancel) => ChatConnect::Cancelled,
        result = backend.chat_stream(mode, endpoint, request, schemas.to_vec()) => match result {
            Ok(stream) => ChatConnect::Ready(stream),
            Err(err) => ChatConnect::Failed(err),
        },
    }
}

async fn wait_until_cancelled(cancel: &CancelToken) {
    while !cancel.is_cancelled() {
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
}
