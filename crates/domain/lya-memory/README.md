# lya-memory

跨会话长期记忆：显式笔记仓储 + 常驻索引渲染。

## 职责

- 记忆 CRUD，正文存 SQLite
- 标签规整与写入长度校验
- 渲染索引段供 `lya-prompt` 注入

## 设计

当前量级下把索引（标题 + 标签 + 摘要）整段放进 system prompt，比向量检索更准；超 [`IndexBudget`] 时只保留最近条目。

## 用法

```rust
use lya_memory::MemoryStore;

let memory = MemoryStore::with_db(db);
memory.create(title, body, tags)?;
```
