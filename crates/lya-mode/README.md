# lya-mode

工作模式：`ask` / `edit` / `agent` 与 RWX 权限上限。

## 职责

- 模式 → 权限上限映射
- 当前模式的 system prompt 段
- 将会话启用工具名交给 `ToolRegistry`，叠加权限过滤

## 用法

```rust
use lya_mode::{Mode, resolve};

let bundle = resolve(mode, &enabled_tools, &registry)?;
// bundle.mode_prompt, bundle.tools.prompt, bundle.tools.schemas
```
