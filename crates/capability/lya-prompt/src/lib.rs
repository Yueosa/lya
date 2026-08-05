//! # lya-prompt
//!
//! 组装发给 LLM 的 system prompt。
//!
//! ## 职责划分
//!
//! | 段落 | 来源 |
//! |------|------|
//! | 系统认知 / 自我认知 / 人设 | **本 crate 内置** |
//! | 元认知（Action） | 调用方注入，未来由 `lya-action` 提供 |
//! | 工具说明 | 调用方注入，通常来自 `lya-tool::ToolBundle.prompt` |
//! | 工作模式 | 调用方注入，未来由 `agent_mode` 提供 |
//! | 记忆索引 | 调用方注入，来自 `lya-memory` |
//!
//! ## 拼接顺序
//!
//! ```text
//! [系统] → [自我认知] → [时间] → [界面] → [动作] → [工具] → [能力补充] → [模式] → [记忆] → [人设]
//! ```
//!
//! 段落标题统一成 `=== [类别] 中文说明 ===` 一种形状，避免出现 `[运行时]`
//! 这种按来源命名、`[Responses]` 这种按 API 命名的混排。
//!
//! 人设放最后：只影响语气风格，并在文案中声明不覆盖前面的行为规则。
//!
//! ## 明确不做什么
//!
//! - 不实现子 agent 模板（当前已砍）
//! - 不依赖 `lya-tool` / `lya-llm`：只收已经拼好的字符串段落
//! - 不往 user/tool 消息写时间戳（由上层嵌入消息前缀）

#![deny(missing_docs)]

pub mod builder;
pub mod identity;
pub mod input;
pub mod media;

pub use builder::PromptBuilder;
pub use identity::{
    DEFAULT_PERSONA, RESPONSES_NATIVE_SEARCH, SELF_AWARENESS, SYSTEM_AWARENESS, TIME_ANCHOR,
};
pub use media::chat_media_section;
pub use input::PromptInput;
