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
use lya_db::Db;
use lya_llm::{ChatEventStream, ChatMessage, LlmEndpoint, LlmError, StreamEvent, ToolCallDelta};
use lya_memory::MemoryStore;
use lya_mode::Mode;
use lya_prompt::PromptBuilder;
use lya_session::{CreateSession, MessagePayload, MessageRole, SessionStore};
use lya_tool::{Permission, Tool, ToolMeta, ToolRegistry, ToolResult, traits::ToolCallFuture};
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
    /// 请求直接失败。
    Fail(String),
}

#[derive(Default)]
struct ScriptedBackend {
    turns: Mutex<Vec<Turn>>,
    /// 每轮收到的 messages，供断言上下文装配。
    seen: Mutex<Vec<Vec<ChatMessage>>>,
    /// 每轮收到的 tools schema 名字。
    seen_tools: Mutex<Vec<Vec<String>>>,
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
}

impl ChatBackend for ScriptedBackend {
    fn chat_stream<'a>(
        &'a self,
        _endpoint: &'a LlmEndpoint,
        messages: Vec<ChatMessage>,
        tools: Vec<Value>,
    ) -> Pin<Box<dyn Future<Output = Result<ChatEventStream, LlmError>> + Send + 'a>> {
        self.seen.lock().unwrap().push(messages);
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
    fn call(&self, args: Value) -> ToolCallFuture<'_> {
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
    let dir = tempfile::tempdir().unwrap();
    let db = Db::open(dir.path().join("lya.db"))
        .unwrap()
        .with_migration(lya_session::MIGRATION_SQL)
        .with_migration(lya_memory::MIGRATION_SQL);
    db.migrate().unwrap();
    let db = Arc::new(db);

    let sessions = Arc::new(SessionStore::with_db(Arc::clone(&db)));
    let memory = Arc::new(MemoryStore::with_db(db));

    let mut tools = ToolRegistry::new();
    tools.register(Arc::new(EchoTool::new())).unwrap();

    let mut actions = ActionRegistry::new();
    register_builtins(&mut actions, Arc::clone(&memory)).unwrap();

    let backend = ScriptedBackend::new(turns);
    let agent = Agent::new(AgentParts {
        backend: Arc::clone(&backend),
        endpoint: LlmEndpoint::new("https://example.invalid/v1", "k"),
        sessions: Arc::clone(&sessions),
        memory,
        tools: Arc::new(tools),
        actions: Arc::new(actions),
        prompt: PromptBuilder::new(),
        max_tool_rounds: max_rounds,
    })
    .unwrap();

    let session_id = sessions
        .create_session(CreateSession {
            work_mode: mode,
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

#[tokio::test]
async fn empty_response_leaves_no_stray_message() {
    let fx = fixture(vec![Turn::Text(String::new())]);
    fx.say("在吗");
    let events = fx.run().await;

    assert_eq!(end_reason(&events), TurnEndReason::EmptyResponse);
    let path = fx.sessions.path_to_active_leaf(&fx.session_id).unwrap();
    assert_eq!(path.len(), 1, "空回复不该在树上留下空壳");
}

#[tokio::test]
async fn request_failure_removes_the_placeholder() {
    let fx = fixture(vec![Turn::Fail("401 unauthorized".into())]);
    fx.say("你好");
    let events = fx.run().await;

    assert!(matches!(end_reason(&events), TurnEndReason::Failed(msg) if msg.contains("401")));
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
    let Some(AgentEvent::AwaitHuman { message_id }) = events
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
        fn call(&self, _args: Value) -> ToolCallFuture<'_> {
            Box::pin(async { ToolResult::ok("已经写进去了！") })
        }
    }

    let dir = tempfile::tempdir().unwrap();
    let db = Arc::new(
        Db::open(dir.path().join("lya.db"))
            .unwrap()
            .with_migration(lya_session::MIGRATION_SQL)
            .with_migration(lya_memory::MIGRATION_SQL),
    );
    db.migrate().unwrap();
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
        endpoint: LlmEndpoint::new("https://example.invalid/v1", "k"),
        sessions: Arc::clone(&sessions),
        memory,
        tools: Arc::new(tools),
        actions: Arc::new(actions),
        prompt: PromptBuilder::new(),
        max_tool_rounds: 8,
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
    // 第一条是系统提示词，第二条是模式变更标记
    assert_eq!(system_msgs.len(), 2);
    assert!(system_msgs[1].content.contains("从 ask 切换为 agent"));

    // 切到同一个模式不该重复记
    fx.agent.switch_mode(&fx.session_id, Mode::Agent).unwrap();
    fx.say("再来");
    fx.run().await;
    let count = fx
        .backend
        .last_messages()
        .into_iter()
        .filter(|m| m.content.contains("[模式变更]"))
        .count();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn name_collision_is_rejected_at_construction() {
    let dir = tempfile::tempdir().unwrap();
    let db = Arc::new(
        Db::open(dir.path().join("lya.db"))
            .unwrap()
            .with_migration(lya_session::MIGRATION_SQL)
            .with_migration(lya_memory::MIGRATION_SQL),
    );
    db.migrate().unwrap();
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
        fn call(&self, _args: Value) -> ToolCallFuture<'_> {
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
        endpoint: LlmEndpoint::new("https://example.invalid/v1", "k"),
        sessions: Arc::new(SessionStore::with_db(db)),
        memory,
        tools: Arc::new(tools),
        actions: Arc::new(actions),
        prompt: PromptBuilder::new(),
        max_tool_rounds: 8,
    });
    assert!(matches!(
        result.err(),
        Some(lya_agent::AgentError::NameCollision(name)) if name == "memory_write"
    ));
}
