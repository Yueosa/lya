# lya-db

SQLite 基建：数据目录、连接、全库 schema 与写事务。

## 职责

- 默认库 `~/.lya/lya.db`（数据根来自 `lya-base`）
- WAL、`foreign_keys`
- 持有 `migrations/` 下的全库 schema，启动时把没跑过的补上
- 串行化读写封装（单进程 agent 够用）

## 非职责

- 不做 ORM，不封装查询构造器；领域 crate 直接写 SQL

## 用法

```rust
use lya_db::Db;

// 打开即带全库 schema，装配方不需要知道有哪些表
let db = Db::open(path)?;
db.migrate()?;
```

测试里要一个建好表的临时库，开 `testing` feature：

```rust
let (_dir, db) = lya_db::testing::open_test_db();
```

## 改库

加 `migrations/NNN_xxx.sql` 并挂进 `SCHEMA`，**不要改已有文件**——已建过库的机器
跳过旧文件，改了只会让新旧两条路走到不同终点。
