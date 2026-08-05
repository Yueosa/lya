//! 会话级媒体留存与校验（`img_cache` / `vdo_cache` / `ado_cache`）。
//!
//! ## 职责
//!
//! - 把聊天里引用的**本地路径**或**远程 URL** 变成可以直接喂给响应的字节
//! - 按类别的留存策略决定要不要在会话目录里留一份；本地文件优先硬链接
//! - 抓远程之前判一次 SSRF（内网、回环、自己的端口一律拒绝）
//! - 校验 MIME 与体积上限
//!
//! ## 非职责
//!
//! - 不做缩略图、不转码、不改尺寸——原样进出
//! - 不管 HTTP 路由与 Range 分片，那是 `lya-api` 的事
//! - 不读配置文件：限额由调用方以 [`CategoryLimits`] 传进来
//!
//! ## 「留存」不是「缓存」
//!
//! 两个开关（`retain_local` / `retain_web`）控制的不是命中，而是**要不要自己留一份**。
//! 读和写因此分开判断：留存目录里有副本就用，不管开关当下是什么状态；只有开着的时候
//! 才把新下载的写进去。这样关掉开关只影响以后，已经留下的不会突然命中不了又重下一遍。

#![deny(missing_docs)]

mod cache;

pub use cache::{
    CachedMedia, CategoryLimits, MediaBytes, MediaCacheError, MediaCategory, MediaLimits,
    RetainKind, Retained, cache_root, ensure_local, ensure_web, remove_session_media, session_dir,
};
