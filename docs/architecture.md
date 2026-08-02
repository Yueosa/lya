#  crate 边界（目标架构）

活文档。**Wave E 已完成**（crate 拆分、`[media.*]` 配置、`GET /api/storage/stats`、设置页存储扇形图）。

## 职责一览

| Crate | 职责 | 不做什么 |
|-------|------|----------|
| **`lya-core`** | 进程入口：读配置、校验/迁移数据库、组装 Agent/Hub/API、绑端口、跑 Tokio | 不含 HTTP handler 实现、不含 SSE 逻辑、不含媒体缓存、不含磁盘统计 |
| **`lya-api`** | Axum 路由、REST handler、`guard`（同源/跨站）、静态 UI fallback | 不含 SessionHub 状态机、不含 `ensure_web` |
| **`lya-hub`** | `SessionHub`：轮次串行、spawn `run_turn`、SSE 广播、快照/`TurnBuffer`、`AgentEvent`→`Envelope` | 不含业务 HTTP、不含媒体字节 |
| **`lya-media`** | 会话媒体：`img_cache` / 预留 `vdo_cache`·`ado_cache`；`ensure_local` / `ensure_web`；Serving 所需元数据 | 不含 `~/.lya` 全局占用统计、不是 LLM tool |
| **`lya-storage`** | 数据目录观测：扫描 `data_root()` 体积分项（sessions、db、config…）；只读 API 供前端扇形图 | 不含媒体 fetch/缓存、不做清除（第一版） |

其余 crate 不变：`lya-agent`（轮次驱动）、`lya-session`、`lya-tool`、`lya-action`、`lya-config` 等。

## 依赖方向（只允许向内）

```text
lya (binary)
  └── lya-core          # wire + start
        ├── lya-api     # router(hub, …)
        ├── lya-hub     # SessionHub
        ├── lya-media   # 媒体服务
        └── lya-storage # 占用统计

lya-api → lya-hub, lya-media, lya-storage, lya-config, lya-session, …
lya-hub → lya-agent, lya-session
lya-media → lya-config, lya-http, lya-tool::web::net（SSRF 复用）
lya-storage → lya-config
```

## Wave E 迁移步骤（顺序固定）

1. **建 crate 空壳 + workspace 成员**（`lya-api`、`lya-hub`、`lya-media`、`lya-storage`）
2. **`lya-media`**：迁 `media_cache.rs` + 图片 serving 逻辑；`lya-api` 路由薄委托
3. **`lya-hub`**：迁 `hub.rs` + `event.rs`
4. **`lya-api`**：迁 `http/*` + `guard.rs` + `router()`
5. **`lya-core`**：仅保留 `run.rs` + `start_server`；**不** re-export 其它 crate 的类型
6. **`lya-storage`**：实现 `scan_usage()` + `GET /api/storage/stats`（经 `lya-api`）
7. 配置：`[media.*]` 进 `lya-config`；**不动用户本地 toml 时只改模板并告知用户**

## 与 tool / action 的边界

- **聊天里显示图片/音视频**：`lya-media` + 前端 Markdown/播放器（**不是** tool/action）
- **模型主动拉视频/音频**：将来 `lya-tool` 里的 tool，内部可调 `lya-media` 写入缓存
- **磁盘占用扇形图**：`lya-storage`，配置页只读展示
