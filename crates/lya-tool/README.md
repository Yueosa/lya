# lya-tool

工具定义与注册中心。

## 职责

- 定义单个工具的 meta / parameters / prompt_hint / call
- 启动时注册全部工具
- 按名字列表 + RWX 权限筛选，导出提示词段与 OpenAI `tools[]` schema
- 内置数值上限集中在 [`src/limits.rs`](src/limits.rs)（不可通过 `runtime.toml` 配置）

## 用法

```rust
use lya_tool::{Permission, ToolRegistry};

let registry = ToolRegistry::new();
// 启动时 register(...)

let bundle = registry.bundle(None, Permission::READ_WRITE_EXEC);
// bundle.prompt  → 拼进 system
// bundle.schemas → 塞进 chat/completions 的 tools
```
