//! 用一个可以卡住的假后端验 Hub 的编排逻辑。
//!
//! 真实后端做不了这些测试：轮次串行要求「上一轮还没结束时再发一条」，而真实
//! 调用要么很快返回、要么依赖网络时序，都不可靠。

use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicU16, AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use lya_action::ActionRegistry;
use lya_agent::{Agent, AgentParts, ChatBackend};
use lya_hub::SessionHub;
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
            .with_migrations(lya_session::MIGRATION_SCOPE, lya_session::MIGRATIONS)
            .with_migrations(lya_memory::MIGRATION_SCOPE, lya_memory::MIGRATIONS),
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
            max_parallel_tools: 3,
            default_enabled_tools: None,
        })
        .unwrap(),
    );

    let session_id = sessions
        .create_session(CreateSession::default())
        .unwrap()
        .id;
    Fixture {
        _dir: dir,
        hub: SessionHub::new(
            agent,
            lya_http::HttpClient::with_defaults().unwrap(),
            Arc::new(AtomicU16::new(0)),
        ),
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
        matches!(err, lya_hub::HubError::Busy(_)),
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

/// 让假后端重新开始吐字。每次开跑前都要调，否则新一轮会立刻空回复收场。
fn arm(fx: &Fixture) {
    fx.stop.store(false, Ordering::Relaxed);
}

/// 让它吐一会儿，然后叫停并等轮次真正结束。
async fn settle(fx: &Fixture) {
    tokio::time::sleep(Duration::from_millis(60)).await;
    fx.stop.store(true, Ordering::Relaxed);
    for _ in 0..50 {
        if !fx.hub.is_running(&fx.session_id) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("轮次没能结束");
}

/// 跑一轮并等它结束，用于把分支铺出来。
async fn run_once(fx: &Fixture) {
    arm(fx);
    fx.hub.start_turn(&fx.session_id).unwrap();
    settle(fx).await;
}

#[tokio::test]
async fn regenerate_forks_instead_of_overwriting() {
    let fx = fixture();
    fx.say("你好");
    run_once(&fx).await;

    let before = fx.hub.snapshot(&fx.session_id).unwrap();
    let first_answer = before.messages.last().unwrap().id;
    assert_eq!(fx.hub.branches(&fx.session_id).unwrap().len(), 1);

    arm(&fx);
    fx.hub.regenerate(&fx.session_id).unwrap();
    settle(&fx).await;

    let branches = fx.hub.branches(&fx.session_id).unwrap();
    assert_eq!(branches.len(), 2, "重新生成应当分叉，而不是覆盖原答案");
    assert!(
        branches.iter().any(|b| b.leaf_id == first_answer),
        "旧答案要留着，随时能切回去"
    );
    assert_eq!(
        branches.iter().filter(|b| b.is_active).count(),
        1,
        "只能有一条当前分支"
    );
    assert!(
        !branches
            .iter()
            .find(|b| b.is_active)
            .unwrap()
            .leaf_id
            .eq(&first_answer)
    );

    // 当前路径里只有一条用户消息，没被复制
    let after = fx.hub.snapshot(&fx.session_id).unwrap();
    let users = after
        .messages
        .iter()
        .filter(|m| m.payload.role == lya_session::MessageRole::User)
        .count();
    assert_eq!(users, 1);
}

#[tokio::test]
async fn switching_branches_changes_the_visible_path() {
    let fx = fixture();
    fx.say("你好");
    run_once(&fx).await;
    let first = fx
        .hub
        .snapshot(&fx.session_id)
        .unwrap()
        .messages
        .last()
        .unwrap()
        .id;

    arm(&fx);
    fx.hub.regenerate(&fx.session_id).unwrap();
    settle(&fx).await;

    fx.hub.switch_branch(&fx.session_id, first).unwrap();
    let snapshot = fx.hub.snapshot(&fx.session_id).unwrap();
    assert_eq!(snapshot.messages.last().unwrap().id, first);
    assert_eq!(snapshot.session.active_leaf_id, Some(first));
}

#[tokio::test]
async fn editing_a_message_keeps_the_old_one_on_a_sibling_branch() {
    let fx = fixture();
    fx.say("原来的问题");
    run_once(&fx).await;

    let path = fx.hub.snapshot(&fx.session_id).unwrap().messages;
    let user_msg = path[0].id;

    arm(&fx);
    fx.hub
        .edit_and_resend(&fx.session_id, user_msg, "改过的问题")
        .unwrap();
    settle(&fx).await;

    let snapshot = fx.hub.snapshot(&fx.session_id).unwrap();
    let first = &snapshot.messages[0];
    assert_eq!(
        first.payload.openai.as_ref().unwrap().content,
        "改过的问题",
        "当前分支上是新问法"
    );
    assert_ne!(first.id, user_msg, "新问法是另一条消息，旧的没被改写");
    assert_eq!(
        fx.hub.branches(&fx.session_id).unwrap().len(),
        2,
        "旧问法连同它的回答成为并列分支"
    );
}

#[tokio::test]
async fn only_user_messages_can_be_edited() {
    let fx = fixture();
    fx.say("你好");
    run_once(&fx).await;
    let answer = fx
        .hub
        .snapshot(&fx.session_id)
        .unwrap()
        .messages
        .last()
        .unwrap()
        .id;

    let err = fx
        .hub
        .edit_and_resend(&fx.session_id, answer, "我来替你说")
        .unwrap_err();
    assert!(matches!(err, lya_hub::HubError::Invalid(_)), "{err}");
}

#[tokio::test]
async fn tree_edits_are_refused_while_a_turn_runs() {
    let fx = fixture();
    fx.say("你好");
    fx.hub.start_turn(&fx.session_id).unwrap();
    tokio::time::sleep(Duration::from_millis(80)).await;

    // 一边跑一边改树会让两者抢着往同一棵树上追加
    assert!(matches!(
        fx.hub.regenerate(&fx.session_id).unwrap_err(),
        lya_hub::HubError::Busy(_)
    ));
    assert!(matches!(
        fx.hub.switch_branch(&fx.session_id, 1).unwrap_err(),
        lya_hub::HubError::Busy(_)
    ));
    assert!(matches!(
        fx.hub.delete_message(&fx.session_id, 1).unwrap_err(),
        lya_hub::HubError::Busy(_)
    ));

    fx.stop.store(true, Ordering::Relaxed);
}

#[tokio::test]
async fn deleting_a_non_leaf_is_refused() {
    let fx = fixture();
    fx.say("你好");
    run_once(&fx).await;

    let path = fx.hub.snapshot(&fx.session_id).unwrap().messages;
    let user_msg = path[0].id;
    let answer = path.last().unwrap().id;

    // 用户消息底下还挂着回答，删了会把子树变成孤儿
    assert!(fx.hub.delete_message(&fx.session_id, user_msg).is_err());
    fx.hub.delete_message(&fx.session_id, answer).unwrap();
    assert_eq!(
        fx.hub.snapshot(&fx.session_id).unwrap().messages.len(),
        1,
        "删掉叶子后回到用户消息"
    );
}

#[tokio::test]
async fn regenerate_needs_a_user_message() {
    let fx = fixture();
    assert!(matches!(
        fx.hub.regenerate(&fx.session_id).unwrap_err(),
        lya_hub::HubError::Invalid(_)
    ));
}

#[tokio::test]
async fn tree_exposes_every_branch_not_just_the_active_path() {
    let fx = fixture();
    fx.say("你好");
    run_once(&fx).await;
    arm(&fx);
    fx.hub.regenerate(&fx.session_id).unwrap();
    settle(&fx).await;

    let tree = fx.hub.tree(&fx.session_id).unwrap();
    assert_eq!(tree.leaves.len(), 2);
    assert_eq!(tree.nodes.len(), 3, "一条用户消息 + 两条并列回答");
    assert!(tree.active_leaf_id.is_some());

    // 父子关系画得出分叉图：两条回答挂在同一个父节点下
    let user = tree.nodes.iter().find(|n| n.parent_id.is_none()).unwrap();
    let children: Vec<_> = tree
        .nodes
        .iter()
        .filter(|n| n.parent_id == Some(user.id))
        .collect();
    assert_eq!(children.len(), 2);

    // 每个节点自带那一步的全部信息，追踪不需要另建一套记录
    assert!(children[0].payload.openai.is_some());
    assert!(children[0].created_at >= user.created_at);
}

#[tokio::test]
async fn session_without_custom_tools_follows_the_global_default() {
    let dir = tempfile::tempdir().unwrap();
    let db = Arc::new(
        Db::open(dir.path().join("lya.db"))
            .unwrap()
            .with_migrations(lya_session::MIGRATION_SCOPE, lya_session::MIGRATIONS)
            .with_migrations(lya_memory::MIGRATION_SCOPE, lya_memory::MIGRATIONS),
    );
    db.migrate().unwrap();
    let sessions = Arc::new(SessionStore::with_db(Arc::clone(&db)));
    let memory = Arc::new(MemoryStore::with_db(db));

    let agent = Agent::new(AgentParts {
        backend: SlowBackend {
            stop: Arc::new(AtomicBool::new(true)),
        },
        endpoints: vec![LlmEndpoint::new("https://example.invalid/v1", "k")],
        default_model: "default".into(),
        sessions: Arc::clone(&sessions),
        memory,
        tools: Arc::new(ToolRegistry::new()),
        actions: Arc::new(ActionRegistry::new()),
        prompt: PromptBuilder::new(),
        max_tool_rounds: 4,
        max_parallel_tools: 3,
        default_enabled_tools: Some(vec!["file_read".into()]),
    })
    .unwrap();

    let meta = sessions.create_session(CreateSession::default()).unwrap();
    assert_eq!(
        agent.effective_tools(&meta),
        Some(vec!["file_read".to_string()]),
        "没自定义过的会话应当跟随全局，而不是一律全开"
    );

    // 会话自己定了就以会话为准
    sessions
        .set_enabled_tools(&meta.id, Some(&["bash".to_string()]))
        .unwrap();
    let meta = sessions.get_session(&meta.id).unwrap().unwrap();
    assert_eq!(agent.effective_tools(&meta), Some(vec!["bash".to_string()]));
}

#[tokio::test]
async fn global_events_reach_their_own_subscribers() {
    let fx = fixture();
    let mut rx = fx.hub.subscribe_global();

    fx.hub
        .broadcast_global("config_changed", serde_json::json!({ "file": "runtime" }));

    let event = tokio::time::timeout(Duration::from_secs(1), rx.recv())
        .await
        .expect("应当很快收到")
        .unwrap();
    assert_eq!(event.scope, "global", "全局事件不属于任何会话");
    assert_eq!(event.kind, "config_changed");
    assert_eq!(event.payload["file"], "runtime");
}

#[tokio::test]
async fn unknown_session_is_reported() {
    let fx = fixture();
    assert!(matches!(
        fx.hub.start_turn("nope").unwrap_err(),
        lya_hub::HubError::NotFound(_)
    ));
    assert!(matches!(
        fx.hub.snapshot("nope").unwrap_err(),
        lya_hub::HubError::NotFound(_)
    ));
}
