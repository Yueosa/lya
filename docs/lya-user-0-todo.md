# 开发待办

活文档。完成项打 `[x]`，封存项见文末。

## Wave A — 安全与测试 Bug（已完成）

- [x] **bash**：双引号内 `$()` / 反引号 → 强制未完全理解，走确认
- [x] **bash**：`steps[]` 只改确认框展示；`needs_confirm` 决策**只**看 `parse(command)`
- [x] **图片路径**：Markdown 尖括号路径、`decodeURIComponent` 边界 + 测试
- [x] **新会话模型名**：`model_id` 为空时 Composer 显示配置默认模型名
- [x] **分支树 filter**：四个「隐藏」项默认全开（`filters.v2`）

## Wave B — 体验（已完成）

- [x] **img_cache + lightbox**：`~/.lya/sessions/{id}/img_cache/`；本地/远程图缓存端点；放大 / 复制图片 / 复制路径 / 保存
- [x] **会话显示偏好**：折叠长块阈值（滑块+输入）；流式结束后自动收起思考/工具
- [x] **models 配置**：模板与 UI 暴露 `max_tokens` 等透传字段示例
- [x] **加载性能**：首屏抑制 stagger、尾部优先渲染、`content-visibility`

## Wave C — 重构（已完成）

- [x] 前端 god-file 拆层：`useChat.ts` → `app/chat/*` 模块 + barrel
- [x] 前端子组件化：`ChatView` → `views/chat/*`（Header、Timeline、Scroll、Drawer…）
- [x] 后端 crate README（14 个 crate 均有 README，对齐 `docs/style.md`）

## Wave D — 调用组 + 通知（进行中）

设计见 [`docs/tool-batch.md`](./tool-batch.md)。

1. [x] **TIME_ANCHOR 澄清**：user=发送时刻，tool=结果落库时刻（≈执行结束）
2. [x] **tool 调用组（后端）**：去 `already_awaiting` stub；auto 并行；多 HITL；批后并行 execute；`max_parallel_tools=3`
3. [x] **tool 调用组（协议/前端）**：`tool_batch_started` SSE；组卡片折叠；HITL `‹ i/n ›`
4. [ ] **notify-send**：completed / hitl / failed / max_rounds；托盘图标；HITL 按 message_id 去重

## Backlog（Wave D 之后）

| # | 项 |
|---|-----|
| 5 | vdo_cache / ado_cache |
| 6 | 每 tool/action 配置页 |
| 7 | lianclaw 迁移 |
| 8 | **上下文管理器**：`lya-token` + `lya-context` |

| 项 | 说明 |
|----|------|
| web_fetch 翻页 | [x] |
| 二进制体积 | 已做 slim 构建 |
| models 字段 | `context_window` / `max_tokens` |

## 封存（不做 / 不追踪）

- 配置写回保留占位 `api_key`（前端无编辑页）
- 图片 URL token 改造
- bash 沙箱
- Action cancel
- 流式落库策略改动
- HITL 长命令终端弹窗（近期）
- HITL 确认超时自动拒绝（永久等人；见 tool-batch）
- embedding / 自动召回
- 配置文件 watcher
- 全局 SSE（会话级够用）
- 分支命名 / `branch_meta`
- 递归删整枝（`delete_leaf` 够用）
- 图片上传、会话导出、快捷键、移动端
- notify 前台抑制（暂不做）
