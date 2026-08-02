# 开发待办

活文档。完成项打 `[x]`，封存项见文末。crate 边界见 [`docs/architecture.md`](./architecture.md)。

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

## Wave C — 前端/文档（已完成）

- [x] 前端 god-file 拆层：`useChat.ts` → `app/chat/*` 模块 + barrel
- [x] 前端子组件化：`ChatView` → `views/chat/*`
- [x] 后端 crate README（14 个 crate 均有 README）

## Wave D — 调用组（已完成）

设计见 [`docs/tool-batch.md`](./tool-batch.md)。

1. [x] **TIME_ANCHOR 澄清**
2. [x] **tool 调用组（后端）**
3. [x] **tool 调用组（协议/前端）**

## Wave E — 架构重构 ✅

| 步骤 | 内容 | 状态 |
|------|------|------|
| E1 | 新建 `lya-hub`、`lya-api`、`lya-media`、`lya-storage` workspace 成员 | [x] |
| E2 | 迁 **lya-media**：`media_cache` + 图片 serving | [x] |
| E3 | 迁 **lya-hub**：`SessionHub` + `event` | [x] |
| E4 | 迁 **lya-api**：全部 `http/*`、`guard`、`router` | [x] |
| E5 | **lya-core** 瘦身为 `run.rs` + `start_server`（无 re-export） | [x] |
| E6 | **lya-storage**：`scan_usage()` + `GET /api/storage/stats` | [x] |
| E7 | 配置 **`[media.*]`** + 前端 **Storage** 扇形图（只读） | [x] |

## Wave F — notify ✅

- [x] **notify-send**：completed / hitl / failed / max_rounds；托盘图标；HITL 按 `message_id` 去重

实现：`lya-hub` 广播 `notify_*` 全局事件；`lya` 托盘订阅 `/api/events` 后调 `notify-send`。

## Backlog（Wave F 之后）

| # | 项 |
|---|-----|
| 1 | 每 tool/action **配置页** |
| 2 | vdo/ado：**lya-media** 扩展 + 可选 **lya-tool** |
| 3 | lianclaw 迁移 |
| 4 | **上下文管理器**：`lya-token` + `lya-context` |

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
- HITL 确认超时自动拒绝
- embedding / 自动召回
- 配置文件 watcher
- 全局 SSE（会话级够用）
- 分支命名 / `branch_meta`
- 递归删整枝（`delete_leaf` 够用）
- 图片上传、会话导出、快捷键、移动端
- notify 前台抑制（暂不做）
- Storage 页「清除缓存」按钮（第一版不做）
