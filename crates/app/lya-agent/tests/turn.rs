//! 用脚本化的假后端验证一轮循环。
//!
//! 假后端按顺序吐出预先排好的回合，于是轮数上限、工具分发、HITL 挂起与恢复、
//! 取消这些逻辑都能在不联网的情况下跑到。

use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use futures_util::StreamExt;
use lya_action::{ActionRegistry, FormAnswer, FormAnswerItem, register_builtins};
use lya_agent::{Agent, AgentEvent, AgentParts, CancelToken, ChatBackend, TurnEndReason};
use lya_db::testing::open_test_db;
use lya_llm::{
    ApiMode, ChatEventStream, ChatMessage, ChatStreamRequest, LlmEndpoint, LlmError, StreamEvent,
    ToolCallDelta, WebSearchStatus,
};
use lya_memory::MemoryStore;
use lya_base::Mode;
use lya_prompt::PromptBuilder;
use lya_session::{CreateSession, HitlBlock, MessagePayload, MessageRole, SessionStore};
use lya_tool::{
    Permission, Tool, ToolCtx, ToolMeta, ToolRegistry, ToolResult, traits::ToolCallFuture,
};
use serde_json::{Value, json};
use tempfile::TempDir;

// ── 脚本化后端 ────────────────────────────────────────────────

/// 一个回合的输出。
#[derive(Clone)]
enum Turn {
    /// 纯文本回复。
    Text(String),
    /// 边说话边调用。
    Call {
        text: String,
        call_id: String,
        name: String,
        arguments: String,
    },
    /// 同一条 assistant 里多个 tool_calls。
    Calls {
        text: String,
        calls: Vec<(String, String, String)>,
    },
    /// 请求直接失败。
    Fail(String),
    /// 原生联网搜索后给正文（Responses 栈）。
    NativeSearch {
        call_id: String,
        query: String,
        text: String,
    },
}

#[derive(Default)]
struct ScriptedBackend {
    turns: Mutex<Vec<Turn>>,
    /// 每轮收到的 messages，供断言上下文装配。
    seen: Mutex<Vec<Vec<ChatMessage>>>,
    /// 每轮收到的 tools schema 名字。
    seen_tools: Mutex<Vec<Vec<String>>>,
    /// 每轮使用的 API 栈。
    seen_mode: Mutex<Vec<ApiMode>>,
    /// Responses 栈的 instructions。
    seen_instructions: Mutex<Vec<String>>,
    /// Responses 栈的 input items。
    seen_input: Mutex<Vec<Vec<Value>>>,
}

impl ScriptedBackend {
    fn new(turns: Vec<Turn>) -> Arc<Self> {
        Arc::new(Self {
            turns: Mutex::new(turns),
            ..Default::default()
        })
    }

    fn rounds(&self) -> usize {
        self.seen.lock().unwrap().len()
    }

    fn last_messages(&self) -> Vec<ChatMessage> {
        self.seen
            .lock()
            .unwrap()
            .last()
            .cloned()
            .unwrap_or_default()
    }

    fn last_tool_names(&self) -> Vec<String> {
        self.seen_tools
            .lock()
            .unwrap()
            .last()
            .cloned()
            .unwrap_or_default()
    }

    fn last_mode(&self) -> Option<ApiMode> {
        self.seen_mode.lock().unwrap().last().copied()
    }

    fn last_instructions(&self) -> Option<String> {
        self.seen_instructions.lock().unwrap().last().cloned()
    }

    fn last_input(&self) -> Option<Vec<Value>> {
        self.seen_input.lock().unwrap().last().cloned()
    }
}

impl ChatBackend for ScriptedBackend {
    fn chat_stream<'a>(
        &'a self,
        mode: ApiMode,
        _endpoint: &'a LlmEndpoint,
        request: ChatStreamRequest,
        tools: Vec<Value>,
    ) -> Pin<Box<dyn Future<Output = Result<ChatEventStream, LlmError>> + Send + 'a>> {
        self.seen_mode.lock().unwrap().push(mode);
        match request {
            ChatStreamRequest::Completions(messages) => {
                self.seen.lock().unwrap().push(messages);
            }
            ChatStreamRequest::Responses { instructions, input, native_web_search: _ } => {
                self.seen_instructions.lock().unwrap().push(instructions);
                self.seen_input.lock().unwrap().push(input);
                self.seen.lock().unwrap().push(Vec::new());
            }
        }
        self.seen_tools.lock().unwrap().push(
            tools
                .iter()
                .filter_map(|t| t["function"]["name"].as_str().map(str::to_string))
                .collect(),
        );

        let turn = {
            let mut turns = self.turns.lock().unwrap();
            if turns.is_empty() {
                Turn::Text("（脚本用完了）".into())
            } else {
                turns.remove(0)
            }
        };

        Box::pin(async move {
            let events: Vec<StreamEvent> = match turn {
                Turn::Fail(msg) => return Err(LlmError::Other(msg)),
                Turn::Text(text) => vec![
                    StreamEvent::TextDelta(text),
                    StreamEvent::Finished {
                        reason: Some("stop".into()),
                    },
                ],
                Turn::Call {
                    text,
                    call_id,
                    name,
                    arguments,
                } => vec![
                    StreamEvent::TextDelta(text),
                    StreamEvent::ToolCallDelta(ToolCallDelta {
                        index: 0,
                        id: Some(call_id),
                        name: Some(name),
                        arguments: Some(arguments),
                    }),
                    StreamEvent::Finished {
                        reason: Some("tool_calls".into()),
                    },
                ],
                Turn::Calls { text, calls } => {
                    let mut events = vec![StreamEvent::TextDelta(text)];
                    for (index, (call_id, name, arguments)) in calls.into_iter().enumerate() {
                        events.push(StreamEvent::ToolCallDelta(ToolCallDelta {
                            index,
                            id: Some(call_id),
                            name: Some(name),
                            arguments: Some(arguments),
                        }));
                    }
                    events.push(StreamEvent::Finished {
                        reason: Some("tool_calls".into()),
                    });
                    events
                }
                Turn::NativeSearch {
                    call_id,
                    query,
                    text,
                } => vec![
                    StreamEvent::WebSearchStatus(WebSearchStatus::Searching {
                        call_id: call_id.clone(),
                    }),
                    StreamEvent::WebSearchCallItem(json!({
                        "type": "web_search_call",
                        "id": call_id,
                        "status": "completed",
                        "action": { "type": "search", "queries": [query] }
                    })),
                    StreamEvent::TextDelta(text),
                    StreamEvent::Finished {
                        reason: Some("stop".into()),
                    },
                ],
            };
            let stream = async_stream::stream! {
                for event in events {
                    yield Ok(event);
                }
            };
            Ok(Box::pin(stream) as ChatEventStream)
        })
    }
}

// ── 一个只读假工具 ────────────────────────────────────────────

struct EchoTool {
    meta: ToolMeta,
    params: Value,
}

impl EchoTool {
    fn new() -> Self {
        Self {
            meta: ToolMeta::new("echo", "回声", "原样返回 text", Permission::READ),
            params: json!({
                "type": "object",
                "properties": { "text": { "type": "string" } },
                "required": ["text"]
            }),
        }
    }
}

struct WebSearchTool {
    meta: ToolMeta,
    params: Value,
}

impl WebSearchTool {
    fn new() -> Self {
        Self {
            meta: ToolMeta::new(
                "web_search",
                "搜索",
                "DuckDuckGo 搜索",
                Permission::READ,
            ),
            params: json!({
                "type": "object",
                "properties": { "query": { "type": "string" } },
                "required": ["query"]
            }),
        }
    }
}

impl Tool for WebSearchTool {
    fn meta(&self) -> &ToolMeta {
        &self.meta
    }
    fn parameters(&self) -> &Value {
        &self.params
    }
    fn prompt_hint(&self) -> &str {
        "测试用"
    }
    fn call(&self, _ctx: ToolCtx, _args: Value) -> ToolCallFuture<'_> {
        Box::pin(async { ToolResult::ok("[]") })
    }
}

struct WebFetchTool {
    meta: ToolMeta,
    params: Value,
}

impl WebFetchTool {
    fn new() -> Self {
        Self {
            meta: ToolMeta::new(
                "web_fetch",
                "抓取",
                "读网页正文",
                Permission::READ,
            ),
            params: json!({
                "type": "object",
                "properties": { "url": { "type": "string" } },
                "required": ["url"]
            }),
        }
    }
}

impl Tool for WebFetchTool {
    fn meta(&self) -> &ToolMeta {
        &self.meta
    }
    fn parameters(&self) -> &Value {
        &self.params
    }
    fn prompt_hint(&self) -> &str {
        "测试用"
    }
    fn call(&self, _ctx: ToolCtx, args: Value) -> ToolCallFuture<'_> {
        Box::pin(async move {
            let url = args.get("url").and_then(Value::as_str).unwrap_or("");
            ToolResult::ok(format!("正文: {url}"))
        })
    }
}

impl Tool for EchoTool {
    fn meta(&self) -> &ToolMeta {
        &self.meta
    }
    fn parameters(&self) -> &Value {
        &self.params
    }
    fn prompt_hint(&self) -> &str {
        "测试用"
    }
    fn call(&self, _ctx: ToolCtx, args: Value) -> ToolCallFuture<'_> {
        Box::pin(async move {
            match args.get("text").and_then(Value::as_str) {
                Some(text) => ToolResult::ok(format!("echo: {text}")),
                None => ToolResult::err("缺少 text"),
            }
        })
    }
}

// ── 装配 ──────────────────────────────────────────────────────

struct Fixture {
    _dir: TempDir,
    agent: Agent<Arc<ScriptedBackend>>,
    backend: Arc<ScriptedBackend>,
    sessions: Arc<SessionStore>,
    session_id: String,
}

fn fixture_with(turns: Vec<Turn>, mode: Mode, max_rounds: u32) -> Fixture {
    fixture_with_session(turns, mode, max_rounds, None)
}

fn fixture_with_session(
    turns: Vec<Turn>,
    mode: Mode,
    max_rounds: u32,
    api_mode: Option<&str>,
) -> Fixture {
    fixture_full(turns, mode, max_rounds, api_mode, 0)
}

fn fixture_full(
    turns: Vec<Turn>,
    mode: Mode,
    max_rounds: u32,
    api_mode: Option<&str>,
    max_consecutive_tool_failures: u32,
) -> Fixture {
    let (dir, db) = open_test_db();

    let sessions = Arc::new(SessionStore::with_db(Arc::clone(&db)));
    let memory = Arc::new(MemoryStore::with_db(db));

    let mut tools = ToolRegistry::new();
    tools.register(Arc::new(EchoTool::new())).unwrap();

    let mut actions = ActionRegistry::new();
    register_builtins(&mut actions, Arc::clone(&memory)).unwrap();

    let backend = ScriptedBackend::new(turns);
    let agent = Agent::new(AgentParts {
        backend: Arc::clone(&backend),
        endpoints: vec![LlmEndpoint::new("https://example.invalid/v1", "k")],
        default_model: "default".into(),
        sessions: Arc::clone(&sessions),
        memory,
        tools: Arc::new(tools),
        actions: Arc::new(actions),
        prompt: PromptBuilder::new(),
        max_tool_rounds: max_rounds,
        max_parallel_tools: 3,
        max_consecutive_tool_failures,
        default_enabled_tools: None,
    })
    .unwrap();

    let session_id = sessions
        .create_session(CreateSession {
            work_mode: mode,
            api_mode: api_mode.map(str::to_string),
            ..Default::default()
        })
        .unwrap()
        .id;

    Fixture {
        _dir: dir,
        agent,
        backend,
        sessions,
        session_id,
    }
}

fn fixture(turns: Vec<Turn>) -> Fixture {
    fixture_with(turns, Mode::Agent, 8)
}

impl Fixture {
    fn say(&self, text: &str) {
        self.sessions
            .append(&self.session_id, MessagePayload::user_text(text), false)
            .unwrap();
    }

    async fn run(&self) -> Vec<AgentEvent> {
        self.run_with(CancelToken::new()).await
    }

    async fn run_with(&self, cancel: CancelToken) -> Vec<AgentEvent> {
        let stream = self.agent.run_turn(self.session_id.clone(), cancel);
        futures_util::pin_mut!(stream);
        let mut events = Vec::new();
        while let Some(event) = stream.next().await {
            events.push(event);
        }
        events
    }
}

fn end_reason(events: &[AgentEvent]) -> TurnEndReason {
    let AgentEvent::TurnEnd { reason } = events.last().expect("至少有一条事件") else {
        panic!("最后一条必须是 TurnEnd，实际是 {:?}", events.last());
    };
    reason.clone()
}

fn text_of(events: &[AgentEvent]) -> String {
    events
        .iter()
        .filter_map(|e| match e {
            AgentEvent::Delta(text) => Some(text.as_str()),
            _ => None,
        })
        .collect()
}

// ── 测试 ──────────────────────────────────────────────────────

#[tokio::test]
async fn plain_reply_ends_the_turn() {
    let fx = fixture(vec![Turn::Text("你好喵~".into())]);
    fx.say("你好");
    let events = fx.run().await;

    assert_eq!(end_reason(&events), TurnEndReason::Completed);
    assert_eq!(text_of(&events), "你好喵~");
    assert_eq!(fx.backend.rounds(), 1, "不带 tool_calls 就该收尾");

    let path = fx.sessions.path_to_active_leaf(&fx.session_id).unwrap();
    assert_eq!(path.len(), 2);
    assert_eq!(path[1].payload.openai.as_ref().unwrap().content, "你好喵~");
}

#[tokio::test]
async fn tool_call_feeds_back_and_continues() {
    let fx = fixture(vec![
        Turn::Call {
            text: "我看看喵~".into(),
            call_id: "c1".into(),
            name: "echo".into(),
            arguments: r#"{"text":"hi"}"#.into(),
        },
        Turn::Text("结果是 hi 喵~".into()),
    ]);
    fx.say("echo 一下");
    let events = fx.run().await;

    assert_eq!(end_reason(&events), TurnEndReason::Completed);
    assert_eq!(fx.backend.rounds(), 2, "有 tool_calls 就该继续下一轮");
    // 边说边干：第一轮的正文也抛出来了
    assert!(text_of(&events).contains("我看看喵~"));

    // 第二轮的上下文里要有配对好的 tool 结果
    let last = fx.backend.last_messages();
    let tool_msg = last
        .iter()
        .find(|m| m.tool_call_id.as_deref() == Some("c1"))
        .expect("工具结果要回灌");
    assert!(tool_msg.content.ends_with("echo: hi"));
    assert!(tool_msg.content.starts_with('['), "tool 消息带时间戳前缀");

    let calls: Vec<_> = events
        .iter()
        .filter(|e| matches!(e, AgentEvent::CallFinished { .. }))
        .collect();
    assert_eq!(calls.len(), 1);
}

#[tokio::test]
async fn unknown_function_is_reported_to_the_model() {
    let fx = fixture(vec![
        Turn::Call {
            text: String::new(),
            call_id: "c1".into(),
            name: "nonexistent".into(),
            arguments: "{}".into(),
        },
        Turn::Text("好吧".into()),
    ]);
    fx.say("乱调一个");
    let events = fx.run().await;

    assert_eq!(end_reason(&events), TurnEndReason::Completed);
    assert!(matches!(
        events
            .iter()
            .find(|e| matches!(e, AgentEvent::CallFinished { .. })),
        Some(AgentEvent::CallFinished { success: false, .. })
    ));
    let last = fx.backend.last_messages();
    let tool_msg = last
        .iter()
        .find(|m| m.tool_call_id.as_deref() == Some("c1"))
        .unwrap();
    assert!(tool_msg.content.contains("nonexistent"));
}

#[tokio::test]
async fn malformed_arguments_are_reported_not_fatal() {
    let fx = fixture(vec![
        Turn::Call {
            text: String::new(),
            call_id: "c1".into(),
            name: "echo".into(),
            arguments: "{不是 json".into(),
        },
        Turn::Text("改好了".into()),
    ]);
    fx.say("go");
    let events = fx.run().await;

    assert_eq!(end_reason(&events), TurnEndReason::Completed);
    let last = fx.backend.last_messages();
    let tool_msg = last
        .iter()
        .find(|m| m.tool_call_id.as_deref() == Some("c1"))
        .unwrap();
    assert!(tool_msg.content.contains("JSON"));
}

#[tokio::test]
async fn round_limit_stops_the_loop() {
    let turns = (0..10)
        .map(|i| Turn::Call {
            text: String::new(),
            call_id: format!("c{i}"),
            name: "echo".into(),
            arguments: r#"{"text":"x"}"#.into(),
        })
        .collect();
    let fx = fixture_with(turns, Mode::Agent, 3);
    fx.say("停不下来");
    let events = fx.run().await;

    assert_eq!(end_reason(&events), TurnEndReason::MaxRounds);
    assert_eq!(fx.backend.rounds(), 3);
}

/// 参数常年传不对时不该一路烧到 max_tool_rounds 才停。
#[tokio::test]
async fn consecutive_tool_failures_stop_the_loop() {
    let turns = (0..10)
        .map(|i| Turn::Call {
            text: String::new(),
            call_id: format!("c{i}"),
            name: "echo".into(),
            // 参数不是合法 JSON，每轮都会失败
            arguments: "".into(),
        })
        .collect();
    let fx = fixture_full(turns, Mode::Agent, 32, None, 3);
    fx.say("一直传错参数");
    let events = fx.run().await;

    match end_reason(&events) {
        TurnEndReason::ToolFailureLoop { count, last_tool } => {
            assert_eq!(count, 3);
            assert_eq!(last_tool, "echo");
        }
        other => panic!("应当熔断，实际 {other:?}"),
    }
    assert_eq!(fx.backend.rounds(), 3, "熔断后不该再多打一次 LLM");
}

/// 中间成功一次就清零，别把「偶尔出错」误判成打转。
#[tokio::test]
async fn a_success_resets_the_failure_streak() {
    let bad = |i: usize| Turn::Call {
        text: String::new(),
        call_id: format!("bad{i}"),
        name: "echo".into(),
        arguments: "".into(),
    };
    let good = Turn::Call {
        text: String::new(),
        call_id: "ok".into(),
        name: "echo".into(),
        arguments: r#"{"text":"x"}"#.into(),
    };
    let turns = vec![bad(0), bad(1), good, bad(2), bad(3), Turn::Text("好了".into())];
    let fx = fixture_full(turns, Mode::Agent, 32, None, 3);
    fx.say("偶尔出错");
    let events = fx.run().await;

    assert_eq!(end_reason(&events), TurnEndReason::Completed);
}

#[tokio::test]
async fn empty_response_leaves_no_stray_message() {
    let fx = fixture(vec![Turn::Text(String::new())]);
    fx.say("在吗");
    let events = fx.run().await;

    assert_eq!(end_reason(&events), TurnEndReason::EmptyResponse);
    let path = fx.sessions.path_to_active_leaf(&fx.session_id).unwrap();
    assert_eq!(path.len(), 1, "空回复不该在树上留下空壳");

    // 占位消息的 MessageCommitted 已经发出去了，删掉它必须也说一声，
    // 否则订阅者那边会留一个永远抹不掉的幽灵
    let committed = committed_ids(&events);
    let deleted = deleted_ids(&events);
    assert_eq!(committed, deleted, "落库过又被删掉的消息要成对出现");
}

/// 事件流里落库过的消息 id。
fn committed_ids(events: &[AgentEvent]) -> Vec<i64> {
    events
        .iter()
        .filter_map(|event| match event {
            AgentEvent::MessageCommitted { record } => Some(record.id),
            _ => None,
        })
        .collect()
}

/// 事件流里被删掉的消息 id。
fn deleted_ids(events: &[AgentEvent]) -> Vec<i64> {
    events
        .iter()
        .filter_map(|event| match event {
            AgentEvent::MessageDeleted { id } => Some(*id),
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn request_failure_removes_the_placeholder() {
    let fx = fixture(vec![Turn::Fail("401 unauthorized".into())]);
    fx.say("你好");
    let events = fx.run().await;

    assert!(matches!(end_reason(&events), TurnEndReason::Failed(msg) if msg.contains("401")));
    assert_eq!(
        committed_ids(&events),
        deleted_ids(&events),
        "请求失败清掉占位消息时也要通知订阅者"
    );
    let path = fx.sessions.path_to_active_leaf(&fx.session_id).unwrap();
    assert_eq!(path.len(), 1);
}

#[tokio::test]
async fn cancel_before_start_ends_immediately() {
    let fx = fixture(vec![Turn::Text("不该被调用".into())]);
    fx.say("你好");
    let cancel = CancelToken::new();
    cancel.cancel();
    let events = fx.run_with(cancel).await;

    assert_eq!(end_reason(&events), TurnEndReason::Cancelled);
    assert_eq!(fx.backend.rounds(), 0);
}

#[tokio::test]
async fn tools_are_filtered_by_mode_and_actions_ride_along() {
    // ask 模式：echo 是 -R- 能用；request_mode_change 在非 agent 模式可见
    let fx = fixture_with(vec![Turn::Text("好".into())], Mode::Ask, 8);
    fx.say("你好");
    fx.run().await;

    let names = fx.backend.last_tool_names();
    assert!(names.contains(&"echo".to_string()));
    assert!(names.contains(&"memory_write".to_string()));
    assert!(names.contains(&"request_mode_change".to_string()));

    // agent 模式下这个动作自己隐藏
    let fx = fixture_with(vec![Turn::Text("好".into())], Mode::Agent, 8);
    fx.say("你好");
    fx.run().await;
    assert!(
        !fx.backend
            .last_tool_names()
            .contains(&"request_mode_change".to_string())
    );
}

#[tokio::test]
async fn system_prompt_is_byte_stable_across_rounds() {
    // 提示词每轮重建，但内容必须逐字节一致，否则 API 商的前缀缓存每轮都会
    // 全量 miss。任何往提示词里塞时间戳、随机序、哈希序的改动都该在这里炸。
    let fx = fixture(vec![
        Turn::Call {
            text: String::new(),
            call_id: "c1".into(),
            name: "echo".into(),
            arguments: r#"{"text":"x"}"#.into(),
        },
        Turn::Text("好了".into()),
    ]);
    fx.say("跑一下");
    fx.run().await;

    let seen = fx.backend.seen.lock().unwrap();
    assert_eq!(seen.len(), 2);
    assert_eq!(
        seen[0][0].content, seen[1][0].content,
        "两轮的系统提示词必须完全一致"
    );
}

#[tokio::test]
async fn default_identity_change_does_not_touch_existing_sessions() {
    /*
      身份与口吻是会话级的，「默认提示词」只是新会话的起点。

      改一次全局默认不该把每一段正在进行的对话都换掉性格——而上面几十条聊天记录
      还是旧性格写的，模型下一轮得同时扮演两个人，答出来的东西两头都不像。
    */
    let fx = fixture(vec![Turn::Text("一".into()), Turn::Text("二".into())]);
    fx.sessions
        .set_identity(&fx.session_id, Some("我是阿罗娜，说话轻快。"))
        .unwrap();

    fx.say("第一句");
    fx.run().await;

    let mut next = (*fx.agent.settings()).clone();
    next.prompt = PromptBuilder::new().with_prompt_file(
        None,
        None,
        None,
        Some("我是普拉娜，说话冷静。".into()),
        None,
    );
    fx.agent.apply_settings(next).unwrap();

    fx.say("第二句");
    fx.run().await;

    let seen = fx.backend.seen.lock().unwrap();
    assert_eq!(seen.len(), 2);
    for (i, system) in seen.iter().enumerate() {
        assert!(
            system[0].content.contains("我是阿罗娜"),
            "第 {} 轮该还是这个会话自己那份身份",
            i + 1
        );
        assert!(
            !system[0].content.contains("普拉娜"),
            "第 {} 轮混进了改后的默认身份",
            i + 1
        );
    }
}

#[tokio::test]
async fn default_tools_change_takes_effect_on_the_next_turn() {
    // 工具开关同理：会话没自定义时跟随全局默认，那个默认也得是活的
    let fx = fixture(vec![Turn::Text("一".into()), Turn::Text("二".into())]);

    fx.say("第一句");
    fx.run().await;
    assert!(fx.backend.last_tool_names().contains(&"echo".to_string()));

    let mut next = (*fx.agent.settings()).clone();
    next.default_enabled_tools = Some(vec![]);
    fx.agent.apply_settings(next).unwrap();

    fx.say("第二句");
    fx.run().await;
    assert!(
        !fx.backend.last_tool_names().contains(&"echo".to_string()),
        "全局关掉之后，没自定义过的会话下一轮就不该再看见它"
    );
}

#[tokio::test]
async fn settings_are_frozen_for_the_duration_of_a_turn() {
    // 轮次开头取一次快照用到底。若有人把 settings.get() 挪进循环里，这一轮就会
    // 在中途换上限、当场以 MaxRounds 收场——前半段按一套规则、后半段按另一套，
    // 那种不一致比「改了要等下一轮」难查得多。
    let fx = fixture(vec![
        Turn::Call {
            text: String::new(),
            call_id: "c1".into(),
            name: "echo".into(),
            arguments: r#"{"text":"x"}"#.into(),
        },
        Turn::Text("好了".into()),
    ]);
    fx.say("跑一下");

    // run_turn 和 apply_settings 都只借 &self，所以能在消费事件的同一个循环里改
    let stream = fx.agent.run_turn(fx.session_id.clone(), CancelToken::new());
    futures_util::pin_mut!(stream);
    let mut events = Vec::new();
    while let Some(event) = stream.next().await {
        if matches!(event, AgentEvent::RoundStarted { round: 1 }) {
            let mut next = (*fx.agent.settings()).clone();
            next.max_tool_rounds = 1;
            fx.agent.apply_settings(next).unwrap();
        }
        events.push(event);
    }

    assert_eq!(fx.backend.rounds(), 2, "本轮该按开跑时的上限跑完");
    assert!(
        matches!(
            events.last(),
            Some(AgentEvent::TurnEnd { reason: TurnEndReason::Completed })
        ),
        "不该在中途被新上限掐断：{:?}",
        events.last()
    );
}

#[test]
fn apply_settings_rejects_a_default_model_that_points_nowhere() {
    // 端点来自 models.toml，进程活着的时候是固定的；让 default_model 指空
    // 会把之后每一轮都变成必然失败，宁可在换的时候就挡下来
    let fx = fixture(vec![Turn::Text("好".into())]);

    let mut next = (*fx.agent.settings()).clone();
    next.default_model = "不存在的模型".into();
    let err = fx.agent.apply_settings(next).unwrap_err();
    assert!(matches!(err, lya_agent::AgentError::Invalid(_)), "{err:?}");

    // 拒绝之后必须维持原样，不能留下半生效的状态
    assert_eq!(fx.agent.settings().default_model, "default");
}

#[tokio::test]
async fn memory_written_this_turn_shows_up_next_round() {
    let fx = fixture(vec![
        Turn::Call {
            text: String::new(),
            call_id: "c1".into(),
            name: "memory_write".into(),
            arguments: r#"{"title":"用户偏好","body":"喜欢简短回答","summary":"简短"}"#.into(),
        },
        Turn::Text("记住了喵~".into()),
    ]);
    fx.say("记住我喜欢简短回答");
    let events = fx.run().await;

    assert_eq!(end_reason(&events), TurnEndReason::Completed);
    // 系统提示词每轮重建，所以刚写的记忆立刻出现在常驻索引里
    let system = fx.backend.last_messages()[0].content.clone();
    assert!(system.contains("用户偏好"), "刚写的记忆应出现在索引中");
    assert!(!fx.backend.seen.lock().unwrap()[0].is_empty());
}

#[tokio::test]
async fn form_suspends_the_turn_and_answer_resumes_it() {
    let fx = fixture(vec![
        Turn::Call {
            text: "我需要确认一下".into(),
            call_id: "c1".into(),
            name: "form".into(),
            arguments: r#"{
                "form_id": "deploy",
                "title": "部署方式",
                "questions": [
                    { "id": "svc", "text": "用哪种？", "kind": "single",
                      "options": [{"key":"systemd","label":"systemd --user"}] }
                ]
            }"#
            .into(),
        },
        Turn::Text("好的，用 systemd 喵~".into()),
    ]);
    fx.say("帮我部署");

    let events = fx.run().await;
    assert_eq!(end_reason(&events), TurnEndReason::AwaitingHuman);
    let Some(AgentEvent::AwaitHuman { message_id, .. }) = events
        .iter()
        .find(|e| matches!(e, AgentEvent::AwaitHuman { .. }))
        .cloned()
    else {
        panic!("应当抛出 AwaitHuman");
    };
    assert_eq!(
        fx.sessions.pending_hitl(&fx.session_id).unwrap(),
        Some(message_id)
    );

    // 挂起期间不许再跑
    let blocked = fx.run().await;
    assert!(matches!(end_reason(&blocked), TurnEndReason::Failed(msg) if msg.contains("未处理")));

    // 答复 → 结清 → 接着跑
    fx.agent
        .submit_form(
            &fx.session_id,
            &FormAnswer {
                form_id: "deploy".into(),
                items: vec![FormAnswerItem {
                    question_id: "svc".into(),
                    values: vec!["systemd".into()],
                    note: None,
                }],
                freetext: None,
            },
        )
        .unwrap();
    assert_eq!(fx.sessions.pending_hitl(&fx.session_id).unwrap(), None);

    let events = fx.run().await;
    assert_eq!(end_reason(&events), TurnEndReason::Completed);

    // 模型看到的是配对好的 tool 结果，HITL 节点不进上下文
    let last = fx.backend.last_messages();
    let tool_msg = last
        .iter()
        .find(|m| m.tool_call_id.as_deref() == Some("c1"))
        .expect("表单答复要作为 tool 结果回灌");
    assert!(
        tool_msg.content.contains("systemd --user"),
        "显示的是 label"
    );
    assert!(tool_msg.content.contains("[表单回答: 部署方式]"));

    let hitl_count = fx
        .sessions
        .path_to_active_leaf(&fx.session_id)
        .unwrap()
        .iter()
        .filter(|r| r.payload.role == MessageRole::Hitl)
        .count();
    assert_eq!(hitl_count, 1, "HITL 节点留在树上供界面回看");
}

#[tokio::test]
async fn approved_mode_change_switches_the_session() {
    let fx = fixture_with(
        vec![
            Turn::Call {
                text: String::new(),
                call_id: "c1".into(),
                name: "request_mode_change".into(),
                arguments: r#"{"to_mode":"agent","reason":"要执行命令"}"#.into(),
            },
            Turn::Text("这就来".into()),
        ],
        Mode::Ask,
        8,
    );
    fx.say("帮我跑个命令");

    let events = fx.run().await;
    assert_eq!(end_reason(&events), TurnEndReason::AwaitingHuman);

    fx.agent.resolve_mode_change(&fx.session_id, true).unwrap();
    assert_eq!(
        fx.sessions
            .get_session(&fx.session_id)
            .unwrap()
            .unwrap()
            .work_mode,
        Mode::Agent
    );

    let events = fx.run().await;
    assert_eq!(end_reason(&events), TurnEndReason::Completed);
    let last = fx.backend.last_messages();
    let tool_msg = last
        .iter()
        .find(|m| m.tool_call_id.as_deref() == Some("c1"))
        .unwrap();
    assert!(tool_msg.content.contains("已同意"));
}

#[tokio::test]
async fn rejected_mode_change_keeps_the_mode() {
    let fx = fixture_with(
        vec![
            Turn::Call {
                text: String::new(),
                call_id: "c1".into(),
                name: "request_mode_change".into(),
                arguments: r#"{"to_mode":"agent","reason":"想执行命令"}"#.into(),
            },
            Turn::Text("那我说说思路".into()),
        ],
        Mode::Ask,
        8,
    );
    fx.say("跑个命令");
    fx.run().await;

    fx.agent.resolve_mode_change(&fx.session_id, false).unwrap();
    assert_eq!(
        fx.sessions
            .get_session(&fx.session_id)
            .unwrap()
            .unwrap()
            .work_mode,
        Mode::Ask
    );

    fx.run().await;
    let last = fx.backend.last_messages();
    let tool_msg = last
        .iter()
        .find(|m| m.tool_call_id.as_deref() == Some("c1"))
        .unwrap();
    assert!(tool_msg.content.contains("拒绝"));
}

#[tokio::test]
async fn out_of_mode_tool_is_blocked_at_execution() {
    // 造一个 -R-W- 的工具，在 ask 模式下不会出现在列表里；模型硬编名字调用，
    // 执行前那道关必须拦住，否则筛选形同虚设
    struct Writer(ToolMeta, Value);
    impl Tool for Writer {
        fn meta(&self) -> &ToolMeta {
            &self.0
        }
        fn parameters(&self) -> &Value {
            &self.1
        }
        fn prompt_hint(&self) -> &str {
            ""
        }
        fn call(&self, _ctx: ToolCtx, _args: Value) -> ToolCallFuture<'_> {
            Box::pin(async { ToolResult::ok("已经写进去了！") })
        }
    }

    let (_dir, db) = open_test_db();
    let sessions = Arc::new(SessionStore::with_db(Arc::clone(&db)));
    let memory = Arc::new(MemoryStore::with_db(db));

    let mut tools = ToolRegistry::new();
    tools
        .register(Arc::new(Writer(
            ToolMeta::new("file_write", "写文件", "写", Permission::READ_WRITE),
            json!({"type":"object","properties":{}}),
        )))
        .unwrap();
    let mut actions = ActionRegistry::new();
    register_builtins(&mut actions, Arc::clone(&memory)).unwrap();

    let backend = ScriptedBackend::new(vec![
        Turn::Call {
            text: String::new(),
            call_id: "c1".into(),
            name: "file_write".into(),
            arguments: "{}".into(),
        },
        Turn::Text("那我不写了".into()),
    ]);
    let agent = Agent::new(AgentParts {
        backend: Arc::clone(&backend),
        endpoints: vec![LlmEndpoint::new("https://example.invalid/v1", "k")],
        default_model: "default".into(),
        sessions: Arc::clone(&sessions),
        memory,
        tools: Arc::new(tools),
        actions: Arc::new(actions),
        prompt: PromptBuilder::new(),
        max_tool_rounds: 8,
        max_consecutive_tool_failures: 0,
        max_parallel_tools: 3,
        default_enabled_tools: None,
    })
    .unwrap();

    let session_id = sessions
        .create_session(CreateSession {
            work_mode: Mode::Ask,
            ..Default::default()
        })
        .unwrap()
        .id;
    sessions
        .append(&session_id, MessagePayload::user_text("写个文件"), false)
        .unwrap();

    let stream = agent.run_turn(session_id.clone(), CancelToken::new());
    futures_util::pin_mut!(stream);
    let mut events = Vec::new();
    while let Some(event) = stream.next().await {
        events.push(event);
    }

    // 没提供给模型
    assert!(
        !backend
            .last_tool_names()
            .contains(&"file_write".to_string())
    );
    // 硬调也执行不了
    assert!(matches!(
        events
            .iter()
            .find(|e| matches!(e, AgentEvent::CallFinished { .. })),
        Some(AgentEvent::CallFinished { success: false, .. })
    ));
    let tool_msg = backend
        .last_messages()
        .into_iter()
        .find(|m| m.tool_call_id.as_deref() == Some("c1"))
        .unwrap();
    assert!(!tool_msg.content.contains("已经写进去了"), "工具不该被执行");
    assert!(tool_msg.content.contains("ask 模式下不可用"));
    assert!(
        tool_msg.content.contains("request_mode_change"),
        "顺便告诉它出路"
    );
}

#[tokio::test]
async fn disabled_tool_is_blocked_with_a_different_reason() {
    let fx = fixture(vec![
        Turn::Call {
            text: String::new(),
            call_id: "c1".into(),
            name: "echo".into(),
            arguments: r#"{"text":"x"}"#.into(),
        },
        Turn::Text("好".into()),
    ]);
    // 权限够，但会话把工具列表限制成了空
    fx.sessions
        .set_enabled_tools(&fx.session_id, Some(&[]))
        .unwrap();
    fx.say("echo 一下");
    fx.run().await;

    let tool_msg = fx
        .backend
        .last_messages()
        .into_iter()
        .find(|m| m.tool_call_id.as_deref() == Some("c1"))
        .unwrap();
    assert!(tool_msg.content.contains("没有在本会话启用"));
}

#[tokio::test]
async fn manual_mode_switch_leaves_a_marker() {
    let fx = fixture_with(vec![Turn::Text("好的".into())], Mode::Ask, 8);
    fx.say("你好");
    fx.run().await;

    fx.agent.switch_mode(&fx.session_id, Mode::Agent).unwrap();
    assert_eq!(
        fx.sessions
            .get_session(&fx.session_id)
            .unwrap()
            .unwrap()
            .work_mode,
        Mode::Agent
    );

    fx.say("现在帮我跑个命令");
    fx.run().await;

    let system_msgs: Vec<_> = fx
        .backend
        .last_messages()
        .into_iter()
        .filter(|m| m.role == lya_llm::Role::System)
        .collect();
    // 只有系统提示词；模式变更标记留在树上供界面回看，不进 API 上下文
    assert_eq!(system_msgs.len(), 1);
    assert!(fx
        .sessions
        .path_to_active_leaf(&fx.session_id)
        .unwrap()
        .iter()
        .any(|m| m
            .payload
            .openai
            .as_ref()
            .is_some_and(|o| o.content.contains("从 ask 切换为 agent"))));

    // 切到同一个模式不该重复记
    fx.agent.switch_mode(&fx.session_id, Mode::Agent).unwrap();
    fx.say("再来");
    fx.run().await;
    let api_count = fx
        .backend
        .last_messages()
        .into_iter()
        .filter(|m| m.content.contains("[模式变更]"))
        .count();
    assert_eq!(api_count, 0, "模式变更标记不应再进 API 上下文");
    let tree_count = fx
        .sessions
        .path_to_active_leaf(&fx.session_id)
        .unwrap()
        .iter()
        .filter(|m| {
            m.payload
                .openai
                .as_ref()
                .is_some_and(|o| o.content.contains("[模式变更]"))
        })
        .count();
    assert_eq!(tree_count, 1, "树上仍保留一条模式变更记录");
}

/// 一个「执行前要确认」的假工具：记录自己有没有真的被执行过。
struct GuardedTool {
    meta: ToolMeta,
    params: Value,
    ran: Arc<Mutex<Vec<String>>>,
}

impl Tool for GuardedTool {
    fn meta(&self) -> &ToolMeta {
        &self.meta
    }
    fn parameters(&self) -> &Value {
        &self.params
    }
    fn prompt_hint(&self) -> &str {
        "危险"
    }
    fn confirm_request(&self, args: &Value) -> Option<lya_tool::ConfirmRequest> {
        Some(lya_tool::ConfirmRequest {
            summary: format!("执行：{}", args["command"].as_str().unwrap_or("")),
            steps: vec![lya_tool::ConfirmStep {
                raw: args["command"].as_str().unwrap_or("").into(),
                explain: "删除东西".into(),
                risk: Some("不可撤销".into()),
                connector: String::new(),
            }],
            reasons: vec!["会删文件".into()],
        })
    }
    fn call(&self, _ctx: ToolCtx, args: Value) -> ToolCallFuture<'_> {
        Box::pin(async move {
            let command = args["command"].as_str().unwrap_or("").to_string();
            self.ran.lock().unwrap().push(command.clone());
            ToolResult::ok(format!("已执行 {command}"))
        })
    }
}

/// 装一个需要确认的工具，返回执行记录。
fn fixture_with_guarded(turns: Vec<Turn>) -> (Fixture, Arc<Mutex<Vec<String>>>) {
    let mut fx = fixture_with(turns, Mode::Agent, 8);
    let ran = Arc::new(Mutex::new(Vec::new()));

    let mut tools = ToolRegistry::new();
    tools.register(Arc::new(EchoTool::new())).unwrap();
    tools
        .register(Arc::new(GuardedTool {
            meta: ToolMeta::new("danger", "危险", "危险操作", Permission::READ_WRITE_EXEC),
            params: json!({
                "type": "object",
                "properties": { "command": { "type": "string" } },
                "required": ["command"]
            }),
            ran: Arc::clone(&ran),
        }))
        .unwrap();

    let mut actions = ActionRegistry::new();
    register_builtins(
        &mut actions,
        Arc::new(MemoryStore::open(fx._dir.path().join("m.db")).unwrap()),
    )
    .unwrap();

    fx.agent = Agent::new(AgentParts {
        backend: Arc::clone(&fx.backend),
        endpoints: vec![LlmEndpoint::new("https://example.invalid/v1", "k")],
        default_model: "default".into(),
        sessions: Arc::clone(&fx.sessions),
        memory: Arc::new(MemoryStore::open(fx._dir.path().join("m2.db")).unwrap()),
        tools: Arc::new(tools),
        actions: Arc::new(actions),
        prompt: PromptBuilder::new(),
        max_tool_rounds: 8,
        max_consecutive_tool_failures: 0,
        max_parallel_tools: 3,
        default_enabled_tools: None,
    })
    .unwrap();
    (fx, ran)
}

#[tokio::test]
async fn risky_tool_suspends_before_running() {
    let (fx, ran) = fixture_with_guarded(vec![
        Turn::Call {
            text: "我来删一下".into(),
            call_id: "c1".into(),
            name: "danger".into(),
            arguments: r#"{"command":"rm -rf build"}"#.into(),
        },
        Turn::Text("删完了".into()),
    ]);
    fx.say("清理一下");

    let events = fx.run().await;
    assert_eq!(end_reason(&events), TurnEndReason::AwaitingHuman);
    assert!(ran.lock().unwrap().is_empty(), "放行之前一步都不能执行");

    // HITL 节点里存下了工具名与参数，恢复时才照得着
    let hitl_id = fx.sessions.pending_hitl(&fx.session_id).unwrap().unwrap();
    let record = fx.sessions.get_message(&fx.session_id, hitl_id).unwrap();
    let Some(HitlBlock::ToolConfirm {
        tool_name,
        arguments,
        steps,
        tool_call_id,
        ..
    }) = record.payload.lya.hitl.clone()
    else {
        panic!("应当是工具确认块");
    };
    assert_eq!(tool_name, "danger");
    assert_eq!(arguments["command"], "rm -rf build");
    assert_eq!(tool_call_id, "c1");
    assert_eq!(steps[0].risk.as_deref(), Some("不可撤销"));
}

#[tokio::test]
async fn approval_executes_and_feeds_the_output_back() {
    let (fx, ran) = fixture_with_guarded(vec![
        Turn::Call {
            text: String::new(),
            call_id: "c1".into(),
            name: "danger".into(),
            arguments: r#"{"command":"rm -rf build"}"#.into(),
        },
        Turn::Text("好了".into()),
    ]);
    fx.say("清理");
    fx.run().await;

    fx.agent
        .resolve_tool_confirm(
            &fx.session_id,
            true,
            Some("可以，但别动日志"),
            CancelToken::new(),
        )
        .await
        .unwrap();
    fx.agent
        .flush_deferred_tool_executions(&fx.session_id, CancelToken::new())
        .await
        .unwrap();

    assert_eq!(
        ran.lock().unwrap().as_slice(),
        ["rm -rf build"],
        "放行后才执行"
    );
    assert_eq!(fx.sessions.pending_hitl(&fx.session_id).unwrap(), None);

    let events = fx.run().await;
    assert_eq!(end_reason(&events), TurnEndReason::Completed);

    let tool_msg = fx
        .backend
        .last_messages()
        .into_iter()
        .find(|m| m.tool_call_id.as_deref() == Some("c1"))
        .expect("执行结果要回灌");
    assert!(tool_msg.content.contains("已执行 rm -rf build"));
    assert!(tool_msg.content.contains("[用户备注: 可以，但别动日志]"));
}

#[tokio::test]
async fn rejection_does_not_execute() {
    let (fx, ran) = fixture_with_guarded(vec![
        Turn::Call {
            text: String::new(),
            call_id: "c1".into(),
            name: "danger".into(),
            arguments: r#"{"command":"rm -rf /"}"#.into(),
        },
        Turn::Text("那我不删了".into()),
    ]);
    fx.say("清理");
    fx.run().await;

    fx.agent
        .resolve_tool_confirm(&fx.session_id, false, Some("太危险了"), CancelToken::new())
        .await
        .unwrap();
    assert!(ran.lock().unwrap().is_empty(), "拒绝就是一步都不执行");

    fx.run().await;
    let tool_msg = fx
        .backend
        .last_messages()
        .into_iter()
        .find(|m| m.tool_call_id.as_deref() == Some("c1"))
        .unwrap();
    assert!(tool_msg.content.contains("[用户拒绝]"));
    assert!(tool_msg.content.contains("太危险了"));
}

#[tokio::test]
async fn permission_is_rechecked_after_approval() {
    let (fx, ran) = fixture_with_guarded(vec![Turn::Call {
        text: String::new(),
        call_id: "c1".into(),
        name: "danger".into(),
        arguments: r#"{"command":"rm -rf build"}"#.into(),
    }]);
    fx.say("清理");
    fx.run().await;

    // 挂起期间用户把会话降到 ask，放行也不该执行
    fx.sessions
        .set_work_mode(&fx.session_id, Mode::Ask)
        .unwrap();
    fx.agent
        .resolve_tool_confirm(&fx.session_id, true, None, CancelToken::new())
        .await
        .unwrap();
    fx.agent
        .flush_deferred_tool_executions(&fx.session_id, CancelToken::new())
        .await
        .unwrap();

    assert!(ran.lock().unwrap().is_empty(), "权限已经不够了，不能执行");
    let record = fx
        .sessions
        .path_to_active_leaf(&fx.session_id)
        .unwrap()
        .into_iter()
        .next_back()
        .unwrap();
    let content = &record.payload.openai.unwrap().content;
    assert!(content.contains("重新检查权限"), "{content}");
}

#[tokio::test]
async fn session_model_selection_is_honoured() {
    let (_dir, db) = open_test_db();
    let sessions = Arc::new(SessionStore::with_db(Arc::clone(&db)));
    let memory = Arc::new(MemoryStore::with_db(db));

    let mut actions = ActionRegistry::new();
    register_builtins(&mut actions, Arc::clone(&memory)).unwrap();
    let backend = ScriptedBackend::new(vec![Turn::Text("好".into()), Turn::Text("好".into())]);

    let agent = Agent::new(AgentParts {
        backend: Arc::clone(&backend),
        endpoints: vec![
            LlmEndpoint::new("https://a.invalid/v1", "k").with_id("cheap"),
            LlmEndpoint::new("https://b.invalid/v1", "k").with_id("smart"),
        ],
        default_model: "cheap".into(),
        sessions: Arc::clone(&sessions),
        memory,
        tools: Arc::new(ToolRegistry::new()),
        actions: Arc::new(actions),
        prompt: PromptBuilder::new(),
        max_tool_rounds: 4,
        max_consecutive_tool_failures: 0,
        max_parallel_tools: 3,
        default_enabled_tools: None,
    })
    .unwrap();

    let id = sessions
        .create_session(CreateSession::default())
        .unwrap()
        .id;
    sessions
        .append(&id, MessagePayload::user_text("hi"), false)
        .unwrap();

    // 没指定就用默认模型
    let stream = agent.run_turn(id.clone(), CancelToken::new());
    futures_util::pin_mut!(stream);
    while stream.next().await.is_some() {}

    // 指名一个不存在的模型要报错，而不是悄悄换成默认的
    sessions.set_model(&id, Some("ghost")).unwrap();
    sessions
        .append(&id, MessagePayload::user_text("再来"), false)
        .unwrap();
    let stream = agent.run_turn(id.clone(), CancelToken::new());
    futures_util::pin_mut!(stream);
    let mut events = Vec::new();
    while let Some(event) = stream.next().await {
        events.push(event);
    }
    assert!(
        matches!(end_reason(&events), TurnEndReason::Failed(msg) if msg.contains("ghost")),
        "{:?}",
        end_reason(&events)
    );

    // 换成存在的模型就能继续
    sessions.set_model(&id, Some("smart")).unwrap();
    let stream = agent.run_turn(id.clone(), CancelToken::new());
    futures_util::pin_mut!(stream);
    let mut events = Vec::new();
    while let Some(event) = stream.next().await {
        events.push(event);
    }
    assert_eq!(end_reason(&events), TurnEndReason::Completed);
}

#[tokio::test]
async fn default_model_must_exist() {
    let (_dir, db) = open_test_db();
    let memory = Arc::new(MemoryStore::with_db(Arc::clone(&db)));

    let result = Agent::new(AgentParts {
        backend: ScriptedBackend::new(vec![]),
        endpoints: vec![LlmEndpoint::new("https://a.invalid/v1", "k").with_id("cheap")],
        default_model: "nope".into(),
        sessions: Arc::new(SessionStore::with_db(db)),
        memory,
        tools: Arc::new(ToolRegistry::new()),
        actions: Arc::new(ActionRegistry::new()),
        prompt: PromptBuilder::new(),
        max_tool_rounds: 4,
        max_consecutive_tool_failures: 0,
        max_parallel_tools: 3,
        default_enabled_tools: None,
    });
    assert!(matches!(
        result.err(),
        Some(lya_agent::AgentError::Invalid(_))
    ));
}

#[tokio::test]
async fn name_collision_is_rejected_at_construction() {
    let (_dir, db) = open_test_db();
    let memory = Arc::new(MemoryStore::with_db(Arc::clone(&db)));

    // 注册一个和动作同名的工具
    struct Clash(ToolMeta, Value);
    impl Tool for Clash {
        fn meta(&self) -> &ToolMeta {
            &self.0
        }
        fn parameters(&self) -> &Value {
            &self.1
        }
        fn prompt_hint(&self) -> &str {
            ""
        }
        fn call(&self, _ctx: ToolCtx, _args: Value) -> ToolCallFuture<'_> {
            Box::pin(async { ToolResult::ok("") })
        }
    }
    let mut tools = ToolRegistry::new();
    tools
        .register(Arc::new(Clash(
            ToolMeta::new("memory_write", "撞名", "", Permission::READ),
            json!({}),
        )))
        .unwrap();

    let mut actions = ActionRegistry::new();
    register_builtins(&mut actions, Arc::clone(&memory)).unwrap();

    let result = Agent::new(AgentParts {
        backend: ScriptedBackend::new(vec![]),
        endpoints: vec![LlmEndpoint::new("https://example.invalid/v1", "k")],
        default_model: "default".into(),
        sessions: Arc::new(SessionStore::with_db(db)),
        memory,
        tools: Arc::new(tools),
        actions: Arc::new(actions),
        prompt: PromptBuilder::new(),
        max_tool_rounds: 8,
        max_consecutive_tool_failures: 0,
        max_parallel_tools: 3,
        default_enabled_tools: None,
    });
    assert!(matches!(
        result.err(),
        Some(lya_agent::AgentError::NameCollision(name)) if name == "memory_write"
    ));
}

async fn flush_if_batch_clear(fx: &Fixture) {
    if fx.sessions.pending_hitl(&fx.session_id).unwrap().is_none() {
        fx.agent
            .flush_deferred_tool_executions(&fx.session_id, CancelToken::new())
            .await
            .unwrap();
    }
}

#[tokio::test]
async fn batch_creates_two_hitl_without_stubbing_second() {
    let (fx, ran) = fixture_with_guarded(vec![
        Turn::Calls {
            text: "两个都要确认".into(),
            calls: vec![
                (
                    "c1".into(),
                    "danger".into(),
                    r#"{"command":"rm -rf build"}"#.into(),
                ),
                (
                    "c2".into(),
                    "danger".into(),
                    r#"{"command":"rm -rf dist"}"#.into(),
                ),
            ],
        },
        Turn::Text("好了".into()),
    ]);
    fx.say("清理");
    let events = fx.run().await;
    assert_eq!(end_reason(&events), TurnEndReason::AwaitingHuman);
    assert!(ran.lock().unwrap().is_empty(), "确认前一步都不该执行");

    let pending = fx.sessions.pending_hitl_all(&fx.session_id).unwrap();
    assert_eq!(pending.len(), 2, "两条都应挂起，而不是把第二条 stub 掉");

    let first = fx.sessions.get_message(&fx.session_id, pending[0]).unwrap();
    let second = fx.sessions.get_message(&fx.session_id, pending[1]).unwrap();
    assert_eq!(
        first.payload.lya.meta.as_ref().and_then(|m| m.get("batch_index")),
        Some(&json!(1))
    );
    assert_eq!(
        second.payload.lya.meta.as_ref().and_then(|m| m.get("batch_index")),
        Some(&json!(2))
    );
}

#[tokio::test]
async fn batch_auto_runs_in_parallel_with_pending_confirms() {
    let (fx, ran) = fixture_with_guarded(vec![
        Turn::Calls {
            text: "一个自动一个要确认".into(),
            calls: vec![
                ("c_auto".into(), "echo".into(), r#"{"text":"hi"}"#.into()),
                (
                    "c1".into(),
                    "danger".into(),
                    r#"{"command":"rm -rf build"}"#.into(),
                ),
            ],
        },
        Turn::Text("好了".into()),
    ]);
    fx.say("混合");
    fx.run().await;

    let path = fx.sessions.path_to_active_leaf(&fx.session_id).unwrap();
    assert!(
        path.iter()
            .any(|m| m.payload.openai.as_ref().is_some_and(|o| {
                o.tool_call_id.as_deref() == Some("c_auto")
                    && o.content.contains("echo: hi")
            })),
        "auto 项应立刻落库"
    );
    assert_eq!(fx.sessions.pending_hitl_all(&fx.session_id).unwrap().len(), 1);
    assert!(ran.lock().unwrap().is_empty(), "需确认的仍不能执行");
}

#[tokio::test]
async fn batch_executes_approved_confirms_after_all_reviewed() {
    let (fx, ran) = fixture_with_guarded(vec![
        Turn::Calls {
            text: String::new(),
            calls: vec![
                (
                    "c1".into(),
                    "danger".into(),
                    r#"{"command":"rm -rf build"}"#.into(),
                ),
                (
                    "c2".into(),
                    "danger".into(),
                    r#"{"command":"rm -rf dist"}"#.into(),
                ),
            ],
        },
        Turn::Text("删完了".into()),
    ]);
    fx.say("清理");
    fx.run().await;

    fx.agent
        .resolve_tool_confirm(&fx.session_id, true, None, CancelToken::new())
        .await
        .unwrap();
    assert!(ran.lock().unwrap().is_empty(), "本批未审完不应执行");

    fx.agent
        .resolve_tool_confirm(&fx.session_id, true, None, CancelToken::new())
        .await
        .unwrap();
    flush_if_batch_clear(&fx).await;

    assert_eq!(
        ran.lock().unwrap().as_slice(),
        ["rm -rf build", "rm -rf dist"],
        "本批审完后按序执行"
    );
}

#[tokio::test]
async fn responses_session_uses_responses_stack() {
    let fx = fixture_with_session(vec![Turn::Text("好".into())], Mode::Agent, 3, Some("responses"));
    fx.say("你好");
    fx.run().await;
    assert_eq!(fx.backend.last_mode(), Some(ApiMode::Responses));
    assert!(
        fx.backend
            .last_instructions()
            .as_ref()
            .is_some_and(|s| !s.is_empty())
    );
    let input = fx.backend.last_input().unwrap();
    assert_eq!(input.len(), 1);
    assert_eq!(input[0]["type"], "message");
    assert_eq!(input[0]["role"], "user");
}

#[tokio::test]
async fn responses_native_web_excludes_ddg_search() {
    let (_dir, db) = open_test_db();

    let sessions = Arc::new(SessionStore::with_db(Arc::clone(&db)));
    let memory = Arc::new(MemoryStore::with_db(db));

    let mut tools = ToolRegistry::new();
    tools.register(Arc::new(EchoTool::new())).unwrap();
    tools.register(Arc::new(WebSearchTool::new())).unwrap();

    let mut actions = ActionRegistry::new();
    register_builtins(&mut actions, Arc::clone(&memory)).unwrap();

    let backend = ScriptedBackend::new(vec![Turn::Text("好".into())]);
    let agent = Agent::new(AgentParts {
        backend: Arc::clone(&backend),
        endpoints: vec![LlmEndpoint::new("https://example.invalid/v1", "k")
            .with_id("default")
            .with_mode_params(
                ApiMode::Completions,
                serde_json::from_value(json!({ "model": "demo" })).unwrap(),
            )
            .with_mode_params(
                ApiMode::Responses,
                serde_json::from_value(json!({ "model": "demo" })).unwrap(),
            )
            .with_mode_capabilities(
                ApiMode::Responses,
                vec!["text".into(), "web_search".into()],
            )],
        default_model: "default".into(),
        sessions: Arc::clone(&sessions),
        memory,
        tools: Arc::new(tools),
        actions: Arc::new(actions),
        prompt: PromptBuilder::new(),
        max_tool_rounds: 3,
        max_consecutive_tool_failures: 0,
        max_parallel_tools: 3,
        default_enabled_tools: None,
    })
    .unwrap();

    let session_id = sessions
        .create_session(CreateSession {
            work_mode: Mode::Agent,
            api_mode: Some("responses".into()),
            ..Default::default()
        })
        .unwrap()
        .id;

    sessions
        .append(
            &session_id,
            MessagePayload::user_text("查一下"),
            false,
        )
        .unwrap();
    let stream = agent.run_turn(&session_id, CancelToken::new());
    futures_util::pin_mut!(stream);
    while stream.next().await.is_some() {}

    let names = backend.last_tool_names();
    assert!(!names.iter().any(|n| n == "web_search"));
    assert!(names.iter().any(|n| n == "echo"));
}

#[tokio::test]
async fn responses_native_search_persists_and_replays_with_web_fetch() {
    let (_dir, db) = open_test_db();

    let sessions = Arc::new(SessionStore::with_db(Arc::clone(&db)));
    let memory = Arc::new(MemoryStore::with_db(db));

    let mut tools = ToolRegistry::new();
    tools.register(Arc::new(WebSearchTool::new())).unwrap();
    tools.register(Arc::new(WebFetchTool::new())).unwrap();

    let mut actions = ActionRegistry::new();
    register_builtins(&mut actions, Arc::clone(&memory)).unwrap();

    let backend = ScriptedBackend::new(vec![
        Turn::NativeSearch {
            call_id: "ws1".into(),
            query: "Rust 2024".into(),
            text: "搜到了".into(),
        },
        Turn::Call {
            text: "我读一下".into(),
            call_id: "c1".into(),
            name: "web_fetch".into(),
            arguments: r#"{"url":"https://example.com/rust"}"#.into(),
        },
        Turn::Text("读完了".into()),
    ]);
    let agent = Agent::new(AgentParts {
        backend: Arc::clone(&backend),
        endpoints: vec![LlmEndpoint::new("https://example.invalid/v1", "k")
            .with_id("default")
            .with_mode_params(
                ApiMode::Completions,
                serde_json::from_value(json!({ "model": "demo" })).unwrap(),
            )
            .with_mode_params(
                ApiMode::Responses,
                serde_json::from_value(json!({ "model": "demo" })).unwrap(),
            )
            .with_mode_capabilities(
                ApiMode::Responses,
                vec!["text".into(), "web_search".into()],
            )],
        default_model: "default".into(),
        sessions: Arc::clone(&sessions),
        memory,
        tools: Arc::new(tools),
        actions: Arc::new(actions),
        prompt: PromptBuilder::new(),
        max_tool_rounds: 4,
        max_consecutive_tool_failures: 0,
        max_parallel_tools: 3,
        default_enabled_tools: None,
    })
    .unwrap();

    let session_id = sessions
        .create_session(CreateSession {
            work_mode: Mode::Agent,
            api_mode: Some("responses".into()),
            ..Default::default()
        })
        .unwrap()
        .id;

    sessions
        .append(&session_id, MessagePayload::user_text("查 Rust"), false)
        .unwrap();
    let stream = agent.run_turn(&session_id, CancelToken::new());
    futures_util::pin_mut!(stream);
    while stream.next().await.is_some() {}

    let path = sessions.path_to_active_leaf(&session_id).unwrap();
    let search_msg = path
        .iter()
        .find(|m| m.payload.role == MessageRole::Assistant)
        .expect("原生搜索应落库");
    assert_eq!(search_msg.payload.lya.responses_items.len(), 1);
    assert_eq!(
        search_msg.payload.lya.responses_items[0]["type"],
        "web_search_call"
    );

    let names = backend.last_tool_names();
    assert!(!names.iter().any(|n| n == "web_search"));
    assert!(names.iter().any(|n| n == "web_fetch"));

    let stream = agent.run_turn(&session_id, CancelToken::new());
    futures_util::pin_mut!(stream);
    while stream.next().await.is_some() {}

    let input = backend.last_input().unwrap();
    assert!(
        input.iter().any(|i| i["type"] == "web_search_call" && i["id"] == "ws1"),
        "第二轮应回灌历史 search item"
    );
    let tool_msg = input
        .iter()
        .find(|i| i["type"] == "function_call_output" && i["call_id"] == "c1")
        .expect("web_fetch 结果应回灌");
    assert!(tool_msg["output"].as_str().unwrap().contains("example.com/rust"));
}
