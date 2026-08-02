//! [`SessionStore`]：会话 CRUD 与消息树操作。
//!
//! 所有写操作都跑在 [`lya_db::Db::write`] 的事务里，读操作走 [`lya_db::Db::read`]。
//! 一次调用 = 一个事务，调用方不需要自己管理原子性。

mod helpers;
mod hitl;
mod meta;
mod tree;

#[cfg(test)]
mod tests;

use std::sync::Arc;

use lya_db::Db;

use crate::{MIGRATION_SCOPE, MIGRATIONS};

/// 会话存储：会话元数据 + 消息树。
pub struct SessionStore {
    /// 共享数据库句柄。
    pub(crate) db: Arc<Db>,
}

impl SessionStore {
    /// 用已打开的 [`Db`] 构造，并把 session 迁移登记进去。
    pub fn new(db: Db) -> Self {
        Self {
            db: Arc::new(db.with_migrations(MIGRATION_SCOPE, MIGRATIONS)),
        }
    }

    /// 复用别处已经建好的 [`Db`]。
    pub fn with_db(db: Arc<Db>) -> Self {
        Self { db }
    }

    /// 打开默认库 `~/.lya/lya.db` 并立即迁移。
    pub fn open_default() -> Result<Self, crate::SessionError> {
        let store = Self::new(Db::open_default()?);
        store.migrate()?;
        Ok(store)
    }

    /// 打开指定库文件并立即迁移。
    pub fn open(path: impl AsRef<std::path::Path>) -> Result<Self, crate::SessionError> {
        let store = Self::new(Db::open(path)?);
        store.migrate()?;
        Ok(store)
    }

    /// 执行已登记的迁移。
    pub fn migrate(&self) -> Result<(), crate::SessionError> {
        self.db.migrate()?;
        Ok(())
    }

    /// 底层数据库，供共享同一文件的其它领域 crate 复用。
    pub fn db(&self) -> &Db {
        &self.db
    }
}
