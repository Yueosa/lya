//! Linux 系统托盘：WebUI / 退出。

use std::sync::LazyLock;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};

use image::ImageFormat;
use ksni::menu::{MenuItem, StandardItem};
use ksni::{Icon, ToolTip, Tray};
use ksni::blocking::TrayMethods;

static TRAY_ICON: LazyLock<Icon> = LazyLock::new(load_tray_icon);

pub fn run() -> Result<(), String> {
    let (port_tx, port_rx) = mpsc::sync_channel(1);
    let (stop_tx, stop_rx) = mpsc::channel::<()>();
    let http = spawn_http_server(port_tx, stop_rx)?;

    let port = port_rx
        .recv()
        .map_err(|_| "HTTP 服务线程启动失败".to_string())??;

    let (exit_tx, exit_rx) = mpsc::channel();
    let tray = LyaTray {
        port,
        exit_tx,
    };
    let tray_handle = tray
        .spawn()
        .map_err(|err| format!("无法启动系统托盘：{err}"))?;

    wait_for_exit(exit_rx)?;

    tray_handle.shutdown().wait();
    let _ = stop_tx.send(());
    http.join()
        .map_err(|_| "HTTP 服务线程异常退出".to_string())?
        .map_err(|err| err.to_string())?;
    Ok(())
}

fn spawn_http_server(
    port_tx: mpsc::SyncSender<Result<u16, String>>,
    stop_rx: Receiver<()>,
) -> Result<JoinHandle<Result<(), String>>, String> {
    thread::Builder::new()
        .name("lya-http".into())
        .spawn(move || {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .thread_name("lya-http-worker")
                .build()
                .map_err(|err| err.to_string())?;

            rt.block_on(async {
                let handle = lya_core::start_server()
                    .await
                    .map_err(|err| err.to_string())?;
                let port = handle.port();
                port_tx.send(Ok(port)).map_err(|_| "主线程已退出".to_string())?;

                let _ = stop_rx.recv();
                handle.shutdown().await.map_err(|err| err.to_string())
            })
        })
        .map_err(|err| err.to_string())
}

fn wait_for_exit(exit_rx: Receiver<()>) -> Result<(), String> {
    exit_rx
        .recv()
        .map_err(|_| "托盘线程意外退出".to_string())
}

struct LyaTray {
    port: u16,
    exit_tx: Sender<()>,
}

impl Tray for LyaTray {
    fn id(&self) -> String {
        "lya".into()
    }

    fn title(&self) -> String {
        format!("lya · :{}", self.port)
    }

    fn tool_tip(&self) -> ToolTip {
        ToolTip {
            title: "lya".into(),
            description: format!("http://127.0.0.1:{}/", self.port),
            ..Default::default()
        }
    }

    fn icon_pixmap(&self) -> Vec<Icon> {
        vec![TRAY_ICON.clone()]
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        vec![
            StandardItem {
                label: "WebUI".into(),
                activate: Box::new(|tray: &mut Self| open_webui(tray.port)),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "退出".into(),
                activate: Box::new(|tray: &mut Self| {
                    let _ = tray.exit_tx.send(());
                }),
                ..Default::default()
            }
            .into(),
        ]
    }
}

fn open_webui(port: u16) {
    let url = format!("http://127.0.0.1:{port}/");
    // `open::that` 会等 launcher 退出，部分浏览器会挂成 lya 的子进程——lya 退出时
    // 整实例一起被杀。`that_detached` 用 setsid 脱离进程组。
    if let Err(err) = open::that_detached(&url) {
        eprintln!("打不开 WebUI ({url})：{err}");
    }
}

fn load_tray_icon() -> Icon {
    let bytes = include_bytes!("../../../web/public/icon.png");
    let image = image::load_from_memory_with_format(bytes, ImageFormat::Png)
        .expect("icon.png 应能解码");
    let rgba = image.into_rgba8();
    let (width, height) = rgba.dimensions();
    rgba_to_icon(rgba.into_raw(), width, height)
}

fn rgba_to_icon(data: Vec<u8>, width: u32, height: u32) -> Icon {
    let mut argb = data;
    for pixel in argb.chunks_exact_mut(4) {
        pixel.rotate_right(1);
    }
    Icon {
        width: width as i32,
        height: height as i32,
        data: argb,
    }
}
