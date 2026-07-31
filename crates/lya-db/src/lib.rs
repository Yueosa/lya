//! # lya-db
//!
//! SQLite 基建层：数据目录、连接、迁移与写事务。
//!
//! ## 职责
//!
//! - 解析数据根 `~/.lya`，默认库文件 `lya.db`
//! - 打开连接并设置 WAL / `foreign_keys`
//! - 执行各领域 crate 注册进来的迁移 SQL（要求语句幂等）
//! - 提供串行化的读写封装，避免多线程交叉写
//!
//! ## 非职责
//!
//! - 不认识 sessions / messages / memory 等业务表；表结构由领域 crate 自带 SQL
//! - 不做 ORM，也不封装查询构造器；领域 crate 直接写 SQL
//!
//! ## 并发模型
//!
//! 内部一把 [`Mutex<Connection>`]，读写都经过它。本地单进程 agent 的写入量很小，
//! 这样最简单也最不容易出错；将来若需要并发读，可在此替换为连接池而不影响调用方。

#![deny(missing_docs)]

mod error;
mod paths;

pub use error::DbError;
pub use paths::{data_root, default_db_path};

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use rusqlite::{Connection, TransactionBehavior};

/// 一个已打开的 SQLite 数据库。
pub struct Db {
    /// 库文件路径。
    path: PathBuf,
    /// 常驻连接，读写共用。
    conn: Mutex<Connection>,
    /// 已注册的迁移 SQL，按注册顺序执行。
    migrations: Vec<&'static str>,
}

impl Db {
    /// 打开默认库：`~/.lya/lya.db`。
    pub fn open_default() -> Result<Self, DbError> {
        Self::open(default_db_path()?)
    }

    /// 打开指定库文件；父目录不存在会自动创建。
    pub fn open(path: impl AsRef<Path>) -> Result<Self, DbError> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|source| DbError::Io {
                path: parent.display().to_string(),
                source,
            })?;
        }

        let conn = Connection::open(&path)?;
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA foreign_keys = ON;",
        )?;

        Ok(Self {
            path,
            conn: Mutex::new(conn),
            migrations: Vec::new(),
        })
    }

    /// 注册一段迁移 SQL，通常来自领域 crate 的 `include_str!`。
    ///
    /// SQL 必须幂等（`CREATE TABLE IF NOT EXISTS` 等），因为每次
    /// [`Db::migrate`] 都会重跑全部脚本。
    pub fn with_migration(mut self, sql: &'static str) -> Self {
        self.migrations.push(sql);
        self
    }

    /// 库文件路径。
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// 依次执行所有已注册迁移。
    pub fn migrate(&self) -> Result<(), DbError> {
        let conn = self.conn.lock().map_err(|_| DbError::LockPoisoned)?;
        for sql in &self.migrations {
            conn.execute_batch(sql)?;
        }
        Ok(())
    }

    /// 在 IMMEDIATE 写事务中执行闭包：`Ok` 提交，`Err` 回滚。
    ///
    /// 闭包错误类型由调用方决定，只要能从 [`DbError`] 转换即可，
    /// 这样领域 crate 可以直接返回自己的错误类型。
    pub fn write<T, E>(&self, f: impl FnOnce(&Connection) -> Result<T, E>) -> Result<T, E>
    where
        E: From<DbError>,
    {
        let mut conn = self.conn.lock().map_err(|_| DbError::LockPoisoned)?;
        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(DbError::from)?;
        let value = f(&tx)?;
        tx.commit().map_err(DbError::from)?;
        Ok(value)
    }

    /// 只读访问；与写共用同一把锁，因此不会读到未提交的中间状态。
    pub fn read<T, E>(&self, f: impl FnOnce(&Connection) -> Result<T, E>) -> Result<T, E>
    where
        E: From<DbError>,
    {
        let conn = self.conn.lock().map_err(|_| DbError::LockPoisoned)?;
        f(&conn)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SQL: &str = "CREATE TABLE IF NOT EXISTS t (id INTEGER PRIMARY KEY, v TEXT NOT NULL);";

    #[test]
    fn migrate_is_idempotent_and_transaction_commits() {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open(dir.path().join("t.db"))
            .unwrap()
            .with_migration(SQL);
        db.migrate().unwrap();
        db.migrate().unwrap();

        db.write::<_, DbError>(|conn| {
            conn.execute("INSERT INTO t (v) VALUES (?1)", ["hi"])?;
            Ok(())
        })
        .unwrap();

        let count: i64 = db
            .read::<_, DbError>(|conn| {
                Ok(conn.query_row("SELECT COUNT(*) FROM t", [], |row| row.get(0))?)
            })
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn failed_transaction_rolls_back() {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open(dir.path().join("t.db"))
            .unwrap()
            .with_migration(SQL);
        db.migrate().unwrap();

        let result = db.write::<(), DbError>(|conn| {
            conn.execute("INSERT INTO t (v) VALUES (?1)", ["boom"])?;
            Err(DbError::Path("intentional".into()))
        });
        assert!(result.is_err());

        let count: i64 = db
            .read::<_, DbError>(|conn| {
                Ok(conn.query_row("SELECT COUNT(*) FROM t", [], |row| row.get(0))?)
            })
            .unwrap();
        assert_eq!(count, 0);
    }
}
