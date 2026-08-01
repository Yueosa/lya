//! 用一个可以卡住的假后端验 Hub 的编排逻辑。
//!
//! 真实后端做不了这些测试：轮次串行要求「上一轮还没结束时再发一条」，而真实
//! 调用要么很快返回、要么依赖网络时序，都不可靠。

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use lya_action::ActionRegistry;
use lya_agent::{Agent, AgentParts, ChatBackend};
use lya_core::SessionHub;
use lya_db::Db;
use lya_llm::{ChatEventStream, ChatMessage, LlmEndpoint, LlmError, StreamEvent};
use lya_memory::MemoryStore;
use lya_prompt::PromptBuilder;
use lya_session::{CreateSession, MessagePayload, SessionStore};
use lya_tool::ToolRegistry;
use serde_json::Value;
use tempfile::TempDir;

/// 一个会一直吐字直到被叫停的后端。
struct SlowBackend {
    /// 置位后停止吐字并收尾。
    stop: Arc<AtomicBool>,
}

impl ChatBackend for SlowBackend {
    fn chat_stream<'a>(
        &'a self,
        _endpoint: &'a LlmEndpoint,
        _messages: Vec<ChatMessage>,
        _tools: Vec<Value>,
    ) -> Pin<Box<dyn Future<Output = Result<ChatEventStream, LlmError>> + Send + 'a>> {
        let stop = Arc::clone(&self.stop);
        Box::pin(async move {
            let stream = async_stream::stream! {
                loop {
                    if stop.load(Ordering::Relaxed) {
                        yield Ok(StreamEvent::Finished { reason: Some("stop".into()) });
                        return;
                    }
                    yield Ok(StreamEvent::TextDelta("喵".into()));
                    tokio::time::sleep(Duration::from_millis(20)).await;
                }
            };
            Ok(Box::pin(stream) as ChatEventStream)
        })
    }
}

struct Fixture {
    _dir: TempDir,
    hub: Arc<SessionHub<SlowBackend>>,
    sessions: Arc<SessionStore>,
    stop: Arc<AtomicBool>,
    session_id: String,
}

fn fixture() -> Fixture {
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
    let stop = Arc::new(AtomicBool::new(false));

    let agent = Arc::new(
        Agent::new(AgentParts {
            backend: SlowBackend {
                stop: Arc::clone(&stop),
            },
            endpoints: vec![LlmEndpoint::new("https://example.invalid/v1", "k")],
            default_model: "default".into(),
            sessions: Arc::clone(&sessions),
            memory,
            tools: Arc::new(ToolRegistry::new()),
            actions: Arc::new(ActionRegistry::new()),
            prompt: PromptBuilder::new(),
            max_tool_rounds: 4,
        })
        .unwrap(),
    );

    let session_id = sessions
        .create_session(CreateSession::default())
        .unwrap()
        .id;
    Fixture {
        _dir: dir,
        hub: SessionHub::new(agent),
        sessions,
        stop,
        session_id,
    }
}

impl Fixture {
    fn say(&self, text: &str) {
        self.sessions
            .append(&self.session_id, MessagePayload::user_text(text), false)
            .unwrap();
    }
}

#[tokio::test]
async fn a_second_turn_is_refused_while_one_is_running() {
    let fx = fixture();
    fx.say("你好");
    fx.hub.start_turn(&fx.session_id).unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(fx.hub.is_running(&fx.session_id));

    let err = fx.hub.start_turn(&fx.session_id).unwrap_err();
    assert!(
        matches!(err, lya_core::HubError::Busy(_)),
        "同一会话不能同时跑两轮：{err}"
    );

    fx.stop.store(true, Ordering::Relaxed);
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert!(!fx.hub.is_running(&fx.session_id), "结束后要能再跑");
    fx.hub.start_turn(&fx.session_id).unwrap();
}

#[tokio::test]
async fn stop_cancels_the_running_turn() {
    let fx = fixture();
    fx.say("你好");
    fx.hub.start_turn(&fx.session_id).unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;

    assert!(fx.hub.stop(&fx.session_id));
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert!(!fx.hub.is_running(&fx.session_id));

    // 没有轮次在跑时停止是无操作，不该报错
    assert!(!fx.hub.stop(&fx.session_id));
}

#[tokio::test]
async fn late_subscriber_gets_what_it_missed() {
    let fx = fixture();
    fx.say("你好");
    fx.hub.start_turn(&fx.session_id).unwrap();
    // 等它吐几个字，模拟「用户中途刷新了页面」
    tokio::time::sleep(Duration::from_millis(150)).await;

    let (snapshot, _rx) = fx.hub.subscribe(&fx.session_id).unwrap();
    let running = snapshot.running.expect("应当能看到正在跑的那轮");
    assert!(
        !running.content.is_empty(),
        "错过的增量要能从快照里补回来，否则刷新就丢渲染"
    );
    assert!(running.content.starts_with('喵'));
    // 用户那条 + 正在写的那条占位（内容还没落库，所以要靠 running 补）
    assert_eq!(snapshot.messages.len(), 2);
    assert_eq!(running.message_id, snapshot.messages[1].id.into());

    fx.stop.store(true, Ordering::Relaxed);
}

#[tokio::test]
async fn subscriber_receives_live_events() {
    let fx = fixture();
    let (_snapshot, mut rx) = fx.hub.subscribe(&fx.session_id).unwrap();
    fx.say("你好");
    fx.hub.start_turn(&fx.session_id).unwrap();

    let first = tokio::time::timeout(Duration::from_secs(2), rx.recv())
        .await
        .expect("应当很快收到事件")
        .unwrap();
    assert_eq!(first.kind, "round_started");
    assert_eq!(first.scope, format!("session:{}", fx.session_id));

    fx.stop.store(true, Ordering::Relaxed);
}

#[tokio::test]
async fn buffer_is_cleared_after_the_turn() {
    let fx = fixture();
    fx.say("你好");
    fx.hub.start_turn(&fx.session_id).unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;
    fx.stop.store(true, Ordering::Relaxed);
    tokio::time::sleep(Duration::from_millis(250)).await;

    let snapshot = fx.hub.snapshot(&fx.session_id).unwrap();
    assert!(snapshot.running.is_none(), "轮次结束后不该还留着缓冲");
    // 内容已经落库，从消息树里读得到
    let last = snapshot.messages.last().unwrap();
    assert!(
        last.payload
            .openai
            .as_ref()
            .is_some_and(|m| m.content.contains('喵'))
    );
}

#[tokio::test]
async fn unknown_session_is_reported() {
    let fx = fixture();
    assert!(matches!(
        fx.hub.start_turn("nope").unwrap_err(),
        lya_core::HubError::NotFound(_)
    ));
    assert!(matches!(
        fx.hub.snapshot("nope").unwrap_err(),
        lya_core::HubError::NotFound(_)
    ));
}
