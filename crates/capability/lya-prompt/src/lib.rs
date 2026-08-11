//! # lya-prompt
//!
//! 组装发给 LLM 的 system prompt。
//!
//! ## 拼接顺序
//!
//! ```text
//! [环境] → [身份] → [运行] → [表达修正] → [时间+界面] → [动作] → [工具]
//!   → [能力补充] → [模式] → [口吻]
//! ```
//!
//! 全局段来自 `prompt.toml`；身份与口吻在创建会话时抄进数据库，之后会话级独立。
//! 记忆索引由 agent 挂在 messages 尾部，不进本 crate 的 system 拼接。

#![deny(missing_docs)]

pub mod builder;
pub mod identity;
pub mod input;
pub mod media;
pub mod sections;

pub use builder::PromptBuilder;
pub use identity::{
    format_environment, format_identity, format_operations, format_style, format_voice,
    DEFAULT_ENVIRONMENT, DEFAULT_IDENTITY, DEFAULT_OPERATIONS, DEFAULT_STYLE, DEFAULT_VOICE,
    RESPONSES_NATIVE_SEARCH, TIME_ANCHOR,
};
#[allow(deprecated)]
pub use identity::{format_persona_section, DEFAULT_PERSONA, SELF_AWARENESS, SYSTEM_AWARENESS};
pub use media::chat_media_section;
pub use input::PromptInput;
pub use sections::SystemSections;
