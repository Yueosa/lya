//! 启动 HTTP 服务：配置、数据库、Agent、监听端口。
//!
//! `lya` 二进制与 `serve` 示例共用这一套组装逻辑。

use std::sync::Arc;
use std::sync::atomic::{AtomicU16, Ordering};
use std::time::Duration;

use lya_action::{ActionRegistry, register_builtins as register_actions};
use lya_agent::{Agent, AgentParts};
use lya_config::Config;
use lya_db::Db;
use lya_http::{HttpClient, HttpConfig};
use lya_llm::{LlmClient, LlmEndpoint};
use lya_memory::MemoryStore;
use lya_prompt::PromptBuilder;
use lya_session::SessionStore;
use lya_tool::tools::web::SelfPort;
use lya_tool::{ToolRegistry, register_builtins as register_tools};
use thiserror::Error;
use tokio::sync::{oneshot, watch};
use tokio::task::JoinHandle;

use crate::{SessionHub, router};

/// 启动失败。
#[derive(Debug, Error)]
pub enum RunError {
    /// 数据目录或配置读写失败。
    #[error("{0}")]
    Config(#[from] lya_config::ConfigError),
    /// 数据库或迁移失败。
    #[error("{0}")]
    Db(#[from] lya_db::DbError),
    /// HTTP 客户端初始化失败。
    #[error("{0}")]
    Http(#[from] lya_http::HttpError),
    /// Agent 组装失败。
    #[error("{0}")]
    Agent(#[from] lya_agent::AgentError),
    /// 工具注册失败。
    #[error("{0}")]
    Tool(#[from] lya_tool::ToolError),
    /// 动作注册失败。
    #[error("{0}")]
    Action(#[from] lya_action::ActionError),
    /// 配置尚未就绪（缺 api_key 等）。
    #[error("配置还没准备好：{0}")]
    NotReady(String),
    /// 所有候选端口都被占用。
    #[error("所有候选端口都被占用了")]
    PortBusy,
    /// 服务运行中出错。
    #[error("{0}")]
    Serve(String),
}

/// 已启动的服务句柄。
pub struct ServerHandle {
    port: u16,
    shutdown_tx: watch::Sender<()>,
    task: JoinHandle<Result<(), RunError>>,
}

impl ServerHandle {
    /// 实际监听的端口（可能与配置默认不同）。
    pub fn port(&self) -> u16 {
        self.port
    }

    /// 请求停止并等待后台任务结束。
    pub async fn shutdown(self) -> Result<(), RunError> {
        let _ = self.shutdown_tx.send(());
        self.task
            .await
            .map_err(|err| RunError::Serve(err.to_string()))??;
        Ok(())
    }

    /// 只发停止信号，不等待。
    pub fn stop(&self) {
        let _ = self.shutdown_tx.send(());
    }

    /// 阻塞直到服务结束。
    pub async fn wait(self) -> Result<(), RunError> {
        self.shutdown().await
    }
}

/// 组装 App 并在后台 `axum::serve`，返回句柄与端口。
pub async fn start() -> Result<ServerHandle, RunError> {
    let (shutdown_tx, shutdown_rx) = watch::channel(());
    let (ready_tx, ready_rx) = oneshot::channel();

    let task = tokio::spawn(async move {
        let result = serve(shutdown_rx, ready_tx).await;
        if let Err(err) = &result {
            eprintln!("HTTP 服务异常退出：{err}");
        }
        result
    });

    let port = ready_rx
        .await
        .map_err(|_| RunError::Serve("服务启动任务提前退出".into()))??;

    Ok(ServerHandle {
        port,
        shutdown_tx,
        task,
    })
}

async fn serve(
    shutdown_rx: watch::Receiver<()>,
    ready_tx: oneshot::Sender<Result<u16, RunError>>,
) -> Result<(), RunError> {
    let dir = lya_config::data_root()?;
    for path in Config::init_missing(&dir)? {
        eprintln!("已生成配置模板：{}", path.display());
    }
    let config = Config::load_from(&dir)?;
    if let Err(err) = config.check_ready() {
        let _ = ready_tx.send(Err(RunError::NotReady(err.to_string())));
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
        .with_migrations(lya_session::MIGRATION_SCOPE, lya_session::MIGRATIONS)
        .with_migrations(lya_memory::MIGRATION_SCOPE, lya_memory::MIGRATIONS);
    db.migrate()?;
    let db = Arc::new(db);
    let sessions = Arc::new(SessionStore::with_db(Arc::clone(&db)));

    match sessions.mark_stale_streaming() {
        Ok(0) => {}
        Ok(n) => eprintln!("清理了 {n} 条上次没写完的消息"),
        Err(err) => eprintln!("清理残留失败：{err}"),
    }
    let memory = Arc::new(MemoryStore::with_db(db));

    let self_port: SelfPort = Arc::new(AtomicU16::new(0));

    let mut tools = ToolRegistry::new();
    register_tools(
        &mut tools,
        http.clone(),
        shell_policy(config.runtime.shell.confirm),
        Arc::clone(&self_port),
    )?;
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
        backend: LlmClient::new(http.clone()),
        endpoints,
        default_model,
        sessions,
        memory,
        tools: Arc::new(tools),
        actions: Arc::new(actions),
        prompt,
        max_tool_rounds: config.runtime.agent.max_tool_rounds,
        max_parallel_tools: config.runtime.agent.max_parallel_tools,
        default_enabled_tools: config.runtime.tools.enabled.clone(),
    })?);

    let hub = SessionHub::new(agent, http, Arc::clone(&self_port));
    let app = router(hub);

    let mut listener = None;
    let mut bound_port = 0u16;
    for port in config.core.server.candidate_ports() {
        let addr = format!("{}:{port}", config.core.server.host);
        match tokio::net::TcpListener::bind(&addr).await {
            Ok(bound) => {
                eprintln!("监听 http://{addr}");
                self_port.store(port, Ordering::Relaxed);
                bound_port = port;
                listener = Some(bound);
                break;
            }
            Err(_) => continue,
        }
    }
    let listener = match listener {
        Some(listener) => listener,
        None => {
            let _ = ready_tx.send(Err(RunError::PortBusy));
            return Ok(());
        }
    };

    if ready_tx.send(Ok(bound_port)).is_err() {
        return Ok(());
    }

    let mut shutdown_rx = shutdown_rx.clone();
    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            let _ = shutdown_rx.changed().await;
        })
        .await
        .map_err(|err| RunError::Serve(err.to_string()))?;

    Ok(())
}

fn shell_policy(confirm: lya_config::ShellConfirm) -> lya_tool::tools::shell::ConfirmPolicy {
    use lya_config::ShellConfirm;
    use lya_tool::tools::shell::ConfirmPolicy;
    match confirm {
        ShellConfirm::Always => ConfirmPolicy::Always,
        ShellConfirm::Unknown => ConfirmPolicy::Unknown,
        ShellConfirm::Risky => ConfirmPolicy::Risky,
    }
}
