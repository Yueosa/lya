# lya

一个本地 agent 应用：单二进制（HTTP + 内嵌前端 + Linux 托盘）

前言：[为什么想做 lya](./docs/lya.md)

进度与取舍：[路线图](./docs/roadmap.md)

## LLM 双栈

lya 支持两种互斥的 API 栈，**在创建会话时锁定**，之后不可切换：

| 栈 | HTTP | 联网搜索 |
|----|------|----------|
| **Completions**（默认） | `POST /v1/chat/completions` | DuckDuckGo `web_search` tool |
| **Responses** | `POST /v1/responses` | 模型原生 `web_search`；DDG 搜索 tool 自动关闭 |

两种栈均保留 `web_fetch`（本地 SSRF 防护、读正文）。模型能力在 `~/.lya/models.toml` 的 `[models.modes.*]` 中声明；例如 Pro 只配 `modes.completions` 时，不能用于 Responses 会话。

完整规格：[docs/responses-api.md](./docs/responses-api.md)
