# lya 计划（活文档）

**唯一待办入口。** 完成一项就改 `[x]` 并写日期；不要另起炉灶写第三份清单。

参考文档（非待办）：

| 文档 | 用途 |
|------|------|
| [`architecture.md`](./architecture.md) | crate 边界 |
| [`tool-batch.md`](./tool-batch.md) | 调用组协议 |
| [`media-cache.md`](./media-cache.md) | 媒体缓存路径 |
| [`style.md`](./style.md) | 前端风格 |
| [`lya.md`](./lya.md) | 产品说明 |
| [`archive/`](./archive/) | 历史计划，只查不改 |

---

## 配置体系（现状，以代码为准）

数据目录：`~/.lya/`。

| 层级 | 存储 | 内容 | 谁改 |
|------|------|------|------|
| 进程 | `core.toml` | 端口、DB 路径、HTTP 超时、日志 | 设置页 / 手改；**改完重启** |
| 全局默认 | `runtime.toml` | 新会话默认：agent、tools.enabled、shell、memory、media | 设置 → **默认配置**；热加载 |
| 模型清单 | `models.toml` | id、密钥、capabilities、context_window、透传 params | 模型页只读 + 手改 |
| 全局人设 | `persona.toml` | 人设正文 | 设置 → 人设 |
| **会话** | **`lya.db` → `sessions` 表** | work_mode、model_id、persona、enabled_tools_json | 聊天侧栏会话设置 |

**没有** `~/.lya/session/config/`。会话级配置在数据库；全局默认只在 `runtime.toml`。

### 前端页职责（不要混）

| 页面 | 职责 |
|------|------|
| **设置 → 默认配置** | 编辑 `runtime.toml`（对话 / 工具默认 / 记忆 / 媒体） |
| **设置 → 人设 / 存储 / 原始文件** | 同上文件体系 |
| **工具** | **只读** catalog：参数 schema、prompt_hint、内置限制；pill 显示全局默认状态 |
| **模型** | **只读** `models.toml` |
| **聊天 → 会话设置（侧栏）** | 本会话 tool 开关、action 只读、**浏览器**显示偏好 |

### 工具数值

- **不可配置**：上限/默认值在 `crates/lya-tool/src/limits.rs`
- **可配置**：新会话默认启用哪些 tool → `runtime.toml` `[tools].enabled`
- 前端只读表：`web/src/utils/toolLimits.ts`（改 limits.rs 时同步）

---

## 当前优先（按顺序）

### P0 — 配置与 UI 信息架构

- [x] 工具数值抽到 `lya-tool/src/limits.rs`（2026-08-03）
- [x] 设置「运行时」→「默认配置」，分块；工具默认从 ToolsView 迁入（2026-08-03）
- [x] ToolsView 只做 catalog 只读（2026-08-03）
- [x] **会话设置扩面板**：合并详情+设置 →「会话」侧栏 Tab（概览/工具/会话/显示）（2026-08-03）
- [ ] **默认配置**再拆 UI 子导航（对话 / 工具 / 记忆 / 媒体），与 `runtime.toml` 段落一一对应
- [ ] `runtime.toml` 文件拆分 — **不做**

### P1 — DeepSeek Responses / 原生联网

前提：**全局 api 模式不影响已有会话**；模型能力在 **`models.toml`**，不在 runtime。

- [ ] `models.toml` 扩展：`api_modes`、`capabilities` 按模式（Flash：completions=`[text]`，responses=`[text, web_search]`）
- [ ] 会话创建时锁定 `api_mode`（DB 新列或 meta JSON）
- [ ] `lya-llm`：Responses 客户端 + SSE → 统一 `StreamEvent` 适配
- [ ] 原生 web：请求里带 provider `web_search`；**关闭** DDG `web_search` tool；保留 `web_fetch`
- [ ] timeline **新块**：原生搜索 in_progress / completed（非 `call_started(web_search)`）
- [ ] LyaSSE：`provider_status`（或等价）→ ChatStatusBar「正在搜索…」
- [ ] 文档：Responses 与 chat/completions 双栈说明

### P2 — 上下文管理器（暂缓实施，先占位）

- [ ] `lya-token` + `lya-context`（见 [`lya-user-0-todo.md`](./lya-user-0-todo.md) 封存前条目）
- [ ] 消费 `models.context_window`；按会话 `api_mode` 选 assembler

### P3 — 体验债

- [ ] 首页悬浮词：随机 spawn + 淡入淡出（已做一版，待用户验收）
- [ ] SessionSettings 与 `usePrefs` 分区标注（「本机显示」vs「本会话」）

---

## 刻意不做

| 项 | 原因 |
|----|------|
| 工具参数进 TOML | 已定：只读 + `limits.rs` |
| tool/action Phase 2 数值配置 | 同上 |
| 配置 watcher | 手动 reload 够用 |
| `~/.lya/session/config/` 文件化 | 除非有明确迁移方案；当前 DB 够用 |

完整封存列表见 [`lya-user-0-todo.md`](./lya-user-0-todo.md)。

---

## 已完成（摘要）

Wave A–F、调用组、crate 拆分、媒体 Phase 1、notify、web_fetch 翻页、tool 全局启用 UI、chat UX 一批修复（2026-08 提交 `8d9a145` 等）。

细节不在这里重复；git log + archive 可查。

---

## 维护规则

1. 新功能先进本文件 P0–P3，做完打 `[x]` 加日期。
2. 不要新建 `TODO-xxx.md`。
3. `roadmap.md` / `lya-user-0-todo.md` 只保留指向本页的说明。
4. 改配置层级时先改 `lya-config/src/lib.rs` 注释，再改本文「配置体系」表。
