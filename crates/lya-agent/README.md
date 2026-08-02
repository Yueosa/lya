# lya-agent

一轮对话的驱动器：读会话树 → 组 prompt → 调 LLM → 分发工具/动作 → 回写消息树。

## 职责

- 唯一知道「一轮怎么跑」的 crate
- `run_turn` 返回流式事件；drop 即停，由上层 `SessionHub` 持有
- HITL 不挂内存：表单发出后本轮结束，用户答复再 append 并继续

## 循环规则

assistant 消息带 `tool_calls` → 执行并回灌、继续下一轮；不带 → 结束本轮。

## 用法

```rust
use lya_agent::{Agent, AgentParts};

let agent = Agent::new(parts)?;
let stream = agent.run_turn(&session_id, cancel).await?;
```
