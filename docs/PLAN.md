# lya 开发计划

记录已完成的模块边界、**特意延后**的工作，以及接下来的实现顺序。

范围锚点见 [TMP.md](../TMP.md)（功能清单与四层架构），本文只跟踪进度与取舍。

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
- **删子树**：目前只允许 `delete_leaf`。
  递归删整条分支要先想清楚「当前 leaf 在被删子树里」怎么办，等 UI 上真的
  有「删除这个分支」按钮时再定语义。
- **消息树 → `lya_llm::ChatMessage` 的转换**：还没人做。
  这一步要决定 HITL 节点、思考内容、被中断的消息怎么喂给模型，属于对话
  策略而不是存储，打算放进 `lya-agent`。

### lya-tool

- **只实现了 `file_read`**：`dir` / `file_write` / `bash` / `web` / `image`
  等一律待补。先用一个工具把 trait、schema 导出、权限筛选跑通，剩下的按
  需要补，每个工具的提示词跟实现放在一起。
- **`invoke` 没有超时与取消**：等 `lya-agent` 的一轮驱动成型后，超时策略
  跟「停止本轮」一起设计，避免两套取消机制。

### lya-prompt

- **`action_section` 目前无人注入**：接口留好了，等 `lya-action` 产出元认知
  段落再接上。

### lya-llm / lya-http

- **无 token 用量统计**：等有地方展示（会话详情或设置页）再采。
- **无重试策略**：本地单用户场景先让错误直接冒到 UI，比静默重试好排查。

### 配置

- **还没有 `lya-config`**：`LlmEndpoint`（base_url / api_key / 模型参数）
  和全局人设目前得由调用方硬构造。`lya-agent` 之前必须补上，见下。

---

## 接下来的顺序

`lya-memory` → `lya-action` → （`lya-config`）→ `lya-agent`

### 1. `lya-memory`

只依赖 `lya-db`。负责记忆的存储与检索：自带迁移 SQL、增删改查、按需召回。
放在 action 之前，因为记忆类 action 要调它。

### 2. `lya-action`

元认知层，产出 `lya-prompt` 的 `action_section`，并执行 memory / form /
`request_mode_change` / `done` 等动作。开工前要先定一件事：**action 和 tool
是不是同一套机制**——两者都要出提示词和 schema，但 action 不受 RWX 约束、
且能改会话自身状态。

### 3. `lya-config`

小 crate，供给 endpoint、模型参数、全局人设。可以并进 `lya-agent` 一起做，
但 agent 无法绕过它。

### 4. `lya-agent`

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
