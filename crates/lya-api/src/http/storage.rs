//! 数据目录占用统计（只读）。

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use lya_storage::{StorageError, UsageReport, scan_usage};

/// `GET /api/storage/stats`：返回 `~/.lya` 体积分项。
pub async fn stats() -> Result<Json<UsageReport>, Response> {
    scan_usage().map(Json).map_err(|err| match err {
        StorageError::Invalid(message) => (StatusCode::BAD_REQUEST, message).into_response(),
        StorageError::Io(err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response(),
    })
}
