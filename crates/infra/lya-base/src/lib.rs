//! # lya-base
//!
//! 谁都要用的那几个词。**没有任何 lya 依赖**，所以它永远在依赖图最底下。
//!
//! ## 为什么要有这一层
//!
//! 这些类型原先散在各自「看起来最相关」的 crate 里，代价有两种：
//!
//! - **倒挂**。`Mode` 住在 `lya-mode`，而那个 crate 为了按权限筛工具依赖 `lya-tool`
//!   （进而 `lya-http`）。于是 `lya-config` 只为了表达 `default_work_mode = "agent"`，
//!   就被垫到了整个工具层之上——一个读 TOML 的 crate 排在 HTTP 客户端后面。
//! - **各写一份**。`ApiMode` 和 capability 键在 `lya-llm` 与 `lya-config` 里**逐字
//!   重复**过；数据根 `~/.lya` 在 `lya-db` 与 `lya-config` 里也各解析了一遍。两份
//!   定义只要有一次改得不一样，配置里写的和请求里发的就对不上，而编译器一声不响。
//!
//! ## 什么能进来
//!
//! 只收同时满足两条的东西：
//!
//! 1. **跨层出现**——基础设施和上层都要用，放在任何一边都会造成倒挂。
//! 2. **不依赖任何东西**——只用 std，不碰 IO，不认识任何业务概念。
//!
//! 反过来说：需要 IO、需要别的 crate、或者只有一个 crate 用的，都不该进。
//! 「反正大家都能用」不是理由，那是杂物间的开场白。
//!
//! 绝大多数住户都是词汇（[`Mode`]、[`ApiMode`]、[`Permission`]）。[`Live`] 是唯一
//! 的例外：它是个几十行的纯 std 小结构，不是名词。收它是因为 agent、工具、记忆三
//! 层都要「配置改了立刻生效」这一个能力，各写一份的话就又回到了本 crate 开头列的
//! 第二种代价上。

#![deny(missing_docs)]

mod api_mode;
mod error;
mod live;
mod mode;
mod paths;
mod permission;

pub use api_mode::{ApiMode, CAPABILITY_TEXT, CAPABILITY_VISION, CAPABILITY_WEB_SEARCH};
pub use error::{BaseError, ModeParseError};
pub use live::Live;
pub use mode::Mode;
pub use paths::{DATA_DIR_NAME, data_root};
pub use permission::{Permission, PermissionParseError};
