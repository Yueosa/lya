//! OpenAI Responses API（`/responses`）wire 类型与解析。

pub mod body;
pub mod input;
pub mod sse;

pub use body::build_responses_body;
pub use sse::ResponsesSseParser;
