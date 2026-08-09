//! # lya-db
//!
//! SQLite 基建层：数据目录、连接、迁移与写事务。
//!
//! ## 职责
//!
//! - 定位默认库文件 `lya.db`（数据根由 `lya-base` 给出，这里不重算）
//! - 打开连接并设置 WAL / `foreign_keys`
//! - **持有全库 schema** 并在启动时把还没跑过的迁移执行掉
//! - 提供串行化的读写封装，避免多线程交叉写
//!
//! ## 非职责
//!
//! - 不做 ORM，也不封装查询构造器；领域 crate 直接写 SQL
//!
//! ## schema 为什么在这里而不在领域 crate
//!
//! 一个库文件、一份 schema。分散到领域 crate 的代价是**没人编排**：装配方得记着
//! 逐个注册，漏一个就拿到半个库——只调 `SessionStore::open` 的测试和工具就是这样，
//! 建出来的库里没有 memory 表。放在这里之后，打开库就等于拿到完整 schema。
//!
//! 改库：**直接改** `migrations/000_init.sql`，并同步 `scripts/upgrade-existing-lya-db.sql`
//! 给已有库手动升级。不要往 [`SCHEMA`] 里追加 version 1、2…——新用户应一步建全表。
//!
//! ## 并发模型
//!
//! 内部一把 [`Mutex<Connection>`]，读写都经过它。本地单进程 agent 的写入量很小，
//! 这样最简单也最不容易出错；将来若需要并发读，可在此替换为连接池而不影响调用方。

#![deny(missing_docs)]

mod error;
mod paths;
#[cfg(feature = "testing")]
pub mod testing;

pub use error::DbError;

/// 当前时间的 RFC 3339 表示，用于迁移台账。
fn now() -> String {
    chrono::Utc::now().to_rfc3339()
}
pub use paths::{data_root, default_db_path};

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use rusqlite::{Connection, OptionalExtension, TransactionBehavior};

/// 一步迁移。
///
/// 每一步跑过就记在案，不会重跑，所以 SQL **不需要**幂等——`ALTER TABLE`
/// 这种没法写成「不存在才执行」的语句因此也能用。
#[derive(Debug, Clone, Copy)]
pub struct Migration {
    /// 版本号，同一 scope 内递增。初始 schema 是 `0`。
    pub version: u32,
    /// 这一步要执行的 SQL。
    pub sql: &'static str,
}

/// 全库 schema 在迁移台账里的归属名。
pub const SCHEMA_SCOPE: &str = "lya";

/// 全库 schema。新版本追加一项，已有项不动。
pub const SCHEMA: &[Migration] = &[Migration {
    version: 0,
    sql: include_str!("../migrations/000_init.sql"),
}];

/// 记录已执行迁移的表。
///
/// 用 `(scope, version)` 而不是 `PRAGMA user_version`：全库 schema 走 `lya`
/// 这个 scope，别处若要往同一个库文件里加自己的表（如将来的插件），可以另起
/// scope 各记各的版本，不必和主线协调编号。
const LEDGER: &str = "CREATE TABLE IF NOT EXISTS _migrations (
    scope       TEXT    NOT NULL,
    version     INTEGER NOT NULL,
    applied_at  TEXT    NOT NULL,
    PRIMARY KEY (scope, version)
);";

/// 一个已打开的 SQLite 数据库。
pub struct Db {
    /// 库文件路径。
    path: PathBuf,
    /// 常驻连接，读写共用。
    conn: Mutex<Connection>,
    /// 已注册的迁移，按 scope 分组。
    migrations: Vec<(&'static str, &'static [Migration])>,
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
             PRAGMA foreign_keys = ON;
             -- 正常情况下写入都被内部那把锁串起来了，这个超时是给「同一库文件
             -- 被另开连接」的情况兜底，免得直接吃 SQLITE_BUSY
             PRAGMA busy_timeout = 5000;",
        )?;

        Ok(Self {
            path,
            conn: Mutex::new(conn),
            // 打开就带上全库 schema：调用方不需要知道有哪些表，也就不可能漏注册
            migrations: vec![(SCHEMA_SCOPE, SCHEMA)],
        })
    }

    /// 追加一套迁移序列。
    ///
    /// `scope` 用来和别的序列各记各的版本。步骤按 `version` 从小到大执行，
    /// 跑过的不再跑。正常不需要调——全库 schema 打开时就带上了。
    pub fn with_migrations(mut self, scope: &'static str, steps: &'static [Migration]) -> Self {
        self.migrations.push((scope, steps));
        self
    }

    /// 库文件路径。
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// 执行所有还没跑过的迁移。
    ///
    /// 每一步单独一个事务：中途某步失败，之前成功的仍然算数，重跑时从失败那步
    /// 继续。整体一个大事务的话，一次失败会把已经对的部分也回滚掉，反而更难修。
    pub fn migrate(&self) -> Result<(), DbError> {
        let mut conn = self.conn.lock().map_err(|_| DbError::LockPoisoned)?;
        conn.execute_batch(LEDGER)?;

        for (scope, steps) in &self.migrations {
            let mut sorted: Vec<&Migration> = steps.iter().collect();
            sorted.sort_by_key(|step| step.version);

            for step in sorted {
                let done: bool = conn
                    .query_row(
                        "SELECT 1 FROM _migrations WHERE scope = ?1 AND version = ?2",
                        rusqlite::params![scope, step.version],
                        |_| Ok(true),
                    )
                    .optional()?
                    .unwrap_or(false);
                if done {
                    continue;
                }

                let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
                tx.execute_batch(step.sql)?;
                tx.execute(
                    "INSERT INTO _migrations (scope, version, applied_at) VALUES (?1, ?2, ?3)",
                    rusqlite::params![scope, step.version, now()],
                )?;
                tx.commit()?;
            }
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

    const V1: &[Migration] = &[Migration {
        version: 1,
        sql: "CREATE TABLE IF NOT EXISTS t (id INTEGER PRIMARY KEY, v TEXT NOT NULL);",
    }];

    /// 第二步给已有的表加一列——`CREATE TABLE IF NOT EXISTS` 做不到这件事，
    /// 这正是要版本化迁移的原因。
    const V2: &[Migration] = &[
        V1[0],
        Migration {
            version: 2,
            sql: "ALTER TABLE t ADD COLUMN note TEXT;",
        },
    ];

    fn column_names(db: &Db) -> Vec<String> {
        db.read::<_, DbError>(|conn| {
            let mut stmt = conn.prepare("PRAGMA table_info(t)")?;
            let names = stmt
                .query_map([], |row| row.get::<_, String>(1))?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(names)
        })
        .unwrap()
    }

    /// 打开 + migrate 就该拿到完整的库，不需要调用方再注册什么。
    #[test]
    fn fresh_database_has_the_whole_schema() {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open(dir.path().join("t.db")).unwrap();
        db.migrate().unwrap();

        let tables: Vec<String> = db
            .read::<_, DbError>(|conn| {
                let mut stmt = conn
                    .prepare("SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name")?;
                let names = stmt
                    .query_map([], |row| row.get::<_, String>(0))?
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(names)
            })
            .unwrap();

        for expected in [
            "memories",
            "memory_tags",
            "messages",
            "model_templates",
            "sessions",
            "tokenizers",
        ] {
            assert!(
                tables.iter().any(|t| t == expected),
                "缺 {expected}：{tables:?}"
            );
        }
        assert!(
            !tables.iter().any(|t| t == "branch_meta"),
            "branch_meta 已经删了"
        );
    }

    #[test]
    fn existing_tables_can_gain_columns() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("t.db");

        // 先按旧版本建库并写点数据，模拟「用户机器上已经有一个库了」
        let old = Db::open(&path).unwrap().with_migrations("demo", V1);
        old.migrate().unwrap();
        old.write::<_, DbError>(|conn| {
            conn.execute("INSERT INTO t (v) VALUES (?1)", ["旧数据"])?;
            Ok(())
        })
        .unwrap();
        assert_eq!(column_names(&old), ["id", "v"]);
        drop(old);

        // 换成新版本再打开：应当补上新列，且旧数据还在
        let upgraded = Db::open(&path).unwrap().with_migrations("demo", V2);
        upgraded.migrate().unwrap();
        assert_eq!(column_names(&upgraded), ["id", "v", "note"]);

        let kept: i64 = upgraded
            .read::<_, DbError>(|conn| {
                Ok(conn.query_row("SELECT COUNT(*) FROM t", [], |r| r.get(0))?)
            })
            .unwrap();
        assert_eq!(kept, 1, "升级不该丢数据");
    }

    #[test]
    fn each_step_runs_at_most_once() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("t.db");
        let db = Db::open(&path).unwrap().with_migrations("demo", V2);

        db.migrate().unwrap();
        // ALTER 不幂等，重跑会报 duplicate column——跑第二遍还能过，
        // 说明台账真的起了作用
        db.migrate().unwrap();
        assert_eq!(column_names(&db), ["id", "v", "note"]);
    }

    #[test]
    fn scopes_keep_separate_version_counters() {
        let dir = tempfile::tempdir().unwrap();
        // 两个领域共用一个库文件，各自都有 v1，互不干扰
        let db = Db::open(dir.path().join("t.db"))
            .unwrap()
            .with_migrations("demo", V1)
            .with_migrations(
                "other",
                &[Migration {
                    version: 1,
                    sql: "CREATE TABLE IF NOT EXISTS u (id INTEGER PRIMARY KEY);",
                }],
            );
        db.migrate().unwrap();

        let tables: i64 = db
            .read::<_, DbError>(|conn| {
                Ok(conn.query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE name IN ('t', 'u')",
                    [],
                    |row| row.get(0),
                )?)
            })
            .unwrap();
        assert_eq!(tables, 2);
    }

    #[test]
    fn migrate_is_idempotent_and_transaction_commits() {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open(dir.path().join("t.db"))
            .unwrap()
            .with_migrations("demo", V1);
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
            .with_migrations("demo", V1);
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
