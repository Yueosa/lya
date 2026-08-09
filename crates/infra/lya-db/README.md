# lya-db

SQLite 基建：数据目录、连接、全库 schema 与写事务。

## 职责

- 默认库 `~/.lya/lya.db`（数据根来自 `lya-base`）
- WAL、`foreign_keys`
- 持有 `migrations/000_init.sql` 全库 schema，新库 migrate 一次到位
- 串行化读写封装（单进程 agent 够用）

## 非职责

- 不做 ORM，不封装查询构造器；领域 crate 直接写 SQL

## 用法

```rust
use lya_db::Db;

let db = Db::open(path)?;
db.migrate()?;
```

测试里要一个建好表的临时库，开 `testing` feature：

```rust
let (_dir, db) = lya_db::testing::open_test_db();
```

## 改库

1. **直接改** `migrations/000_init.sql`（新用户一步建全表）
2. **同步改** `scripts/upgrade-existing-lya-db.sql`（老用户手动 `sqlite3` 执行）
3. **不要**往 `SCHEMA` 追加 version 1、2… 的增量迁移
