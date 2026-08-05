# lya-session

会话与消息树（SQLite 持久化）。

## 职责

- 会话元数据：`work_mode`、`enabled_tools`、`persona`、`active_leaf_id`
- 消息树：`append` / `switch_leaf` / `fork_at` / `delete_leaf`
- HITL 独立节点与 pending 查询
- `message_json`：自研外壳 + OpenAI 结构

## 分支约定

- 当前路径由 `active_leaf_id` 决定
- 编辑重发 = `fork_at` 后再 `append`
- 只允许删除无子节点的叶节点

## 用法

```rust
use lya_session::{SessionStore, CreateSession, MessagePayload};

let store = SessionStore::with_db(db);
let meta = store.create_session(CreateSession::default())?;
store.append(&meta.id, MessagePayload::user_text("你好"), false)?;
```
