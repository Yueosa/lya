# lya 计划（唯一活文档）

`docs/` 下只有这一份活文档。别再新建 `TODO-xxx.md` / `roadmap.md` 之类的第二份清单——
之前就是三份清单互相打架，最后谁都不准。

维护规则：

1. 新条目进 P0–P3；做完打 `[x]` 加日期。
2. 决定不做的挪进「刻意不做」，写清原因；有触发条件的挪进「延后」。
3. 改配置层级时先改 `lya-config/src/lib.rs` 注释，再改本文「配置体系」。
4. 本文只记**计划与取舍**。要写给人读的说明文档，见「文档撰写计划」。

---

## 配置体系（现状，以代码为准）

数据目录：`~/.lya/`。

| 层级 | 存储 | 内容 | 谁改 |
|------|------|------|------|
| 进程 | `core.toml` | 端口、DB 路径、HTTP 超时、日志 | 设置页 / 手改；**改完重启** |
| 全局默认 | `runtime.toml` | 新会话默认：agent、tools.enabled、shell、memory、media | 设置 → **默认配置**；热加载 |
| 模型清单 | `models.toml` | id、密钥、`modes.*`（capabilities + params）、context_window | 模型页只读 + 手改 |
| 全局人设 | `persona.toml` | 人设正文 | 设置 → 人设 |
| **会话** | **`lya.db` → `sessions` 表** | work_mode、model_id、api_mode、persona、enabled_tools_json | 聊天侧栏会话设置 |

**没有** `~/.lya/session/config/`。会话级配置在数据库；全局默认只在 `runtime.toml`。

### 前端页职责（不要混）

| 页面 | 职责 |
|------|------|
| **设置 → 默认配置** | 编辑 `runtime.toml`（对话 / 工具默认 / 记忆 / 媒体） |
| **设置 → 人设 / 存储 / 原始文件** | 同上文件体系 |
| **工具** | **只读** catalog：参数 schema、prompt_hint、内置限制 |
| **模型** | **只读** `models.toml` |
| **聊天 → 会话设置（侧栏）** | 本会话 tool 开关、action 只读、显示偏好；空会话可改 API 栈 |

### 工具数值

- **不可配置**：上限/默认值在 `crates/lya-tool/src/limits.rs`
- **可配置**：新会话默认启用哪些 tool → `runtime.toml` `[tools].enabled`
- 前端只读表：`web/src/utils/toolLimits.ts`（改 `limits.rs` 时同步）

---

## 当前优先

### P0 — 手头的

- [ ] **提示词让模型判断不了自己有没有视觉能力**。`image_scan` 的描述说「要知道图里
      画了什么，得用支持看图的模型」，媒体段又说「你不具备看视频/听音频内容的能力
      ——除非本会话模型本身支持多模态」。两句拼一起，模型在思考里反复纠结「这意味着
      我可能有视觉能力？无法确定」。2026-08-04 实测复现。
- [ ] **默认配置**再拆 UI 子导航（对话 / 工具 / 记忆 / 媒体），与 `runtime.toml` 段落对齐
- [ ] `runtime.toml` 文件拆分 — **不做**

### P1 — 文档重写

`docs/` 下原有 10 份文档已删（见「文档撰写计划」）。按需要自己写，不急。

### P2 — 上下文管理器（暂缓，先占位）

- [ ] `lya-token` + `lya-context`
- [ ] 消费 `models.context_window`；按会话 `api_mode` 选 assembler

### P3 — 前端体验债

- [ ] **三套外壳的特色**。现在东京夜与 MTF 共用默认侧栏。方向：东京夜走紧凑信息流
      （窄侧栏、列表带预览行）、MTF 走宽松卡片（带偏移阴影、留白大）。Minecraft 已做。
      边界不变：外壳可换，视图不可换（`src/shell/types.ts`）
- [ ] **跳到最新按钮**。右下角，四态：隐藏 / 正在跟随 / 输出完毕 / 百分比。
      同时解决「我翻到哪了」和「要不要跟随流式」
- [ ] 首页悬浮词：随机 spawn + 淡入淡出（已做一版，待验收）
- [ ] SessionSettings 与 `usePrefs` 分区标注（「本机显示」vs「本会话」）

---

## 延后（有触发条件，不是放弃）

| 项 | 现状 | 什么时候做 |
|----|------|-----------|
| 多模态 | `image_scan` 只给路径与尺寸，模型看不见画面 | 接支持视觉的模型时；P0 那条提示词问题的根 |
| 图片上传 | 无 | 依赖多模态 |
| `branch_meta` 分支命名 | 表已建，无 API；分支靠叶节点 id 区分 | WebUI 要显示分支名时 |
| 删子树 | 只允许 `delete_leaf` | 要先定「当前 leaf 在被删子树里」的语义 |
| 流式阶段性落盘 | 只落两次库（占位、定稿），崩在中途丢半截正文 | 真觉得可惜时 |
| 确认期间的执行进度 | `resolve_tool_confirm` 同步跑完才返回，界面干等 | 让它也走事件流 |
| token 用量统计 | 不采集 | 有地方展示时（会话详情或设置页） |
| LLM 重试策略 | 无，错误直接冒到 UI | 本地单用户场景先这样，好排查 |
| `bash` 沙箱 | 子进程继承完整环境，确认流程是唯一闸门 | 未定 |
| Action 取消信号 | `ActionCtx` 无 `CancelToken`；动作都很快 | 出现慢动作时 |
| `web_fetch` 翻页 | 只有 `max_chars` 截断，无偏移量 | 真遇到读不全的长文档 |
| `image_scan` EXIF | 不读；要另加解析库 | 真要整理照片时 |
| 配置文件监听 | 「热改」= 重新 `load_from` 一次 | 手动 reload 不够用时 |
| 密钥间接引用 | 明文存 `models.toml`（0600），不支持 `env:VAR` | 本地单用户够用 |
| 本地图片缩略图 | 不生成；32 MiB 上限挡误引用 | 实际卡了再补 |
| 会话导出 | 无 | 未定 |
| 键盘快捷键 / 移动端适配 | 无 | 未定 |
| lianclaw 记忆导入 | 无脚本。字段对应：topic→title、summary→summary、md 正文→body、tags_json→tags，namespace 丢弃 | 真要迁时写个一次性命令 |

---

## 刻意不做

| 项 | 原因 |
|----|------|
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

---

## 文档撰写计划

原 `docs/` 下 10 份文档在 2026-08-04 全部删除——那些是 AI 写的，不足以信任。
下面是**值得重新写**的主题，按需要自己来，不设期限。

删除前的内容可从 git 取回：`git show <删除提交>^:docs/<文件名>`。

| 主题 | 要覆盖什么 | 原文件 |
|------|-----------|--------|
| 架构 / crate 边界 | 依赖方向（只允许向内）、各 crate 不做什么 | `architecture.md` |
| 媒体缓存 | `img/vdo/ado_cache` 布局、会话 media 端点、Range、SSRF 与令牌规则 | `media-cache.md` |
| 调用组协议 | `tool_batch` 元数据、SSE 事件、HITL `‹i/n›`、时间戳前缀语义 | `tool-batch.md` |
| Responses 双栈 | `api_mode` 何时锁定、SSE 事件映射、`web_search_call` 回放 | `responses-api.md` |
| 代码与注释风格 | Rust / Vue 约定、提示词段落标题。**仓库现在没有 `AGENTS.md`，这部分没有落脚点** | `style.md` |
| 设计决策记录 | 为什么上下文是树、HITL 为什么进树、为什么砍子 agent、从 lianclaw 学到的教训 | `archive/PLAN.md` |

对外的文章选题另见 [`archive/plan.md`](./archive/plan.md)，那份是你自己维护的，与本表无关。

---

## 已完成（摘要）

细节不重复，git log 可查。

- [x] DeepSeek Responses API 双栈：`api_mode`、原生联网、持久化回放（2026-08-04）
- [x] 工具连续失败熔断 `max_consecutive_tool_failures`（2026-08-04）
- [x] 分支树节点详情：参数、工具结果、原始 payload（2026-08-04）
- [x] 构建与安装脚本 `build.sh` / `install.sh`（2026-08-04）
- [x] 会话设置扩面板：概览 / 工具 / 会话 / 显示（2026-08-03）
- [x] 工具数值抽到 `lya-tool/src/limits.rs`；ToolsView 只做只读 catalog（2026-08-03）
- [x] 记忆 slot 编号 + 索引 #1 置顶（2026-08-03）
- [x] Wave A–F、调用组、crate 拆分、媒体 Phase 1、桌面通知
