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
| `lya-memory` | 跨会话笔记的仓储 + 常驻索引渲染 | 可用 |
| `lya-action` | 元认知动作：记忆读写、表单、请求切模式 | 可用 |
| `lya-config` | 分层配置：core / runtime / 模型清单 / 人设 | 可用 |

依赖方向保持单向：`session → db`、`memory → db`、`mode → tool`、`llm → http`、
`action → tool/memory/session/mode`、`config → mode`，彼此不反向依赖。

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

### lya-memory

- **没有检索**：现在把全部索引常驻 prompt，模型看得见所有条目、按编号读正文，
  比检索更准。`IndexBudget` 是安全阀——超预算就只留最近更新的若干条并说明还有
  多少没列出。等真的撑爆（或索引明显吃 token）再加 `search`，那时也已经知道
  真实查询模式了。
- **没有自动召回**：上一代设计了基于 embedding 的 auto-recall，但因为 embedding
  从未启用，它一天都没跑过。索引常驻本身已经覆盖了这个需求。
- **没有 lianclaw 导入脚本**：`~/.lianclaw/lianclaw.db` 里有 8 条旧记忆，
  正文在 `~/.lianclaw/memory/long_term/*.md`。要迁的话写个一次性命令即可，
  字段对应关系是 topic→title、summary→summary、md 正文→body、tags_json→tags，
  namespace 丢弃。

### lya-action

- **只有四个动作**：`memory_read` / `memory_write` / `form` /
  `request_mode_change`。`transcript`（回查被省略的历史）等上下文压缩做了
  再说，现在喂给模型的是完整路径，用不上。
- **HITL 的答复没有结构化留档**：用户答完表单后，渲染出的文本会作为
  `role=tool` 消息入树，但 HITL 节点本身只翻转成 resolved，不存原始答案。
  等界面要「回看当时选了什么」时，给 `resolve_hitl` 加一个答案参数存进
  `lya.meta` 即可。
- **动作与工具的重名没人查**：两边的 schemas 由 agent 合并进同一个
  `tools[]`，撞名应当在合并处报错。

### lya-config

- **不做文件监听**：「可热改」目前等于「重新 `load_from` 一次就生效」。
  runtime 那几个值不影响已建立的连接与监听端口，够用了；真 watcher 等 HTTP
  层再说。
- **只暴露了记忆索引的三个数值**：`MemoryLimits`（标题/摘要/正文长度上限）和
  `file_read` 的行数上限仍是代码里的常量。它们是护栏不是口味，没人会调，等
  真有人要改再往 `runtime.toml` 加。
- **密钥明文存 `models.toml`**（权限 0600）。本地单用户场景够了，不支持
  `env:VAR` 这类间接引用。

### lya-prompt

- **`action_section` 已可用**：接 `ActionRegistry::prompt_section(mode)`。

### lya-llm / lya-http

- **无 token 用量统计**：等有地方展示（会话详情或设置页）再采。
- **无重试策略**：本地单用户场景先让错误直接冒到 UI，比静默重试好排查。

---

## 接下来的顺序

`lya-agent` → 补齐工具（`file_write` / `bash` / `dir` / `web` / `image`）
→ HTTP 层与 `SessionHub` → WebUI 与托盘

先 agent 后工具：现在九个 crate 全是单元测试，一次真实 LLM 往返都没跑过，
消息树转 `ChatMessage`、HITL 挂起恢复、结果回灌这些接缝只有跑起来才知道对不对。
补工具是纯增量，`file_read` 已经把路走通了，不会再动架构。

**注意**：在补齐写类工具之前，edit 与 agent 模式实际上和 ask 没区别——唯一的
工具 `file_read` 是只读的，两个高权限模式拿不到任何额外能力，模式系统处于空转。

### lya-agent

一轮对话的驱动器，把前面所有模块串起来：读会话路径装配上下文 → 组 prompt
→ 调 LLM → 分发 tool/action → 结果回写消息树 → HITL 中断与恢复。
它是唯一知道「一轮怎么跑」的地方，其余 crate 保持无状态。

循环规则只有一条：**assistant 消息带 `tool_calls` 就执行并回灌、继续下一轮；
不带就结束本轮。** 「边说边干」靠 `content` 与 `tool_calls` 同时出现表达
（`ChatMessage::assistant_tool_calls`），所以不需要 `done` 这类显式信号。

它要处理的边界：

- 模型闷头连调多轮工具、`content` 始终为空，界面上一片空白——靠提示词治，
  文案抄 lianclaw 的「不要沉默地连续操作多轮」
- 工具调用死循环——靠 `max_tool_rounds` 轮数上限兜
- 模型返回空 `content` 且无 `tool_calls`，本轮静默结束
- 合并 tool 与 action 的 schemas 时检测重名

做完时要能**真的跟 DeepSeek 说上话**：带一个最小命令行 example，读配置、开
会话、发一句话、看到流式输出、让它调一次 `file_read`。跑通了才算数，否则仍然
只有单元测试。

HITL 的完整链路（表单为例）：模型调 `form` → agent 追加带 `tool_calls` 的
assistant 节点**和**一个 `role=hitl` 的 pending 节点 → 用户提交 → agent 用
[`render_form_answer`] 渲染成文本、追加 `role=tool` 节点（`tool_call_id` 对应）
并 `resolve_hitl` → 继续本轮。HITL 节点的 `openai` 保持 `None`，只服务界面与
状态恢复；模型上下文里始终是标准的 tool_call / tool_result 配对。

之后才是 HTTP 层、`SessionHub` 订阅协议、WebUI 与托盘。

---

## 明确不做

- **`done` 动作**：不带 `tool_calls` 的 assistant 消息即本轮结束。上一代的
  `done` 是给后台 worker 用的——后台任务没有用户可回复，必须显式声明完成；
  我们砍了子 agent 就不需要。
- **skills 体系**：工具提示词与实现放在一起，不再单独抽一层。
- **子 agent / `spawn_agent`**：太复杂，砍掉。TMP.md 里相关条目作废。
- **插件系统**：lianclaw 的历史包袱，不带过来。
- 多用户鉴权、远程部署。

---

## 旧实现参考

工作区已清理，只保留 `lya/` 与 `web-bak/`。以下是删除前从旧实现里摘出的、
对后续仍有价值的部分。

### 旧配置分层（原 `.lya-bak/`）

已落实到 `lya-config`，四个文件基本照搬（旧的 `agent.toml` 只有一个
`max_tool_rounds`，并进了 `runtime.toml`）。原始形状记录如下：

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

### 记忆：从 lianclaw 学到的教训

翻了 lianclaw 三个月的真实数据（`~/.lianclaw/lianclaw.db`），几点直接影响了
`lya-memory` 的取舍：

- **只攒了 8 条，正文合计 8.7 KB。** 这个量级下检索是伪需求，索引常驻更准。
- **`memory_embeddings` 表是空的。** 它建了 embedding 表、hybrid 检索（语义 +
  关键词兜底）和依赖 embedding 的 auto-recall，实际一次都没启用过，一直走
  SQL `LIKE`。提前造的检索既没用上也没必要。
- **分类退化：namespace 的 general 占 5/8。** 提示词建议了 `user_profile` /
  `preference` / `environment` / `project` / `workflow`，但连「Hyprland 崩溃
  排查」这种明显属于 workflow 的也躺在 general 里。**给 LLM 兜底选项，它就
  一直选兜底**，所以 lya 索性砍掉分类只留标签。
- **标签用得很好。** 每条 3-20 个具体名词，检索价值远高于分类。
- **正文拆到 Markdown 文件是负担。** 它用 SQLite 存索引 + `*.md` 存正文，
  一致性得自己维护、topic 里的 `/` 要转义、备份要管两处。
- **LLM 无删除权**是有意为之，lya 沿用。
- 写入门槛几乎全靠提示词治理（大段「什么时候该写/不该写」），三个月只攒 8 条
  说明确实有效，这批文案已抄进 `lya-action` 的 `memory_write` 用法说明。

### HITL：从 lianclaw 学到的教训

- **它没有「请求切换模式」这个动作。** 模式只能用户从 HTTP 改，模型撞到权限墙
  时只会干说一句「请切到 agent 模式」。lya 补上了 `request_mode_change`，
  走 HITL 让用户一次确认。
- **表单题型只有 single / multi，没有自由文本。** 「它在哪个目录？」这种只能
  靠表单级的 `_freetext` 兜。而且「备注」不在题目定义里，是硬塞在答案侧的
  `{题目id}_note` 魔法键，题目本身没法声明要不要备注，前端只好给所有题都挂
  一个备注框。lya 加了 `text` 题型和每题显式的 `allow_note`。
- **题目和选项数量没有任何上限。** lya 限制 10 题 / 20 选项。
- **答复走 `role=tool` 而不是伪造用户消息**，`tool_call_id` 对应那次 form
  调用——这点是对的，lya 沿用，正好和 HITL 独立节点互不干扰。

### 旧前端（`web-bak/`）

Vue 3 + TS + Vite，保留用途是**样式参考**，接口与逻辑都会重写。
已删 `node_modules/` 与 `dist/`，需要跑起来看效果得先 `npm install`。

组件：`ChatView` / `Composer` / `MessageBubble` / `ReviewPanel` / `Sidebar` /
`MemoryView` / `SettingsView`；样式集中在 `src/styles/theme.css`。

**多主题的前提**：现在是 Vue SFC，模板、样式、脚本焊在同一个 `.vue` 文件里，
所以「换 html + css、ts 不变」在当前结构下做不到。要么主题只换 CSS
（`theme.css` 已经是这条路，迁 lianclaw 样式很轻松），要么重写前端时就把
模板从 SFC 里拆出去。这个取舍在重写前端时定。
