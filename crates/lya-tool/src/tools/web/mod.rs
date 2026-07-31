//! 网络工具：搜索与抓取。
//!
//! 两者分工明确——`web_search` 找网址，`web_fetch` 读正文。搜索摘要通常不足以
//! 回答问题，所以提示词里明确要求命中之后再去读。

mod fetch;
pub mod html;
mod search;

pub use fetch::WebFetchTool;
pub use search::WebSearchTool;
