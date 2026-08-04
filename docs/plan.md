# lya 计划（唯一活文档）

`docs/` 下只有这一份活文档。别再新建 `TODO-xxx.md` / `roadmap.md` 之类的第二份清单——
之前就是三份清单互相打架，最后谁都不准。

维护规则：

1. 新条目进 P0–P3；做完打 `[x]` 加日期。
2. 决定不做的挪进「刻意不做」，写清原因；有触发条件的挪进「延后」。
3. 改配置层级时先改 `lya-config/src/lib.rs` 注释，再改本文「配置体系」。
4. 本文只记**计划与取舍**，不当说明书用。

---

## 配置体系（现状，以代码为准）

数据目录：`~/.lya/`。四个配置文件，没有第五个。

| 层级 | 存储 | 内容 | 谁改 |
|------|------|------|------|
| 进程 | `core.toml` | `server`（host / port / port_backoff_max）、`log.level`、`db.path`、`http`（超时 ×2、连接池 ×2、user_agent） | 手改；**改完重启** |
| 全局默认 | `runtime.toml` | `agent`（轮次与并发上限、连续失败熔断、三个 default_*）、`tools.enabled`、`shell.confirm`、`memory`（索引预算 ×3）、`media.{image,video,audio}`（max_bytes / retain_local / retain_web） | 设置 → 默认配置；热加载 |
| 模型清单 | `models.toml` | id、name、base_url、api_key、context_window、`modes.{completions,responses}`（capabilities + params） | 模型页只读 + 手改 |
| 全局人设 | `persona.toml` | 人设正文 | 人设页 |
| **会话** | **`lya.db` → `sessions` 表** | 配置列：work_mode、model_id、api_mode、persona、enabled_tools_json | 聊天页会话设置 |

**没有** `~/.lya/session/config/`。会话级配置在数据库；全局默认只在 `runtime.toml`。

「热加载」= 下次请求重新 `Config::load_from` 一次，不含文件监听。

### 前端页职责（不要混）

导航是 `web/src/shell/types.ts` 的 `NAV_ITEMS`，**所有外壳都遍历它**，别手写按钮。

| 页面 | 职责 |
|------|------|
| **记忆** | 长期记忆的增删改查（列表 / 检索 / 编辑正文）。唯一能改记忆的地方 |
| **工具** | **只读** catalog：参数 schema、prompt_hint、内置限制 |
| **模型** | **只读** `models.toml`；可发探活请求 |
| **外观** | 主题切换 |
| **人设** | 编辑 `persona.toml` |
| **设置** | 两个标签页：「默认配置」写 `runtime.toml`（对话 / 工具 / 命令执行 / 记忆 / 媒体），「原始文件」只读四份 TOML 原文 |
| **存储** | **只读**占用观测；硬链接单独标注，不计入可释放空间 |
| **聊天 → 会话设置** | 概览 / 工具 / 会话 / 显示。本会话 tool 开关、显示偏好；空会话才能改 API 栈 |

外壳只有两个：`DefaultShell`（除 mc 外全部主题）和 `McShell`（`themeId === 'mc'`）。
MTF 是**主题**不是外壳，它的宽松卡片外观靠 `themes/mtf.css` 的 `--shadow-card` 等 token 实现。
边界不变：外壳可换，视图不可换（`src/shell/types.ts`）。

### 工具数值

- **不可配置**：上限/默认值在 `crates/lya-tool/src/limits.rs`
- **可配置**：新会话默认启用哪些 tool → `runtime.toml` `[tools].enabled`
- 前端只读表：`web/src/utils/toolLimits.ts`（改 `limits.rs` 时同步；2026-08-05 核对过，没有漂移）

---

## 当前优先

### P2 — 上下文管理器（暂缓，先占位）

- [ ] `lya-token` + `lya-context`
- [ ] 消费 `models.context_window`；按会话 `api_mode` 选 assembler

### P3 — 前端体验债

- [ ] 首页悬浮词的碰撞检测仍不准，两条都还在：`estimateBox()` 把 px 字号乘魔数
      （`fontSize * 0.038`、`fontSize * 0.065`）当百分比用，量纲是错的；`occupiedBoxes()`
      跳过 `phase === 'out'` 的标签，新词会生在正在淡出的词身上。坐标跳动已修

---

## 延后（有触发条件，不是放弃）

| 项 | 现状 | 什么时候做 |
|----|------|-----------|
| **记忆索引与前缀缓存冲突** | 索引是 system prompt 里唯一会变的一段（`TIME_ANCHOR` 刻意不写当前时间就是为了这个），排在第 9 段、人设之前。写一条记忆 → 从索引段起全部失效，包括**整段对话**。编号改成 id、排序改 id 升序之后新记忆只追加在末尾，但省不下什么：`共 N 条` 在所有条目上面，一改就是从段首失效。真要救只有把索引挪出 system prompt、放到对话之后的独立消息里，代价是模型更容易把它当对话内容而不是设定 | 对话长到一次 miss 明显肉疼时 |
| 多模态图片输入 | `models.toml` 的 `vision` capability 已接进提示词（会下确定断言），但 `lya-llm` 侧**没有图片输入通道**——`ChatMessage.content` 是纯 `String`，两条装配路径（`context.rs` / `context_responses.rs`）都只搬文本。声明了 vision 的模型实际也收不到图，提示词里明说了这点 | 补 `content` 多段结构时 |
| 图片上传 | Composer 是纯文本框，没有文件选择也没有拖放；后端无上传端点 | 依赖上面那条 |
| 媒体固有宽高进 `MediaMeta` | 视频已有兜底：CSS `aspect-ratio: var(--local-media-ratio, 16 / 9)`，加载后 JS 读 `videoWidth/Height` 换成真比例。**图片仍然没有占位**，`width/height: auto` 一加载就重排。根治要服务端存下宽高，两边都直接输出 `aspect-ratio` | 图片重排真的碍事时 |
| 本地图片缩略图 | 不生成，原图直接送；32 MiB 上限挡误引用 | 实际卡了再补 |
| 分支命名 | 无表无 API。`BranchInfo` 只有 `leaf_id` / `preview` / 时间，切换请求体也只有 `leaf_id` | WebUI 要显示分支名时 |
| 删子树 | 只有 `delete_leaf`，遇到非叶节点直接报 `NotLeaf` | 要先定「当前 leaf 在被删子树里」的语义 |
| 流式阶段性落盘 | 每轮只写两次库：开头一条空占位（界面靠它的 id 挂增量），结尾 `update_payload` 定稿。中途崩掉，库里留一条空的 `Streaming` 消息，正文只在 SSE 缓冲里 | 真觉得可惜时 |
| 确认期间的执行进度 | 批准只标记 `deferred`，真正执行在 HITL 的 HTTP handler 里同步跑 `flush_deferred_tool_executions`，全程不发 `CallStarted` / `CallFinished`。自动执行的工具是发的，所以是两套体验 | 让它也走事件流 |
| token 用量统计 | 完全不采集。`lya-llm` 里没有任何 `usage` / `*_tokens` 的解析 | 有地方展示时（会话详情或设置页） |
| LLM 重试策略 | 无，`LlmClient::post` 单次尝试，流错误直接 `TurnEndReason::Failed` 冒到 UI | 本地单用户场景先这样，好排查 |
| `bash` 沙箱 | 有的是资源闸门：超时、stdout/stderr 字节上限、`kill_on_drop`、取消轮询。**没有的是隔离**：`Command::new("bash")` 继承完整环境变量，cwd 由调用方给不做囚禁，安全全靠确认启发式 + HITL | 未定 |
| Action 取消信号 | `ActionCtx` 只有 `session_id` 和 `mode`。工具那边 `ToolCtx` 是有 `CancelToken` 的，动作没有 | 出现慢动作时 |
| `image_scan` EXIF | 只读文件头拿宽高与格式（`imagesize::size`），不解码也不读 EXIF | 真要整理照片时 |
| 配置文件监听 | 没有 watcher，也没有 `notify` 依赖；配置端点每次请求重新 `Config::load()` | 手动 reload 不够用时 |
| 密钥间接引用 | 明文存 `models.toml`（0600），不支持 `env:VAR` | 本地单用户够用 |
| 会话导出 | 无端点无入口。前端只有归档 / 改名 / 删除 | 未定 |
| 键盘快捷键 | 只有就地的 Enter / Escape（发送、编辑提交、关弹窗、关灯箱），没有任何全局快捷键 | 未定 |
| 移动端适配 | 四个断点的零星修补：720px（分栏改上下堆叠、聊天内边距与消息宽度、Composer 内边距）、640px（设置表单单列）、520px（MC 外壳），外加 `prefers-reduced-motion`。没有移动导航，没有触摸手势 | 真在手机上用时 |

---

## 刻意不做

| 项 | 原因 |
|----|------|
| 「默认配置」再拆子导航 | 七个面板堆一页里往下滚就够用，多一层导航是给自己找事 |
| 存储页的「清空」按钮 | 缓存本来就该留着；真要删有文件管理器。做了还得解释「清空硬链接释放 0 字节」 |
| lianclaw 记忆导入 | 记忆已经手工整理进库，lianclaw 那边的数据也删了，没有可导的东西了 |
| 工具参数进 TOML | 已定：只读 + `limits.rs` |
| 单个工具的权限收紧 | 会话级启用/禁用已够表达；再加一层让「为什么这个工具不能用」变难排查 |
| 通用的工具风险等级 | 真正需要确认的只有 `bash`，给所有工具挂一个只有它要回答的属性是冗余 |
| 子 agent / `spawn_agent` | 太复杂 |
| 插件系统 | lianclaw 的历史包袱 |
| skills 体系 | 工具提示词与实现放一起，不再抽一层 |
| `done` 动作 | 不带 `tool_calls` 的 assistant 消息即本轮结束 |
| 记忆自动召回 / embedding | 索引常驻已覆盖；lianclaw 建了 embedding 表一次都没用上 |
| 配置写回 `models.api_key` 的前端编辑页 | 密钥不出服务器 |
| `~/.lya/session/config/` 文件化 | 除非有明确迁移方案；当前 DB 够用 |
| 多用户鉴权 / 远程部署 | 不在范围内 |

对外的文章选题见 [`archive/plan.md`](./archive/plan.md)，那份是你自己维护的，与本文无关。

---

## 已完成（摘要）

细节不重复，git log 可查。

- [x] 记忆索引编号改用库内 id、排序改 id 升序，删掉 `pinned`：编号从「位置」变成「身份」，
      不再出现历史里的 `#2` 和当前索引的 `#2` 指两条记忆（2026-08-05）
- [x] 数据库 schema 合并进 `lya-db/migrations/000_init.sql`：`Db::open` 即带全库 schema，
      业务 crate 不再建表；删 `branch_meta` 与种子记忆；测试统一 `open_test_db()`（2026-08-05）
- [x] 媒体 `cache_*` → `retain_*`：删 `.ephemeral`，读写路径分开（有副本就用，只有开关开着才写新的），
      `MediaMeta` 诚实区分硬链接/拷贝并给出来源与落盘两处路径，加载失败有占位（2026-08-05）
- [x] 前端审计三批：行号对齐、移除东京夜、统一媒体路径条、首页动画分层、默认模型入口、
      显示偏好分会话、导航重排、存储页卡片化与分类修正、加载遮罩提到 `App.vue`、
      滚动位置记忆、跳到最新按钮（四态：隐藏 / 跟随 / 完毕 / 百分比）（2026-08-05）
- [x] 提示词审阅：删掉 `transcript` 幻觉、视觉能力改为按 capability 下断言、
      段落标题统一、补「连续失败要停下来报告」（2026-08-05）
- [x] `docs/` 下 10 份 AI 写的文档全部删除，只留本文。要翻旧内容：
      `git show f48e87a^:docs/<文件名>`（2026-08-04）
- [x] DeepSeek Responses API 双栈：`api_mode`、原生联网、持久化回放（2026-08-04）
- [x] 工具连续失败熔断 `max_consecutive_tool_failures`（2026-08-04）
- [x] 分支树节点详情：参数、工具结果、原始 payload（2026-08-04）
- [x] `web_fetch` 按行翻页（`start_line` / `end_line`），长文档能读全（2026-08-04）
- [x] 构建与安装脚本 `build.sh` / `install.sh`（2026-08-04）
- [x] 记忆页：列表 / 检索 / 建 / 改 / 删，独立一级导航（2026-08-03）
- [x] 会话设置扩面板：概览 / 工具 / 会话 / 显示（2026-08-03）
- [x] 工具数值抽到 `lya-tool/src/limits.rs`；ToolsView 只做只读 catalog（2026-08-03）
- [x] Wave A–F、调用组、crate 拆分、媒体 Phase 1、桌面通知
