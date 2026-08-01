//! 最小命令行对话：把整条链路真的跑一遍。
//!
//! ```bash
//! cargo run -p lya-agent --example chat
//! ```
//!
//! 首次运行会在 `~/.lya/` 生成配置模板，填好 `models.toml` 里的 api_key 再跑。
//! 输入 `/quit` 退出，`/mode ask|edit|agent` 切模式，`/new` 开新会话。

use std::io::{self, BufRead, Write};
use std::sync::Arc;
use std::time::Duration;

use futures_util::StreamExt;
use lya_action::{ActionRegistry, register_builtins as register_actions};
use lya_agent::{Agent, AgentEvent, AgentParts, CancelToken, TurnEndReason};
use lya_config::Config;
use lya_db::Db;
use lya_http::{HttpClient, HttpConfig};
use lya_llm::{LlmClient, LlmEndpoint};
use lya_memory::MemoryStore;
use lya_mode::Mode;
use lya_prompt::PromptBuilder;
use lya_session::{CreateSession, MessagePayload, SessionStore};
use lya_tool::{ToolRegistry, register_builtins as register_tools};

/// 配置里的确认策略映射到工具层。`lya-config` 刻意不依赖 `lya-tool`，
/// 所以这层映射由装配方负责。
fn shell_policy(confirm: lya_config::ShellConfirm) -> lya_tool::tools::shell::ConfirmPolicy {
    use lya_config::ShellConfirm;
    use lya_tool::tools::shell::ConfirmPolicy;
    match confirm {
        ShellConfirm::Always => ConfirmPolicy::Always,
        ShellConfirm::Unknown => ConfirmPolicy::Unknown,
        ShellConfirm::Risky => ConfirmPolicy::Risky,
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // ── 配置 ────────────────────────────────────────────────
    let dir = lya_config::data_root()?;
    for path in Config::init_missing(&dir)? {
        println!("已生成配置模板：{}", path.display());
    }
    let config = Config::load_from(&dir)?;
    if let Err(err) = config.check_ready() {
        eprintln!("配置还没准备好：{err}");
        return Ok(());
    }
    let model = config.default_model().expect("check_ready 已确认有模型");
    println!("模型：{} ({})", model.name, model.id);

    // ── 基建 ────────────────────────────────────────────────
    // config 只给朴素数值，映射到各模块的配置类型是装配方的活，也就这几行
    let http_settings = &config.core.http;
    let http = HttpClient::new(&HttpConfig {
        timeout: Some(Duration::from_secs(http_settings.timeout_secs)),
        connect_timeout: Some(Duration::from_secs(http_settings.connect_timeout_secs)),
        pool_idle_timeout: Some(Duration::from_secs(http_settings.pool_idle_timeout_secs)),
        pool_max_idle_per_host: http_settings.pool_max_idle_per_host,
        user_agent: http_settings.user_agent.clone(),
        ..Default::default()
    })?;
    // 整份清单都交给 agent，会话可以各自选
    let endpoints: Vec<LlmEndpoint> = config
        .models
        .models
        .iter()
        .map(|entry| {
            LlmEndpoint::new(&entry.base_url, &entry.api_key)
                .with_id(&entry.id)
                .with_params(entry.params.clone())
        })
        .collect();

    let db = Db::open(config.db_path())?
        .with_migration(lya_session::MIGRATION_SQL)
        .with_migration(lya_memory::MIGRATION_SQL);
    db.migrate()?;
    let db = Arc::new(db);

    let sessions = Arc::new(SessionStore::with_db(Arc::clone(&db)));
    let memory = Arc::new(MemoryStore::with_db(db));

    let mut tools = ToolRegistry::new();
    register_tools(&mut tools, http.clone(), shell_policy(config.runtime.shell.confirm))?;
    let mut actions = ActionRegistry::new();
    register_actions(&mut actions, Arc::clone(&memory))?;

    let mut prompt = PromptBuilder::new();
    if let Some(persona) = &config.persona {
        prompt = prompt.with_persona(persona.clone());
    }

    let agent = Agent::new(AgentParts {
        backend: LlmClient::new(http),
        endpoints,
        default_model: model.id.clone(),
        sessions: Arc::clone(&sessions),
        memory,
        tools: Arc::new(tools),
        actions: Arc::new(actions),
        prompt,
        max_tool_rounds: config.runtime.agent.max_tool_rounds,
        default_enabled_tools: config.runtime.tools.enabled.clone(),
    })?;

    // ── 会话 ────────────────────────────────────────────────
    let mut session_id = sessions
        .create_session(CreateSession {
            title: "命令行".into(),
            work_mode: config.runtime.agent.default_work_mode,
            ..Default::default()
        })?
        .id;
    println!(
        "会话 {session_id}（{} 模式）。/quit 退出，/mode 切模式，/new 开新会话。\n",
        config.runtime.agent.default_work_mode
    );

    // ── 主循环 ──────────────────────────────────────────────
    let stdin = io::stdin();
    loop {
        print!("你 > ");
        io::stdout().flush()?;
        let mut line = String::new();
        if stdin.lock().read_line(&mut line)? == 0 {
            break;
        }
        let line = line.trim();

        match line {
            "" => continue,
            "/quit" => break,
            "/new" => {
                session_id = sessions
                    .create_session(CreateSession {
                        title: "命令行".into(),
                        work_mode: config.runtime.agent.default_work_mode,
                        ..Default::default()
                    })?
                    .id;
                println!("新会话 {session_id}\n");
                continue;
            }
            _ if line.starts_with("/mode ") => {
                match line[6..].trim().parse::<Mode>() {
                    Ok(mode) => {
                        // 走 agent 的接口，会在树上留一条模式变更说明
                        agent.switch_mode(&session_id, mode)?;
                        println!("已切换到 {mode} 模式\n");
                    }
                    Err(err) => println!("{err}\n"),
                }
                continue;
            }
            _ => {}
        }

        sessions.append(&session_id, MessagePayload::user_text(line), false)?;

        print!("lya > ");
        io::stdout().flush()?;
        let stream = agent.run_turn(session_id.clone(), CancelToken::new());
        futures_util::pin_mut!(stream);
        let mut in_reasoning = false;

        while let Some(event) = stream.next().await {
            match event {
                AgentEvent::Reasoning(text) => {
                    if !in_reasoning {
                        print!("\n[思考] ");
                        in_reasoning = true;
                    }
                    print!("{text}");
                    io::stdout().flush()?;
                }
                AgentEvent::Delta(text) => {
                    if in_reasoning {
                        print!("\nlya > ");
                        in_reasoning = false;
                    }
                    print!("{text}");
                    io::stdout().flush()?;
                }
                AgentEvent::CallStarted { name, kind, .. } => {
                    println!("\n  [{kind:?}] {name} …");
                }
                AgentEvent::CallFinished { name, success, .. } => {
                    println!("  [{name}] {}", if success { "完成" } else { "失败" });
                    print!("lya > ");
                    io::stdout().flush()?;
                }
                AgentEvent::AwaitHuman { message_id } => {
                    println!("\n  需要你确认（消息 #{message_id}）——命令行版还没做交互，先跳过");
                }
                AgentEvent::TurnEnd { reason } => {
                    match reason {
                        TurnEndReason::Completed => println!(),
                        other => println!("\n[本轮结束：{other:?}]"),
                    }
                    println!();
                }
                AgentEvent::RoundStarted { .. } | AgentEvent::MessageCommitted { .. } => {}
            }
        }
    }

    Ok(())
}
