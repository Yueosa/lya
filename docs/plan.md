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
| 模型清单 | `models.toml` | id、密钥、`modes.*`（capabilities + params）、context_window | 模型页只读 + 手改 |
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

**完整规格：** [`responses-api.md`](./responses-api.md)（Phase 0–6，按顺序做，避免漏项）

前提：**全局 api 模式不影响已有会话**；模型能力在 **`models.toml`**，不在 runtime。

#### Phase 1 — 配置与会话
- [x] **破坏性**重写 `models.toml`：`modes.*` 唯一入口；删顶层 `capabilities` / 扁平 params；**无旧格式兼容**
- [x] 会话 DB `api_mode`；默认 **`completions`**；创建可显式选 responses；PATCH 不可改
- [x] create / patch `model_id` 校验栈；patch `api_mode` → 400
- [x] 前端：api_mode 默认 completions；模型 Picker 按栈过滤 + 后端硬拒绝

#### Phase 2 — lya-llm Responses 通路（先无原生搜索）
- [x] `POST /responses` + Responses SSE → `StreamEvent`
- [x] `ChatBackend` 按 `api_mode` 分派
- [x] `build_responses_input`（最小：user/assistant/tool 轮次）

#### Phase 3 — 原生联网 + UI
- [x] 请求注入 `{type:web_search}`；**关闭** DDG `web_search` tool；保留 `web_fetch`
- [x] `WebSearchStatus` → `AgentEvent` → LyaSSE `provider_search`
- [x] timeline 新块 + ChatStatusBar「正在搜索…」
- [x] prompt：responses 会话说明原生搜索

#### Phase 4 — 持久化与回放
- [x] 落库 `web_search_call` items；刷新后可展示
- [x] 历史 search items 回灌 Responses `input`

#### Phase 5–6 — 文档与收尾
- [x] ModelsView / ToolsView 只读说明
- [x] 双栈 README；`plan.md` P1 全部勾选

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

- [x] 记忆 slot 编号 + 索引 #1 置顶（2026-08-03）
- [x] Markdown 有序列表 Zpix 修复（2026-08-03）
- [x] Responses 双栈完整设计稿 `docs/responses-api.md`（2026-08-03）

细节不在这里重复；git log + archive 可查。

---

## 维护规则

1. 新功能先进本文件 P0–P3，做完打 `[x]` 加日期。
2. 不要新建 `TODO-xxx.md`。
3. `roadmap.md` / `lya-user-0-todo.md` 只保留指向本页的说明。
4. 改配置层级时先改 `lya-config/src/lib.rs` 注释，再改本文「配置体系」表。
