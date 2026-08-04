# Responses API 双栈实施计划

**状态：** 设计稿（2026-08-03）  
**关联：** [`plan.md`](./plan.md) P1 · [`architecture.md`](./architecture.md)

本文是 P1「DeepSeek Responses / 原生联网」的**完整实施规格**。按文末 **Phase 0–6** 顺序落地；每 Phase 有验收标准，做完在 `plan.md` 打 `[x]`。

---

## 1. 目标与非目标

### 要做什么

| 目标 | 说明 |
|------|------|
| **双栈并存** | 同一会话生命周期内只用一种 API：`completions` 或 `responses` |
| **会话锁定 `api_mode`** | 创建时确定，之后不因全局默认或模型清单变更而自动切换 |
| **Responses 流式对话** | `POST /responses` + SSE → 现有 `StreamEvent` 管线 |
| **原生联网** | Responses 模式下请求体注入 `{"type":"web_search"}`，**关闭** DDG `web_search` tool |
| **保留 `web_fetch`** | 两种模式都保留；搜到 URL 后仍由本地 tool 拉正文 |
| **UI 可感知** | 状态栏「正在搜索…」、timeline 原生搜索块（不是 `call_started(web_search)`） |

### 刻意不做（本 Phase）

| 项 | 原因 |
|----|------|
| 会话中途切换 `api_mode` | 两种 API 的消息 wire 格式不同，切换等于换协议栈 |
| 把 `web_fetch` 换成 provider 侧 | 本地 SSRF 防护、翻页、缓存仍需要 |
| `lya-context` / token 预算 | P2，本 Phase 只预留 `api_mode` 分支点 |
| Codex `apply_patch` custom tool | DeepSeek 兼容项，lya 无需求 |
| 非 DeepSeek 的 Responses | 先做 DeepSeek；其它 vendor 以后按同一 adapter 扩展 |

---

## 2. 现状（2026-08-03）

```text
SessionMeta (model_id, work_mode)
    → Agent::run_turn
        → build_messages() → Vec<ChatMessage>
        → ChatBackend::chat_stream → LlmClient
            → POST {base}/chat/completions
        → ToolBundle (含 DDG web_search + web_fetch + …)
    → AgentEvent → LyaSSE → timeline (text | reasoning | tool | hitl)
```

**唯一 LLM 入口：** `lya-llm::LlmClient` → `/chat/completions`  
**唯一搜索路径：** `lya-tool::web_search`（DuckDuckGo HTML）  
**Session DB：** 无 `api_mode` 列  
**models.toml：** 单套 `capabilities`，无 per-mode 声明

---

## 3. 核心概念

### 3.1 `api_mode`

| 值 | HTTP | 工具策略 |
|----|------|----------|
| `completions` | `POST /v1/chat/completions` | 现有逻辑；DDG `web_search` 可用 |
| `responses` | `POST /v1/responses` | 请求体带 `web_search`；**不**注册 DDG `web_search` |

枚举名与配置、DB、前端 wire 保持一致，字符串 `"completions"` / `"responses"`。

### 3.2 会话锁定与默认栈

**默认栈：`completions`。** 不读模型级 default，也不读 runtime 开关——新会话未显式指定时一律 `completions`。用户要 Responses 须在**创建会话**时主动选。

1. **新建会话**时写入 `api_mode`：
   - 显式传入（API / 前端）→ 须被当前所选 `model_id` 支持；
   - 未传 → **`completions`**。
2. **已有会话** PATCH **拒绝**改 `api_mode`（无例外）。
3. **换模型**（`model_id` PATCH）时：
   - 新模型须支持会话**已锁定**的 `api_mode`；
   - 不支持 → **400**，提示新建会话；
   - **不**静默降级、不自动换栈。

### 3.3 前后端双重约束（防误选）

前端负责**筛选项**，后端负责**硬拒绝**——不依赖其中一层。

| 场景 | 前端 | 后端 |
|------|------|------|
| 创建会话选 `api_mode` | 切换栈后，模型列表只显示支持该栈的条目 | create 校验 `model_id` + `api_mode` |
| 已有会话换模型 | Picker 只列出支持当前 `api_mode` 的模型 | patch `model_id` 校验 |
| PATCH 改 `api_mode` | 不提供控件 | 400 |

实现要点（Phase 1.4）：`GET /api/models` 或 bootstrap 下发 `modes` 元数据；`SessionMetaTab` 的 model Picker **按 `api_mode` 过滤**，避免用户看到切过去会 400 的模型。

### 3.4 能力（capabilities）分层

| 层 | 含义 | 例子 |
|----|------|------|
| **模型能力** | 这个模型在某种 `api_mode` 下能干什么 | responses + `web_search` |
| **模式权限** | ask/edit/agent 能否用某 tool | 现有 `lya-mode` |
| **会话启用** | `enabled_tools_json` | 现有逻辑 |
| **全局默认** | `runtime.toml` `[tools].enabled` | 现有逻辑 |

Responses 原生搜索**不是 lya-tool**，不走 `enabled_tools`；是否启用由「`api_mode == responses` 且模型 capabilities 含 `web_search`」决定。

---

## 4. 配置：`models.toml`（破坏性改版）

> **不做旧格式兼容。** 顶层 `capabilities`、扁平透传字段（`model` / `max_tokens` / `thinking` 等）全部删除；loader **不**回填、**不**静默降级。旧 `models.toml` 加载失败时给出明确错误，用户按模板重写。

### 4.1 砍掉什么

| 删除项 | 替代 |
|--------|------|
| 顶层 `capabilities = ["text"]` | 仅存在于 `[models.modes.*].capabilities` |
| 顶层 `model` / `max_tokens` / `reasoning_effort` / `thinking` … | 仅存在于 `[models.modes.*].params` |
| 顶层 `params` flatten 进请求 | 按会话 `api_mode` 取对应 mode 的 `params` |
| 模型级 `default_api_mode` | 全局默认恒为 `completions`；会话创建时用户显式选 responses |
| `ModelEntry.params` + `serde(flatten)` | `ModelEntry.modes: BTreeMap<ApiMode, ModeConfig>` |

### 4.2 新 schema（唯一合法形态）

```toml
[[models]]
id = "deepseek-v4-flash"
name = "DeepSeek V4 Flash"
base_url = "https://api.deepseek.com"
api_key = "…"
context_window = 1048576

[models.modes.completions]
capabilities = ["text"]
params = {
  model = "deepseek-v4-flash",
  max_tokens = 8192,
  reasoning_effort = "high",
  thinking = { type = "enabled" },
}

[models.modes.responses]
capabilities = ["text", "web_search"]
params = {
  model = "deepseek-v4-flash",
  max_output_tokens = 8192,
  reasoning = { effort = "high" },
}

[[models]]
id = "deepseek-v4-pro"
name = "DeepSeek V4 Pro"
base_url = "https://api.deepseek.com"
api_key = "…"
context_window = 1048576

[models.modes.completions]
capabilities = ["text"]
params = { model = "deepseek-v4-pro", max_tokens = 8192, reasoning_effort = "high", thinking = { type = "enabled" } }
# 不写 modes.responses → 该模型不可用于 responses 会话
```

**固定字段（每条 model）：** `id` · `name` · `base_url` · `api_key` · `context_window` · **`modes`（至少一个）**

**每个 mode：** `capabilities`（非空，必含 `text`）· `params`（整表透传进该 API 的请求体）

### 4.3 透传规则

- 发请求时**只**合并 `modes[session.api_mode].params`。
- `context_window` 仍是 lya 元数据，**永不**进 API body。
- completions 与 responses 字段名不同（`max_tokens` vs `max_output_tokens`）——各自写在对应 mode 的 `params` 里，不共用。
- 新常量：`CAPABILITY_WEB_SEARCH = "web_search"`。

### 4.4 校验（加载即失败）

- 每个 `[[models]]` 至少一个 `modes.*`；
- `modes.*.capabilities` 非空且含 `text`；
- `id` 全局唯一；
- `modes` 的 key 只允许 `completions` / `responses`（Rust 枚举，非法 key 报错）；
- **缺 `modes` 或仍写顶层 `capabilities` → `ConfigError`，不启动。**

### 4.5 代码触点

| 文件 | 改动 |
|------|------|
| `crates/lya-config/src/models.rs` | 重写 `ModelEntry`；删 flatten params / 顶层 capabilities |
| `crates/lya-config/templates/models.toml` | 整文件按 4.2 重写 |
| `crates/lya-config/tests/config.rs` | 删旧格式测试；加破坏性/schema 失败用例 |
| `crates/lya-core/src/run.rs` | `LlmEndpoint` 携带 `modes`；按 `api_mode` 取 params |
| `web/src/api/client.ts` | `ModelInfo.modes` 供 Picker 过滤 |

---

## 5. 会话：`api_mode` 落库

### 5.1 迁移

`crates/lya-session/migrations/003_api_mode.sql`：

```sql
ALTER TABLE sessions ADD COLUMN api_mode TEXT NOT NULL DEFAULT 'completions';
```

- 已有会话全部 `completions`，行为不变。
- 新列**不设** CHECK（方便以后加 vendor 专用 mode）；Rust 层枚举校验。

### 5.2 Rust / API

| 文件 | 改动 |
|------|------|
| `lya-session/src/types.rs` | `SessionMeta.api_mode`、`CreateSession.api_mode` |
| `lya-session/src/store/meta.rs` | CRUD；**无** `set_api_mode` |
| `lya-api/src/http/sessions.rs` | create 接受 `api_mode`；patch **拒绝**改 `api_mode` |
| `web/src/api/client.ts` | `SessionMeta.api_mode` |
| `web/src/views/session/SessionMetaTab.vue` | 创建时可改；已有会话只读展示 |

### 5.3 创建时的默认值

```text
请求体 api_mode  →  未传则 "completions"
```

校验：`model_id` 对应的 `ModelEntry.modes` **必须包含**该 `api_mode`。

---

## 6. `lya-llm`：Responses 客户端

### 6.1 模块划分

```text
lya-llm/
  endpoint.rs      + responses_url()
  message.rs       ChatMessage（completions，保持）
  responses/
    input.rs       Responses input item 类型
    body.rs        build_responses_body()
    sse.rs         Responses SSE → StreamEvent
  client.rs        dispatch by ApiMode
  event.rs         StreamEvent 扩展
```

### 6.2 `StreamEvent` 扩展

```rust
pub enum StreamEvent {
    TextDelta(String),
    ReasoningDelta(String),
    ToolCallDelta(ToolCallDelta),
    /// 原生联网状态（不进 tool batch）
    WebSearchStatus(WebSearchStatus),
    Finished { reason: Option<String> },
}

pub enum WebSearchStatus {
    InProgress,
    Searching,
    Completed { query: Option<String> },
    Failed { message: Option<String> },
}
```

映射 DeepSeek SSE（参考 [Responses API Guide](https://api-docs.deepseek.com/guides/responses_api/)）：

| Provider event | StreamEvent |
|----------------|-------------|
| `response.output_text.delta` | `TextDelta` |
| `response.reasoning_text.delta` | `ReasoningDelta` |
| `response.function_call_arguments.delta` | `ToolCallDelta` |
| `response.web_search_call.in_progress` | `WebSearchStatus(InProgress)` |
| `response.web_search_call.searching` | `WebSearchStatus(Searching)` |
| `response.web_search_call.completed` | `WebSearchStatus(Completed { … })` |
| `response.completed` / `incomplete` / `failed` | `Finished` |

**注意：** Responses 流**没有** `data: [DONE]`；读到 `response.completed` 即结束。

### 6.3 请求体（Responses）

最小字段：

```json
{
  "model": "deepseek-v4-flash",
  "instructions": "<system prompt>",
  "input": [ /* item list，见 6.4 */ ],
  "stream": true,
  "tools": [
    { "type": "function", "name": "…", "parameters": … },
    { "type": "web_search" }
  ],
  "tool_choice": "auto"
}
```

- `instructions` ← system prompt（与 completions 的 system 消息等价）。
- `tools` ← lya function tools（不含 DDG web_search）+ 条件性 `web_search`。
- 透传 params 来自 `modes.responses.params`。

### 6.4 上下文：MessageRecord → Responses input

新增 `lya-agent/src/context_responses.rs`（或 `lya-llm/responses/input.rs` + agent 调用）：

| MessageRecord | Responses item |
|---------------|----------------|
| user | `{ type: "message", role: "user", content: … }` |
| assistant 正文 | `{ type: "message", role: "assistant", content: … }` |
| assistant tool_calls | `{ type: "function_call", call_id, name, arguments }` |
| tool 结果 | `{ type: "function_call_output", call_id, output }` |
| assistant reasoning（历史） | `{ type: "reasoning", … }` 若 API 要求回灌；否则跳过（与 completions 一致，先**不回灌**） |
| 原生搜索（历史） | `{ type: "web_search_call", … }` **按 provider 文档原样回传** |

**时间戳前缀、中断标注、孤儿 tool 补全** 规则与 `context.rs` 对齐。

**落库：** assistant 消息需能存 `web_search_call` output items（扩展 `MessagePayload` / `openai` 旁路字段 `responses_items: Option<Value>`）。Phase 3 可先做「当轮可用、历史搜索块仅展示不回灌」，Phase 4 补全回灌。

### 6.5 `ChatBackend` 演进

```rust
pub enum ApiMode { Completions, Responses }

pub trait ChatBackend {
    fn chat_stream(
        &self,
        mode: ApiMode,
        endpoint: &LlmEndpoint,
        system: &str,
        path: &[MessageRecord],  // 或拆成两种 builder 的输出
        tools: Vec<Value>,
        native_web_search: bool,
    ) -> …;
}
```

实现策略：**一个 `LlmClient`** 内部分派，测试里 Mock 仍实现同一 trait。

---

## 7. Agent 与工具

### 7.1 轮次内流程（responses 分支）

```text
1. 读 SessionMeta.api_mode + model modes
2. native_web = mode==responses && capabilities 含 web_search
3. ToolBundle：
   - 始终：function tools + web_fetch
   - native_web 时：过滤掉 web_search（DDG）
4. build_input(mode, system, path) → messages 或 responses input
5. chat_stream(mode, …, native_web)
6. 流式处理：
   - WebSearchStatus → AgentEvent::ProviderSearch(…)
   - 其余不变
7. tool round：function_call 仍走现有 batch / HITL
```

### 7.2 `lya-tool` 改动

| 位置 | 改动 |
|------|------|
| `lya-agent` `effective_tools` 或 bundle 构造 | 传入 `exclude: ["web_search"]` |
| `web/src/views/ToolsView.vue` | 只读说明：responses 会话下 DDG 搜索不可用，改用原生 |

### 7.3 Prompt 提示

`lya-prompt` / tool prompt 段：responses 会话注入一句——「联网由模型内置搜索完成，不要用 web_search；需要读正文仍用 web_fetch」。

---

## 8. 事件与前端

### 8.1 `AgentEvent` 新增

```rust
ProviderSearch {
    call_id: String,           // provider 侧 id，仅 UI 用
    phase: ProviderSearchPhase,
    query: Option<String>,
}
```

### 8.2 LyaSSE

| SSE type | payload | 用途 |
|----------|---------|------|
| `provider_search` | `{ call_id, phase, query? }` | 状态栏 + timeline |

`phase`: `in_progress` | `searching` | `completed` | `failed`

** deliberately 不用** `call_started(web_search)`，避免与 DDG tool 混淆。

### 8.3 Timeline 新块

```typescript
| { type: 'provider_search'; callId: string; phase: …; query?: string }
```

渲染：轻量 inline 条（类似「🔍 正在搜索：xxx」），完成后折叠或变灰。

### 8.4 状态栏

`turn.ts` / `ChatStatusBar.vue`：

| 条件 | 文案 |
|------|------|
| 最近 `provider_search.phase == searching` | 正在搜索… |
| 有 in_progress | 正在准备搜索… |

优先级：HITL > tool 执行中 > provider 搜索 > 思考 > 回复。

### 8.5 会话设置 UI

`SessionMetaTab.vue`：

- **api_mode**（仅**新建会话**流程或会话 Tab 在「尚无消息」时可选；已有对话的会话只读）：
  - 默认选中 **Completions**；
  - 切换为 Responses 后，模型 Picker **立即过滤**为带 `modes.responses` 的条目。
- **model_id** Picker：选项 = `models` 中 `modes[当前 api_mode]` 存在的条目；已锁定会话按会话 `api_mode` 过滤，不展示切过去会 400 的模型。
- 已有会话：**api_mode 只读** + 「创建时锁定；要换栈请新建会话」。
- 换模型失败（400）：toast 展示后端文案，不静默回退。

---

## 9. 测试策略

| 层级 | 内容 |
|------|------|
| **lya-config** | modes 解析、default、校验失败 case |
| **lya-session** | 迁移、create 写 api_mode、patch 拒绝 |
| **lya-llm** | fixture SSE 文件 → StreamEvent 向量；build body 快照 |
| **lya-agent** | MockBackend 按 mode 断言 URL/tools；web_search 被过滤 |
| **lya-hub** | AgentEvent → Envelope 含 provider_search |
| **web** | timeline reducer、status bar phase（vitest） |
| **集成** | opt-in：`DEEPSEEK_API_KEY` 真连 Flash responses + web_search |

Fixture 目录建议：`crates/lya-llm/tests/fixtures/responses/`.

---

## 10. 风险与决策记录

| 决策 / 风险 | 结论 |
|-------------|------|
| 默认栈 | **`completions`**；Responses 仅创建时显式选 |
| 旧 `models.toml` | **不兼容**；按模板重写，加载失败即报错 |
| Pro 暂不支持 Responses | 不声明 `modes.responses`；前端 Picker 自然筛掉 |
| `web_search_call` 历史回灌 | Phase 3 当轮 UI；Phase 4 持久化 |
| DDG vs 原生搜索 | 文档 + 会话级锁定；completions 会话仍用 DDG |
| 前后端防误选 | 前端过滤 Picker + 后端 400，双层 |

---

## 11. 实施 Phase（执行顺序）

### Phase 0 — 文档与类型骨架 ✅ 本文

- [x] 写 `docs/responses-api.md`
- [ ] `plan.md` P1 改为指向本文的 checklist

**验收：** 团队对 api_mode 锁定、tool 策略、事件名无歧义。

---

### Phase 1 — 配置与会话

| # | 任务 | 验收 |
|---|------|------|
| 1.1 | **破坏性**重写 `models.toml` schema + loader（无旧格式兼容） | 旧格式加载失败；新模板 `cargo test -p lya-config` |
| 1.2 | `SessionMeta.api_mode` 迁移 + store + HTTP；默认 `completions` | 旧会话=completions |
| 1.3 | create / patch `model_id` 校验 `api_mode`；patch `api_mode` → 400 | 集成测试 |
| 1.4 | 前端：api_mode 默认 completions；模型 Picker 按栈过滤 | 切 Responses 后 Pro 不可选 |

---

### Phase 2 — `lya-llm` Responses 通路（无原生搜索）

| # | 任务 | 验收 |
|---|------|------|
| 2.1 | `responses/sse.rs` + fixture 测试 | 文本+reasoning+function_call 增量 |
| 2.2 | `build_responses_body` + `responses_url` | 快照测试 |
| 2.3 | `ChatBackend` 分派；agent 接 `api_mode` | Mock：responses 会话走 `/responses` |
| 2.4 | `context_responses` 最小 input 转换 | 单轮 user→assistant 集成测试 |

**验收：** responses 会话能流式对话、能跑 function tool 轮次；**尚未**开 web_search。

**状态：** ✅ 2026-08-03

---

### Phase 3 — 原生联网

| # | 任务 | 验收 |
|---|------|------|
| 3.1 | 请求体注入 `{type:web_search}`；过滤 DDG tool | agent 测试：schemas 无 web_search |
| 3.2 | `WebSearchStatus` → `AgentEvent` → LyaSSE | hub 测试 |
| 3.3 | timeline + status bar | 手动：Flash responses 搜「今天天气」 |
| 3.4 | prompt 段说明原生搜索 | 模型不再调用 DDG web_search |

**验收：** 真连 DeepSeek（可选）或 fixture 完整走通 UI。

---

### Phase 4 — 持久化与回放

| # | 任务 | 验收 |
|---|------|------|
| 4.1 | assistant 消息存 `web_search_call` items | 刷新后会话 timeline 仍见搜索块 |
| 4.2 | `build_responses_input` 回灌历史 search items | 多轮对话不 400 |
| 4.3 | `web_fetch` 与原生搜索联用场景测试 | 搜索摘要 + fetch 正文 |

---

### Phase 5 —  polish

| # | 任务 | 验收 |
|---|------|------|
| 5.1 | ToolsView / ModelsView 只读说明 | 文案准确 |
| 5.2 | 错误信息：api_mode 不兼容、Pro 无 responses | 用户可理解 |
| 5.3 | `lya-llm` / 根 README 双栈说明 | 与本文一致 |

---

### Phase 6 — 收尾

| # | 任务 | 验收 |
|---|------|------|
| 6.1 | `plan.md` P1 全部 `[x]` | — |

**不做：** runtime 全局 `default_api_mode`（默认栈固定 completions，不增加第二配置源）。

---

## 12. 参考链接

- [DeepSeek Responses API Guide](https://api-docs.deepseek.com/guides/responses_api/)
- [Create Response API](https://api-docs.deepseek.com/api/create-response/)
- [DeepSeek Chat Completions（现有）](https://api-docs.deepseek.com/)

---

## 13. 维护

- 实施中若决策变更，**先改本文**，再改代码。
- 每完成一个 Phase，在 `plan.md` P1 对应项打 `[x]` 并注明日期。
- 不要另起 `responses-todo.md`。
