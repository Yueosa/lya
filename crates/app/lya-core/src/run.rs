//! 启动 HTTP 服务：配置、数据库、Agent、监听端口。
//!
//! `lya` 二进制与 `serve` 示例共用这一套组装逻辑。

use std::sync::Arc;
use std::sync::atomic::{AtomicU16, Ordering};
use std::time::Duration;

use lya_action::{ActionRegistry, register_builtins as register_actions};
use lya_agent::{Agent, AgentParts, TurnSettings};
use lya_config::Config;
use lya_db::Db;
use lya_http::{HttpClient, HttpConfig};
use lya_llm::{LlmClient, LlmEndpoint};
use lya_base::Live;
use lya_memory::{IndexBudget, MemoryStore};
use lya_prompt::PromptBuilder;
use lya_session::SessionStore;
use lya_tool::tools::web::SelfPort;
use lya_tool::{ToolRegistry, register_builtins as register_tools};
use thiserror::Error;
use tokio::sync::{oneshot, watch};
use tokio::task::JoinHandle;

use lya_api::router;
use lya_hub::SessionHub;

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

    let db = Db::open(config.db_path())?;
    db.migrate()?;
    let db = Arc::new(db);
    let sessions = Arc::new(SessionStore::with_db(Arc::clone(&db)));

    match sessions.mark_stale_streaming() {
        Ok(0) => {}
        Ok(n) => eprintln!("清理了 {n} 条上次没写完的消息"),
        Err(err) => eprintln!("清理残留失败：{err}"),
    }
    // 这三个句柄是「配置改了立刻生效」的全部着力点：装配处留一份，
    // 下面的 reload 钩子往里推新值，持有方下次读就拿到新的
    let budget = Live::new(index_budget(&config.runtime.memory));
    let confirm = Live::new(shell_policy(config.runtime.shell.confirm));

    let memory = Arc::new(MemoryStore::with_db(db).with_budget(budget.clone()));

    let self_port: SelfPort = Arc::new(AtomicU16::new(0));

    let mut tools = ToolRegistry::new();
    register_tools(
        &mut tools,
        http.clone(),
        confirm.clone(),
        Arc::clone(&self_port),
    )?;
    let mut actions = ActionRegistry::new();
    register_actions(&mut actions, Arc::clone(&memory))?;

    let settings = turn_settings(&config)?;
    let agent = Arc::new(Agent::new(AgentParts {
        backend: LlmClient::new(http.clone()),
        endpoints: llm_endpoints_from_config(&config),
        default_model: settings.default_model.clone(),
        sessions,
        memory,
        tools: Arc::new(tools),
        actions: Arc::new(actions),
        prompt: settings.prompt.clone(),
        max_tool_rounds: settings.max_tool_rounds,
        max_consecutive_tool_failures: settings.max_consecutive_tool_failures,
        max_parallel_tools: settings.max_parallel_tools,
        default_enabled_tools: settings.default_enabled_tools.clone(),
    })?);

    let hub = SessionHub::new(Arc::clone(&agent), http, Arc::clone(&self_port));

    // 配置写入后由 HTTP 层按这个按钮。放在这里是因为「读配置」和「认识每个组件」
    // 只有装配处同时具备——hub 不该为了重载而去依赖 lya-config
    let reload_dir = dir.clone();
    hub.set_reload(move || {
        let config = Config::load_from(&reload_dir).map_err(|err| err.to_string())?;
        // 先算出全部新值再逐个推：中途因为某个值不合法而失败的话，
        // 前面几个已经生效、后面几个还是旧的，那种半生效状态最难查
        let settings = turn_settings(&config).map_err(|err| err.to_string())?;
        let next_budget = index_budget(&config.runtime.memory);
        let next_confirm = shell_policy(config.runtime.shell.confirm);

        agent.apply_settings(settings).map_err(|err| err.to_string())?;
        budget.set(next_budget);
        confirm.set(next_confirm);
        Ok(())
    });

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

fn llm_endpoints_from_config(config: &Config) -> Vec<LlmEndpoint> {
    config
        .models
        .models
        .iter()
        .map(|entry| {
            let mut ep = LlmEndpoint::new(&entry.base_url, &entry.api_key).with_id(&entry.id);
            for (key, mode_cfg) in &entry.modes {
                if let Some(mode) = lya_config::ApiMode::parse(key) {
                    let llm_mode = match mode {
                        lya_config::ApiMode::Completions => lya_llm::ApiMode::Completions,
                        lya_config::ApiMode::Responses => lya_llm::ApiMode::Responses,
                    };
                    ep = ep.with_mode_params(llm_mode, mode_cfg.params.clone());
                    ep = ep.with_mode_capabilities(llm_mode, mode_cfg.capabilities.clone());
                }
            }
            ep
        })
        .collect()
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

fn index_budget(memory: &lya_config::MemorySettings) -> IndexBudget {
    IndexBudget {
        max_entries: memory.max_index_entries,
        max_chars: memory.max_index_chars,
        summary_chars: memory.index_summary_chars,
    }
}

/// 由配置推出 agent 的每轮设置。
///
/// 启动装配与后续重载**都走这里**。两条路各写一份映射，迟早会长出「重启之后的
/// 行为和刚改完配置时不一样」这种没人想查的 bug。
fn turn_settings(config: &Config) -> Result<TurnSettings, RunError> {
    let mut prompt = PromptBuilder::new();
    // 空人设在 Config 里已经被规整成 None，这里不必再判一次空串：
    // 那会走成 `with_persona("")`，语义是「本轮完全不要人设段」，不是「用内置默认」
    if let Some(persona) = &config.persona {
        prompt = prompt.with_persona(persona.clone());
    }
    let default_model = config
        .default_model()
        .ok_or_else(|| RunError::NotReady("models.toml 里没有任何模型".into()))?
        .id
        .clone();
    Ok(TurnSettings {
        default_model,
        prompt,
        max_tool_rounds: config.runtime.agent.max_tool_rounds,
        max_parallel_tools: config.runtime.agent.max_parallel_tools,
        max_consecutive_tool_failures: config.runtime.agent.max_consecutive_tool_failures,
        default_enabled_tools: config.runtime.tools.enabled.clone(),
    })
}
