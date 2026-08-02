# 代码与注释风格

团队默认约定；新代码尽量对齐，旧代码随触达顺手改。

## Rust

- **模块注释**：crate 根 `lib.rs` 写能力清单；子模块用 `//!` 说明职责即可，不写「不负责 xxx」清单。
- **函数注释**：解释**为什么**和边界条件，不重复签名能看出的行为。
- **分段**：大 `impl` 块用 `// ── 标题 ──` 分隔（见 `lya-session/src/store/`）。
- **错误**：领域错误用 crate 内枚举；HTTP 层统一 `Into<ApiError>`，不把 rusqlite 细节 leak 出去。
- **事务**：一次 public API = 一次 `db.read` / `db.write`，不在 caller 侧拼事务。

## 提示词段落标题

注入 LLM 的可读段落统一：

```text
=== [元认知] Actions ===
=== [工具] Tools ===
=== [记忆] … ===   （memory_write 标题前缀，见 lya-action）
```

`lya-prompt` 只负责系统/自我/人设；Actions、Tools 由各自 registry 生成后传入 builder。

## 前端（Vue / TS）

- **分层**：`api/` → `store/` → `app/` composables → `views/`；视图不直接拼 fetch URL。
- **SSE**：会话事件走 hub 订阅；UI 状态以服务端快照 + 增量事件为准。
- **样式**：Tokyo Night 变量；动画优先 `opacity`，避免布局抖动。
- **注释**：组件顶部一句话说明页面职责；复杂交互（HITL、分支）写清数据从哪来。

## 文档

- 活路线图：`docs/roadmap.md`
- 历史计划：`docs/archive/`（只读参考，不再更新）
- 产品动机：`docs/lya.md`
