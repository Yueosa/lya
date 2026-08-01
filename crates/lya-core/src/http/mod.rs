//! HTTP 接口。
//!
//! 分工照 TMP.md 定的来：**写操作走 REST，只改后端真相；结果一律通过订阅推给
//! 前端。** 所以发消息返回 202 就走了，正文从 SSE 出来——这样同一个会话在网页和
//! 手机上看到的是同一份流，而不是各自请求各自的响应。

pub mod guard;
mod sessions;

use std::sync::Arc;

use axum::Router;
use axum::routing::{get, post};

use crate::hub::SessionHub;

/// 组装路由。
pub fn router(hub: Arc<SessionHub>) -> Router {
    Router::new()
        .route("/api/sessions", get(sessions::list).post(sessions::create))
        .route(
            "/api/sessions/{id}",
            get(sessions::snapshot).patch(sessions::patch),
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
        .route("/api/sessions/{id}/subscribe", get(sessions::subscribe))
        .layer(axum::middleware::from_fn(guard::same_origin))
        .with_state(hub)
}
