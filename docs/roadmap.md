# lya 路线图

当前仓库的**活文档**。历史计划见 [`docs/archive/`](./archive/)。crate 边界见 [`docs/architecture.md`](./architecture.md)。

## 当前优先

Backlog 见 [`lya-user-0-todo.md`](./lya-user-0-todo.md)（tool/action 配置页、vdo/ado 等）。

## 已完成（近期）

- 后端骨架、WebUI、托盘、Wave A–C
- img_cache、二进制 slim、`web_fetch` 翻页、`context_window` 配置
- **Wave D**：tool 调用组（后端 + SSE + 前端）
- **Wave E**：架构拆分 + `[media.*]` + 存储扇形图
- **Wave F**：托盘 `notify-send`（completed / hitl / failed / max_rounds）

## 之后

1. vdo/ado（media + tool）
2. lianclaw 迁移
3. 上下文管理器（`lya-token` + `lya-context`）

## 刻意不做 / 封存

见 [`lya-user-0-todo.md`](./lya-user-0-todo.md)「封存」一节。
