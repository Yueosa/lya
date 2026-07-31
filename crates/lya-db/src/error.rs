//! 数据库错误。

use std::io;

/// `lya-db` 可返回的错误。
#[derive(Debug, thiserror::Error)]
pub enum DbError {
    /// SQLite 底层错误。
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    /// 文件系统错误（创建数据目录等）。
    #[error("io error at {path}: {source}")]
    Io {
        /// 出错的路径。
        path: String,
        /// 底层 IO 错误。
        #[source]
        source: io::Error,
    },

    /// 数据目录 / 库路径无法确定。
    #[error("data path error: {0}")]
    Path(String),

    /// 连接锁被 poison（持锁线程 panic 过）。
    #[error("db lock poisoned")]
    LockPoisoned,
}
