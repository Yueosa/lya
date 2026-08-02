//! HTTP 接口。
//!
//! 分工照 TMP.md 定的来：**写操作走 REST，只改后端真相；结果一律通过订阅推给
//! 前端。** 所以发消息返回 202 就走了，正文从 SSE 出来——这样同一个会话在网页和
//! 手机上看到的是同一份流，而不是各自请求各自的响应。

mod config;
pub mod guard;
mod image;
mod media;
mod media_limits;
mod introspect;
mod memories;
mod sessions;
mod static_ui;
mod storage;

use std::sync::Arc;

use axum::Router;
use axum::routing::{get, post};

use lya_hub::SessionHub;

/// 组装路由。
pub fn router(hub: Arc<SessionHub>) -> Router {
    Router::new()
        .route("/api/sessions", get(sessions::list).post(sessions::create))
        .route("/api/sessions/archived", get(sessions::archived))
        .route(
            "/api/sessions/{id}",
            get(sessions::snapshot).patch(sessions::patch).delete(sessions::remove),
        )
        .route("/api/sessions/{id}/messages", post(sessions::send))
        .route(
            "/api/sessions/{id}/messages/{message_id}",
            post(sessions::edit_message).delete(sessions::delete_message),
        )
        .route(
            "/api/sessions/{id}/branches",
            get(sessions::branches).post(sessions::switch_branch),
        )
        .route("/api/sessions/{id}/regenerate", post(sessions::regenerate))
        .route("/api/sessions/{id}/stop", post(sessions::stop))
        .route("/api/sessions/{id}/hitl", post(sessions::hitl))
        .route("/api/sessions/{id}/tree", get(sessions::tree))
        .route("/api/sessions/{id}/subscribe", get(sessions::subscribe))
        .route(
            "/api/sessions/{id}/tools/{tool}",
            axum::routing::put(introspect::toggle_tool),
        )
        // 白盒：模型手里有什么，用户看得见
        .route("/api/tools", get(introspect::tools))
        .route("/api/actions", get(introspect::actions))
        // 配置：core 只读，其余可写；写入会广播 global 事件
        .route("/api/bootstrap", get(config::bootstrap))
        .route("/api/config", get(config::read))
        .route(
            "/api/config/runtime",
            axum::routing::put(config::write_runtime),
        )
        .route(
            "/api/config/persona",
            axum::routing::put(config::write_persona),
        )
        .route("/api/config/raw/{file}", get(config::raw))
        .route("/api/models", get(config::models))
        .route("/api/models/probe", post(config::probe))
        // 数据目录占用（只读）
        .route("/api/storage/stats", get(storage::stats))
        // 本地图片：家目录内 + 令牌校验
        .route("/api/local-image", get(image::local_image))
        // 会话媒体缓存（img_cache）
        .route(
            "/api/sessions/{id}/media/image",
            get(media::session_image),
        )
        // 全局事件：配置变更，以后还有桌面通知、会话列表变化
        .route("/api/events", get(sessions::subscribe_global))
        // 记忆：模型只能读写，删除只走这里
        .route("/api/memories", get(memories::list).post(memories::create))
        .route("/api/memories/search", get(memories::search))
        .route(
            "/api/memories/{id}",
            get(memories::read)
                .patch(memories::update)
                .delete(memories::delete),
        )
        .fallback(static_ui::serve_ui)
        .layer(axum::middleware::from_fn(guard::same_origin))
        .with_state(hub)
}
