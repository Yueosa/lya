//! 记忆错误。

use lya_db::DbError;

/// `lya-memory` 错误。
#[derive(Debug, thiserror::Error)]
pub enum MemoryError {
    /// 底层数据库错误。
    #[error(transparent)]
    Db(#[from] DbError),

    /// 记忆不存在。
    #[error("memory not found: {0}")]
    NotFound(i64),

    /// 标题已被别的记忆占用。
    ///
    /// 想覆盖同名记忆请用 [`crate::MemoryStore::upsert_by_title`]。
    #[error("memory title already exists: {0}")]
    DuplicateTitle(String),

    /// 字段为空或超出长度上限。
    #[error("{0}")]
    Invalid(String),
}

impl From<rusqlite::Error> for MemoryError {
    fn from(err: rusqlite::Error) -> Self {
        Self::Db(DbError::Sqlite(err))
    }
}
