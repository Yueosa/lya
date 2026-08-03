# 开发待办（归档）

**活待办已迁至 [`plan.md`](./plan.md)。** 完成项请在那里维护，不要改本文件。

---

## 历史 Wave 清单（只读）

Wave A–F、tool 调用组、crate 拆分、媒体 Phase 1、notify 等均已落地。明细见 git history 与 [`archive/`](./archive/)。

## 曾标记完成、易误解的项

| 项 | 实际交付 |
|----|----------|
| tool 配置 UI Phase 1 | **全局 tool 启用**（`runtime.tools.enabled`）+ 会话覆盖 + ToolsView **只读** limits；**不是** tool 数值可配 |
| vdo/ado Phase 1 | 播放器 + 缓存端点 + 提示词；**无** video_scan tool |

## 封存（不做）

- 工具/action 数值进 TOML（改 `limits.rs` 即可）
- 配置写回 models api_key 的前端编辑页
- bash 沙箱、Action cancel、embedding、配置 watcher、全局 SSE 等

完整列表见 [`plan.md` → 刻意不做](./plan.md#刻意不做)。

## Backlog 原条目

| # | 项 | 状态 |
|---|-----|------|
| 4 | 上下文管理器 | 暂缓 → [`plan.md` P2](./plan.md) |
