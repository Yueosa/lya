# lya-action

元认知动作：模型用来操作**自己的状态**（记忆、模式、与用户交互）。

## 与 lya-tool 的关系

对外同一套 function calling 协议；内部按两类治理：

| | 工具 | 动作 |
|---|---|---|
| 作用对象 | 外部环境 | 自己的状态 |
| 用户可否禁用 | 可以 | 不可以 |
| 流转 | 回灌后继续 | 可能挂起等人（HITL） |

## 用法

```rust
use lya_action::ActionRegistry;

let mut actions = ActionRegistry::new();
register_actions(&mut actions, memory)?;
```
