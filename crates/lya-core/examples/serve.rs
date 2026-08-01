//! 起一个 HTTP 服务，用 curl 就能把整条链路走一遍。
//!
//! ```bash
//! cargo run -p lya-core --example serve
//!
//! # 另开一个终端
//! curl -s localhost:51616/api/sessions -X POST -H 'content-type: application/json' -d '{}'
//! curl -N localhost:51616/api/sessions/<id>/subscribe &
//! curl -s localhost:51616/api/sessions/<id>/messages -X POST \
//!      -H 'content-type: application/json' -d '{"text":"你好"}'
//! ```

use std::sync::Arc;
use std::time::Duration;

use lya_action::{ActionRegistry, register_builtins as register_actions};
use lya_agent::{Agent, AgentParts};
use lya_config::Config;
use lya_core::{SessionHub, router};
use lya_db::Db;
use lya_http::{HttpClient, HttpConfig};
use lya_llm::{LlmClient, LlmEndpoint};
use lya_memory::MemoryStore;
use lya_prompt::PromptBuilder;
use lya_session::SessionStore;
use lya_tool::{ToolRegistry, register_builtins as register_tools};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = lya_config::data_root()?;
    for path in Config::init_missing(&dir)? {
        println!("已生成配置模板：{}", path.display());
    }
    let config = Config::load_from(&dir)?;
    if let Err(err) = config.check_ready() {
        eprintln!("配置还没准备好：{err}");
        return Ok(());
    }

    let http_settings = &config.core.http;
    let http = HttpClient::new(&HttpConfig {
        timeout: Some(Duration::from_secs(http_settings.timeout_secs)),
        connect_timeout: Some(Duration::from_secs(http_settings.connect_timeout_secs)),
        user_agent: http_settings.user_agent.clone(),
        ..Default::default()
    })?;

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
    let default_model = config
        .default_model()
        .expect("check_ready 已确认有模型")
        .id
        .clone();

    let agent = Arc::new(Agent::new(AgentParts {
        backend: LlmClient::new(http),
        endpoints,
        default_model,
        sessions,
        memory,
        tools: Arc::new(tools),
        actions: Arc::new(actions),
        prompt,
        max_tool_rounds: config.runtime.agent.max_tool_rounds,
    })?);

    let hub = SessionHub::new(agent);
    let app = router(hub);

    // 端口被占用就依次往后试，和配置里的 port_backoff_max 对应
    let mut listener = None;
    for port in config.core.server.candidate_ports() {
        let addr = format!("{}:{port}", config.core.server.host);
        match tokio::net::TcpListener::bind(&addr).await {
            Ok(bound) => {
                println!("监听 http://{addr}");
                listener = Some(bound);
                break;
            }
            Err(_) => continue,
        }
    }
    let listener = listener.ok_or("所有候选端口都被占用了")?;

    axum::serve(listener, app).await?;
    Ok(())
}

/// 配置里的确认策略映射到工具层。
fn shell_policy(confirm: lya_config::ShellConfirm) -> lya_tool::tools::shell::ConfirmPolicy {
    use lya_config::ShellConfirm;
    use lya_tool::tools::shell::ConfirmPolicy;
    match confirm {
        ShellConfirm::Always => ConfirmPolicy::Always,
        ShellConfirm::Unknown => ConfirmPolicy::Unknown,
        ShellConfirm::Risky => ConfirmPolicy::Risky,
    }
}
