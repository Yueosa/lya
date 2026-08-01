//! 会话错误。

use lya_db::DbError;

/// `lya-session` 错误。
#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    /// 底层数据库错误。
    #[error(transparent)]
    Db(#[from] DbError),

    /// 会话不存在。
    #[error("session not found: {0}")]
    NotFound(String),

    /// 消息不存在或不属于该会话。
    #[error("message not found: {0}")]
    MessageNotFound(i64),

    /// 试图删除仍有子节点的消息。
    #[error("message {0} is not a leaf; only leaf nodes can be deleted")]
    NotLeaf(i64),

    /// 当前叶是未决 HITL，拒绝裸追加普通用户消息等。
    #[error("session blocked by pending HITL on message {0}")]
    PendingHitl(i64),

    /// 会话已归档，只能回看不能再写。
    #[error("session {0} is archived and read-only")]
    Archived(String),

    /// JSON 编解码失败。
    #[error("message json error: {0}")]
    Json(#[from] serde_json::Error),

    /// 非法参数。
    #[error("{0}")]
    Invalid(String),
}

impl From<rusqlite::Error> for SessionError {
    fn from(err: rusqlite::Error) -> Self {
        Self::Db(DbError::Sqlite(err))
    }
}
