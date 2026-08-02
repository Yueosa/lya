# lya-db

SQLite 基建：数据目录、连接、迁移与写事务。

## 职责

- 数据根 `~/.lya`，默认库 `lya.db`
- WAL、`foreign_keys`、各领域 crate 注册的迁移 SQL
- 串行化读写封装（单进程 agent 够用）

## 非职责

- 不认识 sessions / memory 等业务表；表结构由领域 crate 自带 SQL

## 用法

```rust
use lya_db::Db;

let db = Db::open(path)?
    .with_migrations("session", lya_session::MIGRATIONS);
db.migrate()?;
```
