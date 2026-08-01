# lya 开发计划

记录已完成的模块边界、**特意延后**的工作，以及接下来的实现顺序。

范围锚点见 [TMP.md](./TMP.md)（功能清单与四层架构），本文只跟踪进度与取舍。

---

## 现状

| crate | 职责 | 状态 |
|-------|------|------|
| `lya-http` | 共享 `reqwest` 客户端、流式响应、错误分类 | 可用 |
| `lya-llm` | OpenAI 兼容 chat/completions、SSE 解析与增量装配 | 可用 |
| `lya-tool` | `Tool` trait、注册中心、按 RWX 与名单筛选出 prompt + schema | 可用（9 个工具） |
| `lya-prompt` | 系统认知 / 自我认知 / 人设，其余段落外部注入 | 可用 |
| `lya-mode` | ask / edit / agent 与权限映射、模式提示词、工具筛选 | 可用 |
| `lya-db` | 数据目录、连接、迁移、写事务 | 可用 |
| `lya-session` | 会话元数据、消息树、分支、HITL 独立节点 | 可用 |
| `lya-memory` | 跨会话笔记的仓储 + 常驻索引渲染 | 可用 |
| `lya-action` | 元认知动作：记忆读写、表单、请求切模式 | 可用 |
| `lya-config` | 分层配置：core / runtime / 模型清单 / 人设 | 可用 |
| `lya-agent` | 一轮对话的驱动器，串起以上全部 | 可用 |

**后端骨架已经打通**：从配置加载、开会话、组提示词、调 LLM、分发工具与动作、
结果回写消息树，到 HITL 挂起与恢复，整条链路都跑通了，109 个测试覆盖。
剩下的主要是**广度**（工具只有一个）和**外壳**（HTTP 层、界面、托盘）。

依赖方向保持单向：`session → db`、`memory → db`、`mode → tool`、`llm → http`、
`action → tool/memory/session/mode`、`config → mode`、`agent → 全部`，
彼此不反向依赖。

**启用工具的三层语义是统一的**：`None`（会话表里是 NULL、配置里是键缺省）
= 全部启用；`Some(list)` = 只启用列出的；`Some([])` = 全禁。`lya-config`
的 `tools.enabled`、`sessions.enabled_tools_json`、`ToolRegistry::bundle`
的 `names` 三者对齐，中间不做转换。

`lya-session` 与 `lya-memory` 共享同一个 `Arc<Db>`：调用方先把两份迁移都
`with_migration` 进去再 `migrate()`，然后用 `with_db` 分别构造。这样写入
仍走同一把锁，不会两个连接互相抢。

---

## 特意延后的工作

延后不等于放弃，每条都记了**为什么**和**什么时候做**。

### lya-session

- **`branch_meta` 表（分支命名）**：表已建，没有对应 API。
  分支现在靠叶节点 id 区分够用了，等 WebUI 要显示「分支名」时再补。
- **~~`session_events` 表~~已删**：曾为 SSE 断线续传预留，定协议时发现用不上，
  理由见下面的「LyaSSE」。
- **`set_model` 不校验模型是否存在**：会话层不认识模型清单（那是 `lya-config`
  的事），只存 id。校验在 HTTP 层写入时做；真指到不存在的模型，`lya-agent` 会
  在跑一轮时**报错而不是悄悄退回默认**——静默换成另一个模型（可能更贵、能力也
  不同）比直接说清楚更让人困惑。
- **删子树**：目前只允许 `delete_leaf`。
  递归删整条分支要先想清楚「当前 leaf 在被删子树里」怎么办，等 UI 上真的
  有「删除这个分支」按钮时再定语义。

### lya-tool

已有 6 个，按权限分级（`tools::tests::builtin_tools_are_graded_by_permission`
钉住这张表）：

| 工具 | 权限 | 可见于 |
|------|------|--------|
| `file_read` / `dir_list` / `system_info` / `web_search` / `web_fetch` | `-R-` | ask 起 |
| `file_write` / `file_edit` | `-R-W-` | edit 起 |
| `file_manage`（删/移/拷/信息/建目录） / `bash` | `-R-W-X-` | 仅 agent |

删除与移动划到执行级而不是写级：它们不可逆，和「改文件内容」不是一回事。

粒度上刻意比上一代粗——它文件 8 个、目录 8 个共 16 个工具，我们 5 个。读和写
各自独立（用得最多，schema 要精确），低频的删/移/拷/信息/建目录合成一个
`file_manage`，省提示词预算。

- **`bash` 的解析是「够看懂」而不是完整 shell 语法**。遇到 `$(...)`、反引号、
  引号没闭合就如实标注看不懂并强制确认——解析失败恰恰是最该拦的情况，绝不能
  当作安全放行。想覆盖更多语法就得往真解析器走，暂时不值得。
- **`bash` 没有沙箱**：子进程继承完整环境，`kill_on_drop` + 超时能收掉进程，
  但拦不住它在超时前干的事。确认流程是唯一的闸门。
- **动作收不到取消信号**：`Tool::call` 有 `ToolCtx` 了，但 `Action::call` 只有
  `ActionCtx`（会话 id + 模式）。动作都很快，暂时不需要；真要加就往 `ActionCtx`
  里塞一个 `CancelToken`。
- **`image_scan` 不做感知哈希**：上一代那三个工具（扫描、详情、找重复）合成了
  一个，靠 `imagesize` **只读文件头**拿尺寸与格式——真去解码每张图会慢两个数量
  级，而我们并不需要像素。找重复只做 sha256 精确匹配：上一代的 dHash 能找出
  「相似但不同」的图，但必须完整解码加缩放，且阈值调松了误报、调紧了漏，而
  「同一张图存了两份」已经覆盖清理家目录的主要场景。
- **`image_scan` 不读 EXIF**：拍摄时间、机型、GPS 要另加一个解析库。等真要整理
  照片再说。
- **图片仍不是视觉能力**：工具只给路径和尺寸，模型看不见画面。要看懂图得靠支持
  多模态的模型，见「多模态」。
- **`web_fetch` 不能翻页**：只有 `max_chars` 截断，没有偏移量。长文档读不全时
  只能换更具体的页面。等真遇到了再加。
- **`web_fetch` 不拦内网地址**：只挡住了非 http(s) 协议。等 HTTP 层落地后要重新
  评估——那时 lya 自己会在 `127.0.0.1:51616` 上监听且无鉴权，网页里的提示词注入
  有可能诱导模型去抓它。现在没有那个面，先不拦，免得挡掉「看看我本地开发服务器」
  这类正当用途。
- **`web_search` 依赖 DDG 的 HTML 结构**：页面改版就会解析不出结果。选它是因为
  不需要 API key，对本地应用最省事。解析时**必须滤掉 `result--ad`**——DDG 会把
  广告混在结果最前面，链接指向自家跳转统计页，模型分辨不出那是广告。
- **不做 `text_*`**（统计/diff/正则替换那六个）和 `http_request`：前者模型自己
  就能处理，后者与 `web_fetch` 重叠且滥用面更大。
- **`bash` 需要自带命令解析与确认**：模型很爱返回一长串 `&&` 和 `|` 串起来的
  命令，原样丢给用户等于让人闭眼签字。这个工具要把命令拆开、逐段说明它在干
  什么，再走 `HitlBlock::ToolConfirm` 请用户放行。**确认逻辑属于 bash 自己**，
  不做成所有工具通用的风险等级，见「明确不做」。

  上一代在这件事上没做成，可抄与不可抄的部分见文末「bash」一节。落地时要先解决
  一个结构问题：**目前只有 action 能挂起，工具不能**——`dispatch` 里工具只能返回
  `Dispatched::Result`，得先给它开一条通向 `AwaitHuman` 的出口。
- **工具内部没有超时**：「停止本轮」已经由 `lya-agent` 的 `CancelToken` 覆盖，
  但一个卡死的工具调用本身还拦不住。等 `bash` 落地时一并设计，那才是真会卡的
  场景（上一代默认 30 秒）。

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
  ~~**HITL 的答复没有结构化留档**~~：已做。`resolve_hitl` 带一个答案参数，把原始
  作答存进 HITL 节点的 `lya.meta.answer`，界面回看时能原样回显勾选项，不必从
  渲染后的中文里反解。

### lya-agent

- **流式只落两次库**（开始占位、结束定稿）。进程崩在生成中途会丢掉半截正文，
  只留一条 `Streaming` 记录。真觉得可惜再改成阶段性落盘。
  ~~**`Streaming` 残留没有启动清理**~~：已做。`SessionStore::mark_stale_streaming`
  在 `serve` 启动时扫一遍标成 `Interrupted`——不清的话界面会把崩溃留下的半截消息
  渲染成一条永远转圈的气泡。
- **换模型不会重算已有上下文**：会话中途换模型后，历史消息原样喂给新模型。
  不同模型对同一段历史的理解可能有差异，但重写历史更糟，先这样。
- **确认期间的执行没有流式进度**：`resolve_tool_confirm` 里同步跑完才返回，
  一条跑三十秒的命令在界面上就是干等。要给进度得让它也走事件流。
- **命令行 example 不处理 HITL**：碰到需要确认时只打印一行提示。表单交互留给
  WebUI，`submit_form` / `resolve_mode_change` 接口已经有了。

### lya-config

- **不做文件监听**：「可热改」目前等于「重新 `load_from` 一次就生效」。
  runtime 那几个值不影响已建立的连接与监听端口，够用了；真 watcher 等 HTTP
  层再说。
- **只暴露了记忆索引的三个数值**：`MemoryLimits`（标题/摘要/正文长度上限）和
  `file_read` 的行数上限仍是代码里的常量。它们是护栏不是口味，没人会调，等
  真有人要改再往 `runtime.toml` 加。
- **密钥明文存 `models.toml`**（权限 0600）。本地单用户场景够了，不支持
  `env:VAR` 这类间接引用。

### lya-core（HTTP 层）

- **本地图片端点的令牌**：`/api/local-image` 额外要一个启动时随机生成的令牌，
  从 `/api/bootstrap` 下发。原因是跨站守卫对「没带 `Origin` 的 GET」放行，而
  **`<img src>` 按规范就不带 `Origin`**——不加令牌的话任何网页都能拿这个端点
  探测你家目录里某个路径是不是图片、并读到它的尺寸。令牌只走 JSON 端点：跨域
  `fetch` 一定带 `Origin` 会被挡，跨域 `<script>`/`<img>` 又读不到 JSON。
  代价是重启后旧页面的图片链接失效，刷新即可。
- **本地图片不做缩略图**：生成缩略图要解码加缩放，得拉进 `image` crate。
  localhost 上传输不是瓶颈，真正的开销是浏览器解码大图——先用 32 MiB 上限挡住
  误引用，等实际卡了再补。
- **本地图片的路径规则比 `file_read` 严**：只认家目录内，且 `canonicalize` 之后
  再校验。工具允许绝对路径逃出家目录（有模式权限兜底），但这是浏览器能直接访问
  的 URL，放开就等于任意文件读取；不解符号链接的话 `~/link -> /etc/shadow`
  就绕过去了。MIME 认不出就拒绝，不猜。

### lya-prompt

- **提示词必须逐字节确定**：整段不含任何随时间变化的内容，否则 API 商的前缀
  缓存每轮都会全量失效。当前时间通过消息前缀传达（见 `TIME_ANCHOR`）。
  `lya-agent` 有一条 `system_prompt_is_byte_stable_across_rounds` 测试钉住
  这个不变量，往提示词里塞时间戳或随机序会在那里炸。
- **记忆写入会打断缓存**：记忆索引在系统提示词里，写一条记忆就换掉整个前缀，
  那一次请求全量 miss。写入很稀疏（上一代三个月 8 次），先不管。真变频繁了
  可以把索引挪到靠近末尾的独立消息，那样只失效尾部——代价是模型会更倾向把它
  当对话内容而不是设定。

### lya-llm / lya-http

- **无 token 用量统计**：等有地方展示（会话详情或设置页）再采。
- **无重试策略**：本地单用户场景先让错误直接冒到 UI，比静默重试好排查。

---

## 接下来的顺序

WebUI 与托盘 →（视需要）多模态

后端到此存档：九个工具 + `image_scan`、五个动作、HTTP 层与 `SessionHub` 都已落地，
存档前清掉了两条会影响前端的遗留项（`Streaming` 残留、HITL 答复留档）。剩下的延后
项都不挡前端动工。

工具已经补齐九个，三个模式真的不一样了。**工具确认链路也打通了**，实现方式：

- `Tool::confirm_request(&self, args) -> Option<ConfirmRequest>` 默认返回 `None`，
  现有工具一行不改。`ConfirmRequest` 定义在 `lya-tool`，**不能引用 `HitlBlock`**
  ——`lya-session → lya-mode → lya-tool`，反向依赖会成环，所以两边各定义一份、
  由 `lya-agent` 映射。
- 把「要不要确认」和「执行」拆成两个方法：前者是对参数的纯函数，后者才有副作用，
  且只在放行后发生。
- **恢复流程和表单不同**：表单的答复本身就是 tool 结果；确认的「同意」意味着
  *现在才去执行*。所以 HITL 节点存下工具名与参数，`resolve_tool_confirm` 批准时
  执行并**重新检查一遍权限**（挂起期间用户可能改过模式），拒绝时写 `[用户拒绝] …`。
  用户备注按 `[用户备注: "..."]` 混进结果。

**action 侧已经做完了。** 上一代 14 个 action 里，我们只需要 `memory` / `form` /
`transcript` 三类，其余 11 个（`delegate` / `interrupt` / `report` /
`spawn_worker` / `abort_plan` / `query_status` / `create_plan` / `modify_plan` /
`done` / `failed`）全是多角色与后台任务系统的架构开销，砍了子 agent 就一个都不
需要。而 `transcript` 的前提是上下文压缩，现在喂完整路径，用不上。

`SessionHub`（core 层）要负责的事，agent 刻意没做：spawn 任务消费
`run_turn` 的流并转发到广播（否则用户刷新页面就把对话掐了）、同会话轮次串行的
锁、以及在线订阅者的增量缓冲。

### LyaSSE：服务端主动推送协议

SSE 在这里不只是「聊天增量的管道」，而是**打破 REST 只能客户端发问的那一层**。
未来要承载桌面通知（托盘订阅后调 notify-send）、会话列表变化、配置被另一端改动
等等。这决定了两件事：

**一、事件信封现在就要留出 `scope`。**

```
event: message_delta
data: {"scope":"session:abc","type":"message_delta","seq":42,"payload":{…}}
```

现在只有 `session:<id>`，将来加 `global` 时**客户端的分发逻辑一行不用改**——它
本来就按 scope 路由。以后想合并成一条 `/api/events` 连接同时承载全局与会话事件，
也只是服务端多路复用，协议不变。反过来，若现在把事件写成裸的 `{"delta":"…"}`，
加第二种事件源时所有客户端都得改。

**二、订阅 = 先快照再增量，因此不需要事件表。**

流式文本是累积的，快照就是「到此刻为止的全部」，客户端收到直接整体替换。于是
**首次打开和断线重连走完全同一条路**，天然幂等，不需要 `Last-Event-ID`、不需要
序号对齐、不需要把事件落库重放。上一代「流式输出到一半刷新页面就丢渲染」的毛病，
根子在于 HTTP handler 直接持有执行流——handler 一断执行也断。

那份实时缓冲不是另一份数据，就是**正在写的那条 assistant 消息的当前内容**；
一轮结束落库、缓冲清掉。

**已知的坑：** `broadcast::Receiver` 在订阅者消费慢时会返回 `Lagged(n)`，
这时不能当错误断开，而要**重发一次快照**再继续。

**通知归托盘不归服务端**：服务端只广播事件，托盘进程订阅后自己调 notify-send，
这样服务端不用关心桌面环境。

**配置变更也走这条路**：改配置的入口在服务端，改完 reload 并广播，多端才不会
显示不一致。这顺带解决了 `lya-config` 那个「可热改只是重新 load」的半吊子状态。

### lya-agent 的设计（已实现）

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

`run_turn` **不接收用户输入**：用户消息由调用方先 append 进树。于是「发消息」
「重新生成」「编辑重发」「HITL 答复后继续」退化成同一套动作——改树，再跑一轮。

由此 **agent 自身无状态，HITL 不在内存里挂起**。表单发出去本轮就正常结束，
没有挂起的 future、没有 waiter 表、没有超时；用户何时答复都行，进程重启也能
接上。上一代把状态放内存，就得配一整套阻塞与超时机制。

验证方式：`cargo run -p lya-agent --example chat`。首次运行生成配置模板并提示
填 api_key，填好后就能真的对话。

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
- **单个工具的权限收紧**：上一代允许用户把某个工具的 RWX 改得比它声明的更严
  （`current_perm` 覆盖 `declared_perm`）。会话级的启用/禁用已经够表达「我不想
  让它用这个」，再加一层只会让「为什么这个工具不能用」变难排查。
- **通用的工具风险等级**：上一代给每个工具标 `risk_default` 和 `confirm_policy`
  （`never` / `on_high_risk` / `always`）。但真正需要确认的只有 `bash`——给所有
  工具都挂一个只有 bash 需要回答的属性，是冗余设计。而且笼统的「高风险，是否
  执行？」帮不了用户判断，真正有用的是把那串 `&&` 拆开讲清楚，那是 bash 自己的
  活。
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

### bash：从 lianclaw 学到的教训

它有**两套**命令检查代码：实际跑的是 `HighRiskCommandGuard`，**纯正则匹配整条
命令字符串**（17 条规则：`rm -rf /`、`mkfs`、`dd if=/dev/zero`、fork 炸弹、
`sudo`、`chmod 777 /`、`systemctl stop`、`kill -9` 等）。另有一个 `exec_guard/`
模块，用 shlex 正经分词、按 `| || && ; &` 拆边界、逐段按 argv 判断，还能识别
`curl | sh` 和 `eval`——**写完了但从未接进运行时**，是死代码。

而且它的确认界面没能解决真正的问题：payload 里 `command` 字段填的是**工具名**
`"Bash"`，真正的命令埋在 `arguments_preview` 的 JSON 里。用户看到的是
「ExecGuard 安全审查 / Bash / 参数: {...}」，**没有任何逐段解释**。所以我们要做
的命令拆解与人话说明，这边没有现成的可抄，那份死代码只能当规则表的起点。

值得照抄的细节：

- 输出上限 50 KiB，工具结果再截到 4096 字符（保头尾、省中间）
- 默认超时 30 秒，默认 cwd 是 `$HOME`
- 确认超时 120 秒，超时按拒绝处理
- 拒绝回给模型的是 `[用户拒绝] 用户未确认该操作，已取消执行。`
- **批准时用户可以附带备注**，以 `[用户备注: "..."]` 前缀混进结果——用户能说
  「可以，但别动日志」，比单纯的是/否有用
- 写文件类操作的确认**带 unified diff**，让用户看清改了什么再点

它没有任何沙箱，子进程继承完整环境。

### 时间与工具管控：从 lianclaw 学到的教训

- **时间不进系统提示词。** 它只在系统提示词放一段静态说明，真正的时间戳在
  序列化时加到 user / tool 消息前缀上，取自消息不可变的 `created_at`。这样
  历史渲染永远一致，前缀缓存不受影响。lya 照抄了这套，连「（距上一条消息
  11 小时）」「（日期已变更：…）」两个节奏提示一起。
- **模式切换要有痕迹。** 用户从界面切模式后，它插一条一次性的
  `[mode_switch]` 系统消息告诉模型。lya 做成**持久**的 system 节点——树是唯一
  真相，以后回看也解释得通行为边界为什么变了。
- **工具管控是两道关：列表过滤 + 执行前再查一次。** 只做前者挡不住模型凭空
  编一个没提供的名字。lya 原本就漏了第二道，照此补上了，判断依据复用本轮那次
  筛选的结果而不是重算条件。
- 它的工具开关是**全局**的（`tools` 表一行一个工具），lya 是**按会话**——这是
  当初就定好的差异，不改。

### 旧前端（`web-bak/`）

Vue 3 + TS + Vite，保留用途是**样式参考**，接口与逻辑都会重写。
已删 `node_modules/` 与 `dist/`，需要跑起来看效果得先 `npm install`。

组件：`ChatView` / `Composer` / `MessageBubble` / `ReviewPanel` / `Sidebar` /
`MemoryView` / `SettingsView`；样式集中在 `src/styles/theme.css`。

**多主题的前提**：现在是 Vue SFC，模板、样式、脚本焊在同一个 `.vue` 文件里，
所以「换 html + css、ts 不变」在当前结构下做不到。要么主题只换 CSS
（`theme.css` 已经是这条路，迁 lianclaw 样式很轻松），要么重写前端时就把
模板从 SFC 里拆出去。这个取舍在重写前端时定。
