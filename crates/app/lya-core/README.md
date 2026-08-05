# lya-core

进程启动层：读配置、开库、组装 Agent 与 Hub、挂载 [`lya-api`](../lya-api) 路由并监听。

## 职责

- [`start_server`](src/run.rs)：Config → Db → Agent → Hub → API router → 监听
- **只**导出 `start_server` / `ServerHandle` / `RunError`；其它类型从对应 crate 直接 `use`

## 不在此 crate

| 内容 | Crate |
|------|-------|
| HTTP 路由与 handler | [`lya-api`](../lya-api) |
| SessionHub、SSE 信封 | [`lya-hub`](../lya-hub) |
| 媒体缓存与 serving | [`lya-media`](../../capability/lya-media) |
| 磁盘占用统计 | [`lya-storage`](../../domain/lya-storage) |

依赖方向只允许向内：`lya-core → {lya-api, lya-hub, ...}`。媒体与存储**不**直接依赖，
它们由 `lya-api` 挂路由时用到。
