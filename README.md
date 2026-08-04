# lya

一个本地 agent 应用：单二进制（HTTP + 内嵌前端 + Linux 托盘）

进度与取舍：[计划](./docs/plan.md)

## 构建

```sh
./build.sh      # 产物到 output/lya_<版本>_<系统>_<架构>/
./install.sh    # 构建并安装到 ~/.local/bin/lya
```

前端必须先构建——WebUI 是 rust-embed 从 `web/dist/` 编进二进制的，两个脚本都已经
按顺序处理好了。

## LLM 双栈

lya 支持两种互斥的 API 栈，会话创建时按 `runtime.toml` 的 `default_api_mode` 选定；
落下第一条消息前还能在会话设置里改，之后锁定。

| 栈 | HTTP | 联网搜索 |
|----|------|----------|
| **Completions**（默认） | `POST /v1/chat/completions` | DuckDuckGo `web_search` tool |
| **Responses** | `POST /v1/responses` | 模型原生 `web_search`；DDG 搜索 tool 自动关闭 |

两种栈均保留 `web_fetch`（本地 SSRF 防护、读正文）。模型能力在 `~/.lya/models.toml`
的 `[models.modes.*]` 中声明；例如 Pro 只配 `modes.completions` 时，不能用于
Responses 会话。
