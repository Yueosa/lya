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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let handle = lya_core::start_server().await?;
    println!("按 Ctrl+C 退出");
    tokio::signal::ctrl_c().await?;
    handle.shutdown().await?;
    Ok(())
}
