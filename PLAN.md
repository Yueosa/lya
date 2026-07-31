# lya 开发计划

记录已完成的模块边界、**特意延后**的工作，以及接下来的实现顺序。

范围锚点见 [TMP.md](./TMP.md)（功能清单与四层架构），本文只跟踪进度与取舍。

---

## 现状

| crate | 职责 | 状态 |
|-------|------|------|
| `lya-http` | 共享 `reqwest` 客户端、流式响应、错误分类 | 可用 |
| `lya-llm` | OpenAI 兼容 chat/completions、SSE 解析与增量装配 | 可用 |
| `lya-tool` | `Tool` trait、注册中心、按 RWX 与名单筛选出 prompt + schema | 可用（仅 `file_read`） |
| `lya-prompt` | 系统认知 / 自我认知 / 人设，其余段落外部注入 | 可用 |
| `lya-mode` | ask / edit / agent 与权限映射、模式提示词、工具筛选 | 可用 |
| `lya-db` | 数据目录、连接、迁移、写事务 | 可用 |
| `lya-session` | 会话元数据、消息树、分支、HITL 独立节点 | 可用 |

依赖方向保持单向：`session → db`、`mode → tool`、`llm → http`，彼此不反向依赖。

---

## 特意延后的工作

延后不等于放弃，每条都记了**为什么**和**什么时候做**。

### lya-session

- **`branch_meta` 表（分支命名）**：表已建，没有对应 API。
  分支现在靠叶节点 id 区分够用了，等 WebUI 要显示「分支名」时再补。
- **`session_events` 表（事件序列）**：表已建，没有对应 API。
  它是给 SSE 断线续传 / 重放用的，等 `SessionHub` 的订阅协议定下来再写，
  否则字段形状会跟着协议反复改。
- **`sessions` 表缺 `model_id` 列**：会话要能各自选模型就得加，
  等 `lya-config` 的模型清单定型后一起补迁移。
- **删子树**：目前只允许 `delete_leaf`。
  递归删整条分支要先想清楚「当前 leaf 在被删子树里」怎么办，等 UI 上真的
  有「删除这个分支」按钮时再定语义。
- **消息树 → `lya_llm::ChatMessage` 的转换**：还没人做。
  这一步要决定 HITL 节点、思考内容、被中断的消息怎么喂给模型，属于对话
  策略而不是存储，放进 `lya-agent`。

### lya-tool

- **只实现了 `file_read`**：`dir` / `file_write` / `bash` / `web` / `image`
  等一律待补。先用一个工具把 trait、schema 导出、权限筛选跑通，剩下的按
  需要补，每个工具的提示词跟实现放在一起。
- **`invoke` 没有超时与取消**：等 `lya-agent` 的一轮驱动成型后，超时策略
  跟「停止本轮」一起设计，避免两套取消机制。
- **`Tool::call` 拿不到会话上下文**：现在签名是 `(args) -> ToolResult`。
  action 要改会话状态，做 `lya-action` 时会碰到这个边界，见下。

### lya-prompt

- **`action_section` 目前无人注入**：接口留好了，等 `lya-action` 产出元认知
  段落再接上。

### lya-llm / lya-http

- **无 token 用量统计**：等有地方展示（会话详情或设置页）再采。
- **无重试策略**：本地单用户场景先让错误直接冒到 UI，比静默重试好排查。

---

## 接下来的顺序

`lya-memory` → `lya-action` → （`lya-config`）→ `lya-agent`

具体先后再议，但 `lya-config` 必须排在 `lya-agent` 之前。

### lya-memory

只依赖 `lya-db`。记忆的存储与检索：自带迁移 SQL、增删改查、按需召回。
放在 action 之前，因为记忆类 action 要调它。旧实现的表结构可直接参考，见文末。

### lya-action

元认知层，产出 `lya-prompt` 的 `action_section`，并执行 memory / form /
`request_mode_change` / `done` 等动作。

**已定**：action 与 tool **对外走同一套协议**（OpenAI function calling），
lya 内部当两类东西治理——tool 受 mode 的 RWX 与会话启用名单约束，
action 不受约束、始终可见。

**待解**：`Tool::call(args)` 没有会话上下文，而 action 恰恰要改会话状态。
倾向共用「schema 与提示词导出」这一层，执行分两路，由 agent 按名字路由。

### lya-config

三级：

1. **core**：进程级，启动读取、改了要重启。端口、日志级别、db 路径、
   http 超时与连接池。
2. **runtime**：各模块默认值，可热改。默认模式、默认启用工具、全局人设、
   `max_tool_rounds`、`file_read` 行数上限等。
3. **session**：**不在配置文件里**，已经存在 `sessions` 表。`lya-config`
   对这一级的唯一职责是「会话没设时给什么默认值」，不重复存储。

模型清单是**第四类**东西，既不是层级也不是默认值，而是带密钥的资源目录，
单独成文件并注意文件权限。

### lya-agent

一轮对话的驱动器，把前面所有模块串起来：读会话路径装配上下文 → 组 prompt
→ 调 LLM → 分发 tool/action → 结果回写消息树 → HITL 中断与恢复。
它是唯一知道「一轮怎么跑」的地方，其余 crate 保持无状态。

之后才是 HTTP 层、`SessionHub` 订阅协议、WebUI 与托盘。

---

## 明确不做

- **skills 体系**：工具提示词与实现放在一起，不再单独抽一层。
- **子 agent / `spawn_agent`**：太复杂，砍掉。TMP.md 里相关条目作废。
- **插件系统**：lianclaw 的历史包袱，不带过来。
- 多用户鉴权、远程部署。

---

## 旧实现参考

工作区已清理，只保留 `lya/` 与 `web-bak/`。以下是删除前从旧实现里摘出的、
对后续仍有价值的部分。

### 旧配置分层（原 `.lya-bak/`）

旧实现按文件分层，和现在定的三级基本一致，可直接借鉴：

- `core.toml`：`[server]` host / port（51616）/ `port_backoff_max`（端口被占
  用时向后试的最大偏移）；`[db] sqlite_path`（相对 `~/.lya`）；
  `[http]` `timeout_secs = 120` / `connect_timeout_secs = 10` /
  `pool_idle_timeout_secs = 90` / `pool_max_idle_per_host = 4` / `user_agent`
- `agent.toml`：`max_tool_rounds = 32`，单次用户输入内 LLM↔tool 的最大轮数，
  防死循环
- `model.toml`：`[[models]]` 数组，固定字段 `id` / `name` / `base_url` /
  `api_key` 会校验，**其余字段原样保留并组进请求体**（如 `reasoning_effort`、
  `thinking = { type = "enabled" }`），这样加新模型参数不用改代码
- `persona.toml`：全局人设 `text`；会话人设非空时**覆盖**而非合并

### 旧 memory 表结构（原 `.db-bak/init.sql`）

明确定位为「跨会话显式笔记，无 embedding」：

```sql
memories(id TEXT PK, name TEXT, body TEXT, created_at, updated_at)
memory_categories(memory_id → memories.id, category, PK(memory_id, category))
memory_tags(memory_id → memories.id, tag, PK(memory_id, tag))
-- category / tag 各建索引
```

分类与标签拆成关联表而不是塞 JSON 字段，是为了能按 category / tag 直接查。

### 旧前端（`web-bak/`）

Vue 3 + TS + Vite，保留用途是**样式参考**，接口与逻辑都会重写。
已删 `node_modules/` 与 `dist/`，需要跑起来看效果得先 `npm install`。

组件：`ChatView` / `Composer` / `MessageBubble` / `ReviewPanel` / `Sidebar` /
`MemoryView` / `SettingsView`；样式集中在 `src/styles/theme.css`。

**多主题的前提**：现在是 Vue SFC，模板、样式、脚本焊在同一个 `.vue` 文件里，
所以「换 html + css、ts 不变」在当前结构下做不到。要么主题只换 CSS
（`theme.css` 已经是这条路，迁 lianclaw 样式很轻松），要么重写前端时就把
模板从 SFC 里拆出去。这个取舍在重写前端时定。
