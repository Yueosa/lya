# lya-prompt

组装发给 LLM 的 **system prompt**。

## 本 crate 负责

- 系统认知（你是 lya…）
- 自我认知（主会话助手边界；可指引查阅记忆）
- 人设（全局默认 + 会话覆盖；始终放最后）

## 由外部注入（本 crate 不实现）

- **元认知 / Action** ← 未来 `lya-action`
- **工具说明** ← `lya-tool` 的 `ToolBundle.prompt`
- **工作模式** ← 未来 `agent_mode`

## 用法

```rust
use lya_prompt::{PromptBuilder, PromptInput};

let builder = PromptBuilder::new().with_persona("温柔一点");
let system = builder.build(&PromptInput {
    tool_section: Some(bundle.prompt.clone()),
    mode_section: Some(mode_text),
    ..Default::default()
});
```
