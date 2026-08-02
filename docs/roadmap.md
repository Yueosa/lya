# lya 路线图

当前仓库的**活文档**。历史计划见 [`docs/archive/`](./archive/)。crate 边界见 [`docs/architecture.md`](./architecture.md)。

## 当前优先

Backlog 见 [`lya-user-0-todo.md`](./lya-user-0-todo.md)：

1. **上下文管理器**（`lya-token` + `lya-context`）
2. lianclaw 迁移

## 已完成（Wave A–F + 近期）

- 后端骨架、WebUI、托盘
- img_cache、二进制 slim、**web_fetch 翻页**（`start_line` / `end_line`）、`context_window` 配置
- **Wave D**：tool 调用组
- **Wave E**：crate 拆分、`[media.image]`、存储扇形图
- **Wave F**：托盘 `notify-send`（**够用，收工**）
- **tool 配置 UI Phase 1**（ToolsView + SessionSettings）
- **vdo/ado Phase 1**（提示词 + 缓存端点 + Markdown 原生播放器 + ConfigView + 路径条；无 tool）

## 之后

1. **上下文管理器**（`lya-token` + `lya-context`）
2. lianclaw 迁移
3. vdo/ado 专用 tool（仅在有需求时）

## 刻意不做 / 封存

见 [`lya-user-0-todo.md`](./lya-user-0-todo.md)「封存」一节。
