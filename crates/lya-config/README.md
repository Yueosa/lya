# lya-config

分层配置，全部落在 `~/.lya/` 下。

## 文件

| 文件 | 层级 | 何时生效 |
|------|------|----------|
| `core.toml` | 端口、日志、库路径 | 重启 |
| `runtime.toml` | 轮数上限、默认模式/模型 | 重新加载 |
| `models.toml` | 模型清单（含密钥） | 重新加载 |
| `persona.toml` | 全局人设 | 重新加载 |

会话级设置（模式、工具、人设）存在 `sessions` 表，不在此 crate。

## 用法

```rust
use lya_config::Config;

let config = Config::load()?;
let default_model = config.default_model();
```

透传字段（如 `max_tokens`）写在 `models.toml`，由 `lya-llm` 原样带进请求体。
