//! lya — 本地 agent：HTTP 服务 + 系统托盘。

#[cfg(target_os = "linux")]
mod notify;

#[cfg(target_os = "linux")]
mod tray;

fn main() {
    if let Err(err) = run() {
        eprintln!("{err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        tray::run()
    }

    #[cfg(not(target_os = "linux"))]
    {
        run_headless()
    }
}

#[cfg(not(target_os = "linux"))]
fn run_headless() -> Result<(), String> {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|err| err.to_string())?;
    rt.block_on(async {
        let handle = lya_core::start_server()
            .await
            .map_err(|err| err.to_string())?;
        eprintln!("监听 http://127.0.0.1:{}/", handle.port());
        eprintln!("按 Ctrl+C 退出");
        tokio::signal::ctrl_c()
            .await
            .map_err(|err| err.to_string())?;
        handle.shutdown().await.map_err(|err| err.to_string())
    })
}
